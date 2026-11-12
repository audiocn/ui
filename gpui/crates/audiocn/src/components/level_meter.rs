//! Port of `components/ui/level-meter.tsx`: a peak and RMS level meter with
//! zones, peak hold, a scale, a readout and a clip light.
//!
//! **How it maps.** React paints the meter outside React from a
//! `requestAnimationFrame` task that writes CSS variables (`--meter-level`,
//! `--meter-rms`, `--meter-hold`). Here [`LevelMeter`] steps the same painter
//! ([`MeterPainter`]) when it renders, and calls
//! `window.request_animation_frame()` only while the painter says it needs
//! another frame, so a settled meter costs nothing. A [`FrameSource`] is a
//! shared slot, so the meter reads its version on every render, and while
//! asleep polls it so a producer on another thread wakes it.
//!
//! The parts (`LevelMeterChannels`, `LevelMeterChannel`, `LevelMeterTrack`,
//! `LevelMeterBar`, `LevelMeterHold`, `LevelMeterScale`, `LevelMeterValue`,
//! `LevelMeterClip`) read the surrounding meter through a context, like
//! React's `useLevelMeter`: [`use_level_meter`] works in any element rendered
//! inside a `LevelMeter`, so you can build your own parts on it.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, ColorSpace, ContentMask, Context, ElementId, Entity, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Task, WeakEntity, Window, canvas, div,
    fill, linear_color_stop, linear_gradient, point, prelude::FluentBuilder, px, size,
};

use crate::audio::ballistics::{
    Ballistics, BallisticsInput, BallisticsOptions, BallisticsPreset, resolve_ballistics,
};
use crate::audio::decibels::{
    DEFAULT_MAX_DB, DEFAULT_MIN_DB, FormatDbOptions, SILENCE_DB, format_db,
};
use crate::audio::frame_loop::{FrameTask, PainterClock, create_frame_task, now_ms};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::taper::{AnyTaper, TaperInput, resolve_taper};
use crate::audio::types::{ChannelLevel, MeterFrame, MeterZone, MeterZoneName, Orientation, Taper};
use crate::audio::zones::{CLIP_HOLD_MS, CLIP_THRESHOLD_DB, DEFAULT_ZONES, zone_for_db};
use crate::components::clip_indicator::{ClipIndicator, ClipIndicatorActions};
use crate::components::db_readout::{
    DbReadout, DbReadoutChannel, DbReadoutDecorate, DbReadoutDisplay, DbReadoutMeasure,
    read_channel,
};
use crate::components::db_scale::{DbScale, DbScaleSide, DbScaleTick};
use crate::config::{AudioConfig, AudioSize};
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::ActiveTheme;
use crate::ui::styles::{provide, use_context};

const ARIA_INTERVAL_MS: f64 = 250.0;
const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
const DEFAULT_SEGMENTS: usize = 24;
const POSITION_EPSILON: f32 = 0.0005;
/// A meter this close to its input has nothing visible left to animate (about
/// 0.06 dB on the default range, under a pixel on any meter), so it sleeps.
const SETTLE_EPSILON: f32 = 0.001;
/// How often a sleeping meter checks its source for new frames.
const SOURCE_POLL_MS: u64 = 16;
/// `[--meter-gap:0.25rem]`.
const METER_GAP: f32 = 4.0;
/// `[--meter-scale-size:1rem]`.
const SCALE_SIZE: f32 = 16.0;
/// The gap between a segment and the next (`calc(100% / n - 2px)`).
const SEGMENT_GAP: f32 = 2.0;

/// `variant`: `Segmented` is an LED ladder. Default `Solid`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LevelMeterVariant {
    #[default]
    Solid,
    Segmented,
    Gradient,
}

/// Which measurement a bar shows (`measure`). Same as the readout's.
pub type LevelMeterMeasure = DbReadoutMeasure;

/// The colour of each zone (`--meter-ok`, `--meter-warn`, `--meter-clip`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelMeterColors {
    pub ok: Hsla,
    pub warn: Hsla,
    pub clip: Hsla,
}

impl LevelMeterColors {
    /// The theme's meter colours.
    pub fn from_theme(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            ok: theme.meter_ok,
            warn: theme.meter_warn,
            clip: theme.meter_clip,
        }
    }

    pub fn get(&self, zone: MeterZoneName) -> Hsla {
        match zone {
            MeterZoneName::Ok => self.ok,
            MeterZoneName::Warn => self.warn,
            MeterZoneName::Clip => self.clip,
        }
    }

    fn set(&mut self, zone: MeterZoneName, color: Hsla) {
        match zone {
            MeterZoneName::Ok => self.ok = color,
            MeterZoneName::Warn => self.warn = color,
            MeterZoneName::Clip => self.clip = color,
        }
    }
}

// ---------------------------------------------------------------------------
// Zone fill (`--meter-fill`)

/// One stretch of the zone fill along a track, 0..1 from its start: a solid
/// zone colour (`from == to`) or a blend between two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneFillPiece {
    pub start: f32,
    pub end: f32,
    pub from: MeterZoneName,
    pub to: MeterZoneName,
}

/// Zones in rising order, as a new array (stable, like the TS insertion).
fn by_from_db(zones: &[MeterZone]) -> Vec<MeterZone> {
    let mut sorted: Vec<MeterZone> = Vec::with_capacity(zones.len());
    for zone in zones {
        match sorted.iter().position(|other| other.from_db > zone.from_db) {
            Some(index) => sorted.insert(index, *zone),
            None => sorted.push(*zone),
        }
    }
    sorted
}

/// `buildZoneFill`: the `--meter-fill` gradient as pieces. `Gradient` blends
/// from each zone's start to the next; the others are hard stops.
pub fn build_zone_fill(
    zones: &[MeterZone],
    taper: &dyn Taper,
    variant: LevelMeterVariant,
) -> Vec<ZoneFillPiece> {
    let sorted = by_from_db(zones);
    let Some(first) = sorted.first() else {
        return Vec::new();
    };
    // Rounded like the CSS stops; a stop never sits before the previous one.
    let mut starts = Vec::with_capacity(sorted.len());
    let mut previous = 0.0f32;
    for zone in &sorted {
        let start = ((taper.to_position(zone.from_db) * 100.0 * 1000.0).round() / 1000.0 / 100.0)
            .max(previous);
        starts.push(start);
        previous = start;
    }
    let mut pieces = Vec::new();
    let mut push = |start: f32, end: f32, from: MeterZoneName, to: MeterZoneName| {
        if end > start {
            pieces.push(ZoneFillPiece {
                start,
                end,
                from,
                to,
            });
        }
    };
    // Before the first stop, the first colour.
    push(0.0, starts[0], first.zone, first.zone);
    for (index, zone) in sorted.iter().enumerate() {
        let start = starts[index];
        let end = starts.get(index + 1).copied().unwrap_or(1.0);
        let to = if variant == LevelMeterVariant::Gradient {
            sorted.get(index + 1).map_or(zone.zone, |next| next.zone)
        } else {
            zone.zone
        };
        push(start, end, zone.zone, to);
    }
    pieces
}

/// The visible stretches of a track (0..1 from its start) under the segment
/// mask (`buildSegmentMask`): each segment minus a 2px gap at its far end.
fn segment_spans(length: f32, segments: Option<usize>) -> Vec<(f32, f32)> {
    match segments {
        Some(count) if count > 0 && length > 0.0 => {
            let size = length / count as f32;
            (0..count)
                .map(|k| {
                    let start = k as f32 * size;
                    (start, (start + size - SEGMENT_GAP).max(start))
                })
                .collect()
        }
        _ => vec![(0.0, length)],
    }
}

/// A stretch along a track's axis (pixels from its start: left, or bottom
/// when vertical) as a rectangle in window coordinates.
fn axis_rect(bounds: Bounds<Pixels>, horizontal: bool, start: f32, end: f32) -> Bounds<Pixels> {
    let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if horizontal {
        Bounds::new(point(px(ox + start), px(oy)), size(px(end - start), px(h)))
    } else {
        Bounds::new(
            point(px(ox), px(oy + h - end)),
            size(px(w), px(end - start)),
        )
    }
}

/// Paints the zone fill of a track from its start up to `level` (0..1),
/// clipped to the pill shape of the track, under an optional segment mask.
/// Used by bars and by the unlit segments of the `Segmented` variant.
#[allow(clippy::too_many_arguments)]
pub fn paint_zone_fill(
    bounds: Bounds<Pixels>,
    orientation: Orientation,
    pieces: &[ZoneFillPiece],
    colors: &LevelMeterColors,
    level: f32,
    segments: Option<usize>,
    opacity: f32,
    window: &mut Window,
) {
    let horizontal = orientation == Orientation::Horizontal;
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let length = if horizontal { w } else { h };
    let reach = level.clamp(0.0, 1.0) * length;
    if reach <= 0.0 || opacity <= 0.0 {
        return;
    }
    let radius = px(w.min(h) / 2.0);
    let angle = if horizontal { 90.0 } else { 0.0 };
    let spans = segment_spans(length, segments);
    for piece in pieces {
        let (a, b) = (piece.start * length, (piece.end * length).min(reach));
        if b <= a {
            continue;
        }
        let from = colors.get(piece.from);
        let to = colors.get(piece.to);
        for &(s0, s1) in &spans {
            let (start, end) = (a.max(s0), b.min(s1));
            if end <= start {
                continue;
            }
            let mask = ContentMask {
                bounds: axis_rect(bounds, horizontal, start, end),
            };
            window.with_content_mask(Some(mask), |window| {
                let quad = if piece.from == piece.to {
                    fill(bounds, from.opacity(opacity))
                } else {
                    fill(
                        bounds,
                        linear_gradient(
                            angle,
                            linear_color_stop(from.opacity(opacity), piece.start),
                            linear_color_stop(to.opacity(opacity), piece.end),
                        )
                        .color_space(ColorSpace::Oklab),
                    )
                };
                window.paint_quad(quad.corner_radii(radius));
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Painter (`createMeterPainter`)

/// A meter's range, taper and zones (`MeterScale`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeterScale {
    pub min_db: f32,
    pub max_db: f32,
    pub taper: AnyTaper,
    pub zones: Vec<MeterZone>,
}

/// What one channel shows this frame: the values React writes as
/// `--meter-level`, `--meter-rms`, `--meter-hold`, `data-zone` and `data-active`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterChannelReading {
    /// Smoothed peak position, 0..1.
    pub level: f32,
    /// Smoothed RMS position, 0..1.
    pub rms: f32,
    /// Peak-hold position, 0..1.
    pub hold: f32,
    /// The zone of the smoothed peak.
    pub zone: MeterZoneName,
    /// The level is above the bottom of the range.
    pub active: bool,
}

impl Default for MeterChannelReading {
    fn default() -> Self {
        Self {
            level: 0.0,
            rms: 0.0,
            hold: 0.0,
            zone: MeterZoneName::Ok,
            active: false,
        }
    }
}

#[derive(Clone, Debug)]
struct ChannelState {
    peak: Ballistics,
    rms: Ballistics,
    level: f32,
    rms_level: f32,
    hold: f32,
    zone: MeterZoneName,
}

/// Only moves a written position when it changed visibly (`writePosition`).
fn write_position(previous: f32, next: f32) -> f32 {
    if (previous - next).abs() > POSITION_EPSILON {
        next
    } else {
        previous
    }
}

/// The meter's root state after a paint: what React writes as
/// `data-clipping`, `data-zone`, `aria-valuenow` and `aria-valuetext`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeterRootReading {
    pub clipping: bool,
    pub zone: Option<MeterZoneName>,
    /// The loudest level, clamped to the range (`aria-valuenow`).
    pub value_now: Option<f32>,
    /// The loudest level as text (`aria-valuetext`).
    pub value_text: Option<SharedString>,
}

/// Steps a meter's ballistics each frame and keeps what it shows. Returns
/// from [`paint`](Self::paint) whether it needs another frame, so a meter
/// that has settled stops costing frames until something wakes it.
#[derive(Clone, Debug)]
pub struct MeterPainter {
    ballistics: BallisticsOptions,
    reduced_motion: bool,
    states: BTreeMap<usize, ChannelState>,
    clock: PainterClock,
    scale: MeterScale,
    clip_until: f64,
    last_aria_ms: f64,
    root: MeterRootReading,
    last_paint_ms: f64,
}

impl MeterPainter {
    pub fn new(
        ballistics: Option<&BallisticsInput>,
        reduced_motion: bool,
        scale: MeterScale,
    ) -> Self {
        let ballistics = if reduced_motion {
            BallisticsPreset::Instant.options()
        } else {
            resolve_ballistics(ballistics)
        };
        Self {
            ballistics,
            reduced_motion,
            states: BTreeMap::new(),
            clock: PainterClock::default(),
            scale,
            clip_until: 0.0,
            last_aria_ms: f64::NEG_INFINITY,
            root: MeterRootReading::default(),
            last_paint_ms: f64::NEG_INFINITY,
        }
    }

    /// A new range, taper or zones, used from the next frame.
    pub fn set_scale(&mut self, scale: MeterScale) {
        self.scale = scale;
    }

    pub fn scale(&self) -> &MeterScale {
        &self.scale
    }

    /// What channel `index` shows, once painted.
    pub fn reading(&self, index: usize) -> Option<MeterChannelReading> {
        self.states.get(&index).map(|state| MeterChannelReading {
            level: state.level.max(0.0),
            rms: state.rms_level.max(0.0),
            hold: state.hold.max(0.0),
            zone: state.zone,
            active: state.level > 0.0,
        })
    }

    /// The root's state after the last paint.
    pub fn root(&self) -> &MeterRootReading {
        &self.root
    }

    fn state_for(&mut self, index: usize) -> &mut ChannelState {
        let options = self.ballistics;
        self.states.entry(index).or_insert_with(|| ChannelState {
            peak: Ballistics::new(options),
            rms: Ballistics::new(BallisticsOptions {
                peak_hold_ms: 0.0,
                ..options
            }),
            level: -1.0,
            rms_level: -1.0,
            hold: -1.0,
            zone: MeterZoneName::Ok,
        })
    }

    /// Returns the smoothed peak in dB and whether the channel has settled.
    fn paint_channel(&mut self, index: usize, input: ChannelLevel, now_ms: f64) -> (f32, bool) {
        let taper = self.scale.taper.clone();
        let zones = self.scale.zones.clone();
        let state = self.state_for(index);
        let input_rms_db = input.rms_db.unwrap_or(input.peak_db);
        let peak = state.peak.step(input.peak_db, now_ms);
        let rms = state.rms.step(input_rms_db, now_ms);
        let level = taper.to_position(peak.db);
        let rms_level = taper.to_position(rms.db);
        let hold = taper.to_position(peak.hold_db);
        state.level = write_position(state.level, level);
        state.rms_level = write_position(state.rms_level, rms_level);
        state.hold = write_position(state.hold, hold);
        state.zone = zone_for_db(peak.db, &zones);
        let settled = (level - taper.to_position(input.peak_db)).abs() <= SETTLE_EPSILON
            && (rms_level - taper.to_position(input_rms_db)).abs() <= SETTLE_EPSILON
            && (hold - level).abs() <= SETTLE_EPSILON;
        (peak.db, settled)
    }

    fn paint_root(&mut self, loudest: f32, now_ms: f64, settled: bool) {
        self.root.clipping = now_ms < self.clip_until;
        let text: SharedString = format_db(
            loudest,
            FormatDbOptions {
                floor_db: self.scale.min_db,
                ..Default::default()
            },
        )
        .into();
        let zone = zone_for_db(loudest, &self.scale.zones);
        if self.root.value_text.as_ref() == Some(&text) && self.root.zone == Some(zone) {
            return;
        }
        // Throttled while the level moves; a settled level is written at once.
        if !settled && now_ms - self.last_aria_ms < ARIA_INTERVAL_MS {
            return;
        }
        self.last_aria_ms = now_ms;
        self.root.value_text = Some(text);
        self.root.zone = Some(zone);
        self.root.value_now = Some(loudest.clamp(self.scale.min_db, self.scale.max_db));
    }

    /// Paints one frame of `channels` from the latest input frame at frame
    /// time `frame_ms`. Returns true while the meter needs another frame.
    pub fn paint(
        &mut self,
        frame_ms: f64,
        latest: Option<&MeterFrame>,
        channels: &[usize],
    ) -> bool {
        let now_ms = self.clock.tick(frame_ms);
        if self.reduced_motion && now_ms - self.last_paint_ms < REDUCED_MOTION_INTERVAL_MS {
            return true;
        }
        self.last_paint_ms = now_ms;
        let mut loudest = SILENCE_DB;
        let mut settled = true;
        let mut input_clipping = false;
        for &index in channels {
            let input = latest
                .and_then(|frame| frame.channels.get(index).copied())
                .unwrap_or(ChannelLevel::SILENT);
            if input.peak_db >= CLIP_THRESHOLD_DB {
                self.clip_until = now_ms + CLIP_HOLD_MS;
                input_clipping = true;
            }
            let (db, channel_settled) = self.paint_channel(index, input, now_ms);
            loudest = loudest.max(db);
            settled &= channel_settled;
        }
        self.paint_root(loudest, now_ms, settled);
        // A clip light counting down after the input dropped needs the frames
        // that turn it off; one held by a still-clipping input does not.
        let releasing_clip = now_ms < self.clip_until && !input_clipping;
        !settled || releasing_clip
    }
}

/// `serializeLevels`: the declarative levels as a key, `None` when there are none.
fn declared_frame(
    channels: Option<&[ChannelLevel]>,
    peak_db: Option<f32>,
    rms_db: Option<f32>,
) -> Option<MeterFrame> {
    if let Some(channels) = channels {
        return Some(MeterFrame {
            channels: channels.to_vec(),
        });
    }
    if peak_db.is_none() && rms_db.is_none() {
        return None;
    }
    Some(MeterFrame::mono(ChannelLevel {
        peak_db: peak_db.unwrap_or(SILENCE_DB),
        rms_db,
    }))
}

// ---------------------------------------------------------------------------
// Context (`useLevelMeter`)

#[derive(Default)]
struct MeterShared {
    /// Channel indices rendered in the last layout.
    registered: RefCell<BTreeSet<usize>>,
    /// A `LevelMeterScale` was rendered in the last layout.
    has_scale: Cell<bool>,
}

/// The surrounding meter's range, scale, frames and readings, for parts built
/// on it (React's `LevelMeterContextValue`). Get it with [`use_level_meter`].
#[derive(Clone)]
pub struct LevelMeterContext {
    pub orientation: Orientation,
    pub variant: LevelMeterVariant,
    pub size: AudioSize,
    /// `--meter-thickness`: 4, 8 or 12 px by size.
    pub thickness: Pixels,
    pub min_db: f32,
    pub max_db: f32,
    pub taper: AnyTaper,
    pub zones: Vec<MeterZone>,
    /// Tracks the meter shows: declared, seen in the frames, or `channel_count`.
    pub channel_count: usize,
    /// Every frame the meter accepts (source, declared levels, actions).
    pub frames: FrameSource<MeterFrame>,
    /// The declarative levels, when the meter has values instead of a source.
    pub declared: Option<MeterFrame>,
    /// Segments of the `Segmented` variant.
    pub segments: usize,
    /// The zone colours (`--meter-ok/warn/clip`).
    pub colors: LevelMeterColors,
    /// The zone fill (`--meter-fill`).
    pub fill: Rc<Vec<ZoneFillPiece>>,
    /// The root's clip / zone / value state (`data-clipping`, ...).
    pub root: MeterRootReading,
    /// The channel index, inside a `LevelMeterChannel`.
    pub channel: Option<usize>,
    readings: Rc<BTreeMap<usize, MeterChannelReading>>,
    shared: Rc<MeterShared>,
    scale_padding: bool,
}

impl LevelMeterContext {
    /// What channel `index` shows this frame.
    pub fn reading(&self, index: usize) -> MeterChannelReading {
        self.readings.get(&index).copied().unwrap_or_default()
    }

    /// What the surrounding `LevelMeterChannel` shows (channel 0 outside one):
    /// React's `var(--meter-level)` etc.
    pub fn channel_reading(&self) -> MeterChannelReading {
        self.reading(self.channel.unwrap_or(0))
    }

    /// The segment mask for this meter's variant.
    pub fn segment_mask(&self) -> Option<usize> {
        (self.variant == LevelMeterVariant::Segmented).then_some(self.segments)
    }

    /// Marks a channel as rendered, so the meter paints it (React's
    /// `registerChannel`). `LevelMeterChannel` calls it; custom channel parts
    /// that don't wrap one should too.
    pub fn register_channel(&self, index: usize, window: &mut Window) {
        self.shared.registered.borrow_mut().insert(index);
        if !self.readings.contains_key(&index) {
            // Not painted this frame: paint it next frame.
            window.request_animation_frame();
        }
    }

    fn with_channel(&self, index: usize) -> Self {
        Self {
            channel: Some(index),
            ..self.clone()
        }
    }
}

/// The surrounding meter, if this element renders inside a [`LevelMeter`].
pub fn try_use_level_meter() -> Option<LevelMeterContext> {
    use_context::<LevelMeterContext>()
}

/// The surrounding meter's range, scale and frames, for parts built on it
/// (React's `useLevelMeter(part)`). Panics outside a [`LevelMeter`].
pub fn use_level_meter(part: &str) -> LevelMeterContext {
    try_use_level_meter().unwrap_or_else(|| panic!("{part} must be used inside LevelMeter."))
}

// ---------------------------------------------------------------------------
// Parts

/// The tracks of a meter, stacked (horizontal) or side by side (vertical).
#[derive(IntoElement, Default)]
pub struct LevelMeterChannels {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LevelMeterChannels {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RenderOnce for LevelMeterChannels {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterChannels");
        let horizontal = meter.orientation == Orientation::Horizontal;
        let mut el = div()
            .relative()
            .flex()
            .min_h_0()
            .min_w_0()
            .flex_1()
            .gap(px(METER_GAP))
            .map(|el| {
                if horizontal {
                    el.flex_col()
                } else {
                    el.h_full().flex_row()
                }
            });
        el.style().refine(&self.style);
        el.children(self.children)
    }
}

/// One track's row (or column): which channel of the frames it shows.
#[derive(IntoElement, Default)]
pub struct LevelMeterChannel {
    index: usize,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LevelMeterChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Which channel of the frames this track shows. Default 0.
    pub fn index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }
}

impl RenderOnce for LevelMeterChannel {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterChannel");
        meter.register_channel(self.index, window);
        let horizontal = meter.orientation == Orientation::Horizontal;
        let mut el = div()
            .flex()
            .min_h_0()
            .min_w_0()
            .map(|el| if horizontal { el.w_full() } else { el.h_full() });
        el.style().refine(&self.style);
        provide(meter.with_channel(self.index), el.children(self.children))
    }
}

/// The background of a track; holds the bars and the hold marker.
#[derive(IntoElement, Default)]
pub struct LevelMeterTrack {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LevelMeterTrack {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RenderOnce for LevelMeterTrack {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterTrack");
        let horizontal = meter.orientation == Orientation::Horizontal;
        let segmented = meter.variant == LevelMeterVariant::Segmented;
        let (orientation, fill_pieces, colors, segments) = (
            meter.orientation,
            meter.fill.clone(),
            meter.colors,
            meter.segments,
        );
        let mut el = div()
            .relative()
            .overflow_hidden()
            .rounded_full()
            .bg(cx.theme().muted)
            .map(|el| {
                if horizontal {
                    el.h(meter.thickness).w_full()
                } else {
                    el.h_full().w(meter.thickness)
                }
            })
            // The unlit LEDs: the zone fill under the mask, at 20%.
            .when(segmented, |el| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            paint_zone_fill(
                                bounds,
                                orientation,
                                &fill_pieces,
                                &colors,
                                1.0,
                                Some(segments),
                                0.2,
                                window,
                            );
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            });
        el.style().refine(&self.style);
        el.children(self.children)
    }
}

/// The lit part of a track, in the zone colours. Layer a `Rms` and a `Peak`
/// bar for a dual meter.
#[derive(IntoElement)]
pub struct LevelMeterBar {
    measure: LevelMeterMeasure,
    opacity: f32,
}

impl Default for LevelMeterBar {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeterBar {
    pub fn new() -> Self {
        Self {
            measure: LevelMeterMeasure::Peak,
            opacity: 1.0,
        }
    }

    /// Which measurement this bar shows. Default `Peak`.
    pub fn measure(mut self, measure: LevelMeterMeasure) -> Self {
        self.measure = measure;
        self
    }

    /// Bar opacity (React `className="opacity-40"`). Default 1.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

impl RenderOnce for LevelMeterBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterBar");
        let reading = meter.channel_reading();
        let level = match self.measure {
            LevelMeterMeasure::Rms => reading.rms,
            LevelMeterMeasure::Peak => reading.level,
        };
        let mask = meter.segment_mask();
        let (orientation, pieces, colors, opacity) = (
            meter.orientation,
            meter.fill.clone(),
            meter.colors,
            self.opacity,
        );
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paint_zone_fill(
                    bounds,
                    orientation,
                    &pieces,
                    &colors,
                    level,
                    mask,
                    opacity,
                    window,
                );
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// The peak-hold marker: a 2px line at the held peak, hidden at silence.
#[derive(IntoElement, Default)]
pub struct LevelMeterHold {
    color: Option<Hsla>,
}

impl LevelMeterHold {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marker colour. Default `foreground` at 80%.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for LevelMeterHold {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterHold");
        let hold = meter.channel_reading().hold;
        let horizontal = meter.orientation == Orientation::Horizontal;
        let color = self
            .color
            .unwrap_or_else(|| cx.theme().foreground.opacity(0.8));
        // `opacity-[calc(var(--meter-hold)*50)]`.
        let opacity = (hold * 50.0).clamp(0.0, 1.0);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if opacity <= 0.0 {
                    return;
                }
                let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let length = if horizontal { w } else { h };
                let end = hold * length;
                let mask = ContentMask {
                    bounds: axis_rect(bounds, horizontal, end - 2.0, end),
                };
                window.with_content_mask(Some(mask), |window| {
                    window.paint_quad(
                        fill(bounds, color.opacity(color.a * opacity))
                            .corner_radii(px(w.min(h) / 2.0)),
                    );
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// A [`DbScale`] with the meter's range, taper and orientation. Horizontal
/// meters reserve space below the tracks for it.
#[derive(IntoElement)]
pub struct LevelMeterScale {
    ticks: Option<Vec<f32>>,
    side: DbScaleSide,
    labels: bool,
    format: Option<Rc<dyn Fn(f32) -> SharedString>>,
    children: Vec<DbScaleTick>,
    style: StyleRefinement,
}

impl Default for LevelMeterScale {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeterScale {
    pub fn new() -> Self {
        Self {
            ticks: None,
            side: DbScaleSide::End,
            labels: true,
            format: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Tick values. Default: common values inside the range.
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = Some(ticks.into_iter().collect());
        self
    }

    /// Which side of the marks the labels sit on. Default `End`.
    pub fn side(mut self, side: DbScaleSide) -> Self {
        self.side = side;
        self
    }

    /// Show labels. Default true.
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// A custom tick.
    pub fn child(mut self, tick: DbScaleTick) -> Self {
        self.children.push(tick);
        self
    }
}

impl Styled for LevelMeterScale {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LevelMeterScale {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterScale");
        let horizontal = meter.orientation == Orientation::Horizontal;
        if horizontal {
            meter.shared.has_scale.set(true);
            if !meter.scale_padding {
                window.request_animation_frame();
            }
        }
        let mut scale = DbScale::new()
            .min_db(meter.min_db)
            .max_db(meter.max_db)
            .orientation(meter.orientation)
            .taper(meter.taper.clone())
            .side(self.side)
            .labels(self.labels)
            .children(self.children)
            .when_some(self.ticks, |scale, ticks| scale.ticks(ticks))
            .when_some(self.format, |scale, format| {
                scale.format(move |db| format(db))
            })
            // `absolute top-[calc(100%+var(--meter-gap))] left-0 h-(--meter-scale-size)`.
            .when(horizontal, |scale| {
                scale
                    .absolute()
                    .left_0()
                    .bottom(px(-(SCALE_SIZE + METER_GAP)))
                    .h(px(SCALE_SIZE))
            });
        scale.style().refine(&self.style);
        scale
    }
}

/// The level as text, with the meter's floor: a live readout of the meter's
/// frames, or its declared value.
#[derive(IntoElement)]
pub struct LevelMeterValue {
    id: ElementId,
    measure: DbReadoutMeasure,
    channel: DbReadoutChannel,
    interval_ms: Option<f64>,
    hold_ms: Option<f64>,
    decimals: Option<usize>,
    unit: Option<bool>,
    floor_db: Option<f32>,
    zones: Option<Vec<MeterZone>>,
    format: Option<Rc<dyn Fn(f32) -> SharedString>>,
    decorate: Option<DbReadoutDecorate>,
    style: StyleRefinement,
}

impl Default for LevelMeterValue {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeterValue {
    pub fn new() -> Self {
        Self {
            id: "level-meter-value".into(),
            measure: DbReadoutMeasure::Peak,
            channel: DbReadoutChannel::Max,
            interval_ms: None,
            hold_ms: None,
            decimals: None,
            unit: None,
            floor_db: None,
            zones: None,
            format: None,
            decorate: None,
            style: StyleRefinement::default(),
        }
    }

    /// Element id, unique inside the meter. Default `"level-meter-value"`.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn measure(mut self, measure: DbReadoutMeasure) -> Self {
        self.measure = measure;
        self
    }

    pub fn channel(mut self, channel: impl Into<DbReadoutChannel>) -> Self {
        self.channel = channel.into();
        self
    }

    pub fn interval_ms(mut self, interval_ms: f64) -> Self {
        self.interval_ms = Some(interval_ms);
        self
    }

    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.hold_ms = Some(hold_ms);
        self
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    pub fn unit(mut self, unit: bool) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Default: the meter's `min_db`.
    pub fn floor_db(mut self, floor_db: f32) -> Self {
        self.floor_db = Some(floor_db);
        self
    }

    pub fn zones(mut self, zones: impl Into<Vec<MeterZone>>) -> Self {
        self.zones = Some(zones.into());
        self
    }

    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// See [`DbReadout::decorate`].
    pub fn decorate(
        mut self,
        decorate: impl Fn(&DbReadoutDisplay, gpui::Div) -> AnyElement + 'static,
    ) -> Self {
        self.decorate = Some(Rc::new(decorate));
        self
    }
}

impl Styled for LevelMeterValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LevelMeterValue {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterValue");
        // Declarative levels only change on render, so show them as a value. A
        // stream keeps its live readout, which falls to −∞ when it stops.
        let value = meter
            .declared
            .as_ref()
            .map(|frame| read_channel(frame, self.measure, self.channel));
        let source = meter.declared.is_none().then(|| meter.frames.clone());
        let mut readout = DbReadout::new(self.id)
            .text_color(cx.theme().muted_foreground)
            .text_size(px(12.0))
            .line_height(px(16.0))
            .floor_db(self.floor_db.unwrap_or(meter.min_db))
            .source(source)
            .value_opt(value)
            .measure(self.measure)
            .channel(self.channel)
            .when_some(self.interval_ms, DbReadout::interval_ms)
            .when_some(self.hold_ms, DbReadout::hold_ms)
            .when_some(self.decimals, DbReadout::decimals)
            .when_some(self.unit, DbReadout::unit)
            .when_some(self.zones, DbReadout::zones)
            .when_some(self.format, |r, format| r.format(move |db| format(db)))
            .when_some(self.decorate, |r, decorate| {
                r.decorate(move |display, el| decorate(display, el))
            });
        readout.style().refine(&self.style);
        readout
    }
}

/// A [`ClipIndicator`] fed by the meter's frames.
#[derive(IntoElement)]
pub struct LevelMeterClip {
    inner: ClipIndicator,
}

impl Default for LevelMeterClip {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeterClip {
    pub fn new() -> Self {
        Self {
            inner: ClipIndicator::new("level-meter-clip"),
        }
    }

    /// Element id, unique inside the meter. Default `"level-meter-clip"`.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.inner = self.inner.with_id(id);
        self
    }

    pub fn clipping(mut self, clipping: bool) -> Self {
        self.inner = self.inner.clipping(clipping);
        self
    }

    pub fn threshold_db(mut self, threshold_db: f32) -> Self {
        self.inner = self.inner.threshold_db(threshold_db);
        self
    }

    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.inner = self.inner.hold_ms(hold_ms);
        self
    }

    pub fn on_clipping_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.inner = self.inner.on_clipping_change(handler);
        self
    }

    pub fn show_count(mut self, show_count: bool) -> Self {
        self.inner = self.inner.show_count(show_count);
        self
    }

    pub fn actions(mut self, actions: ClipIndicatorActions) -> Self {
        self.inner = self.inner.actions(actions);
        self
    }
}

impl ParentElement for LevelMeterClip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements);
    }
}

impl Styled for LevelMeterClip {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl RenderOnce for LevelMeterClip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LevelMeterClip");
        self.inner.source(meter.frames)
    }
}

// ---------------------------------------------------------------------------
// LevelMeter

/// Imperative handle (React `actionsRef`): paint frames from your own loop,
/// or drop the meter to silence.
#[derive(Clone, Default)]
pub struct LevelMeterActions {
    state: Rc<RefCell<Option<WeakEntity<LevelMeterState>>>>,
}

impl LevelMeterActions {
    pub fn new() -> Self {
        Self::default()
    }

    fn entity(&self) -> Option<Entity<LevelMeterState>> {
        self.state.borrow().as_ref().and_then(WeakEntity::upgrade)
    }

    /// Paint a frame directly, for callers that own their own frame loop.
    pub fn paint(&self, frame: MeterFrame, cx: &mut App) {
        if let Some(state) = self.entity() {
            state.update(cx, |state, cx| {
                state.accept_and_count(frame);
                cx.notify();
            });
        }
    }

    /// Drop the level and the peak hold to silence.
    pub fn reset(&self, cx: &mut App) {
        if let Some(state) = self.entity() {
            state.update(cx, |state, cx| {
                state.accept(MeterFrame::default());
                cx.notify();
            });
        }
    }

    /// Whether the meter shows a clip (React's `data-clipping`).
    pub fn clipping(&self, cx: &App) -> bool {
        self.entity()
            .is_some_and(|state| state.read(cx).painter.root().clipping)
    }
}

/// Keyed state of a [`LevelMeter`].
pub struct LevelMeterState {
    painter: MeterPainter,
    painter_key: (BallisticsOptions, bool),
    task: FrameTask,
    emitter: FrameEmitter<MeterFrame>,
    latest: Option<MeterFrame>,
    source: Option<FrameSource<MeterFrame>>,
    version: Option<u64>,
    observed_count: Option<usize>,
    declared: Option<Option<MeterFrame>>,
    shared: Rc<MeterShared>,
    polling: Rc<Cell<bool>>,
    poll: Option<Task<()>>,
}

impl LevelMeterState {
    fn accept(&mut self, frame: MeterFrame) {
        self.emitter.emit(frame.clone());
        self.latest = Some(frame);
        self.task.wake();
    }

    fn accept_and_count(&mut self, frame: MeterFrame) {
        let count = frame.channels.len();
        self.accept(frame);
        if count > 0 {
            self.observed_count = Some(count);
        }
    }

    /// Reads the source's latest frame if it is new.
    fn consume(&mut self) {
        let Some(source) = &self.source else {
            return;
        };
        let version = source.version();
        if self.version == Some(version) {
            return;
        }
        self.version = Some(version);
        if let Some(frame) = source.latest() {
            self.accept_and_count(frame);
        }
    }

    /// While asleep with a source, checks it for new frames off the frame loop.
    fn ensure_poll(&mut self, cx: &mut Context<Self>) {
        if self.polling.get() {
            return;
        }
        let Some(source) = self.source.clone() else {
            return;
        };
        let seen = self.version;
        let polling = self.polling.clone();
        polling.set(true);
        self.poll = Some(cx.spawn(async move |this, cx| {
            let interval = Duration::from_millis(SOURCE_POLL_MS);
            loop {
                cx.background_executor().timer(interval).await;
                if Some(source.version()) != seen {
                    polling.set(false);
                    this.update(cx, |_, cx| cx.notify()).ok();
                    break;
                }
            }
        }));
    }
}

/// A peak and RMS level meter with zones, peak hold, a scale, a readout and a
/// clip light. Give it a [`FrameSource`] or static levels; without children it
/// renders one track per channel.
///
/// ```ignore
/// LevelMeter::new("mic").source(signal.meter.clone())
/// LevelMeter::new("quiet").peak_db(-42.0)
/// LevelMeter::new("program")
///     .source(meter)
///     .child(
///         LevelMeterChannels::new()
///             .child(LevelMeterChannel::new().index(0).child(
///                 LevelMeterTrack::new().child(LevelMeterBar::new()).child(LevelMeterHold::new()),
///             ))
///             .child(LevelMeterScale::new()),
///     )
///     .child(LevelMeterValue::new())
///     .child(LevelMeterClip::new())
/// ```
#[derive(IntoElement)]
pub struct LevelMeter {
    id: ElementId,
    source: Option<FrameSource<MeterFrame>>,
    peak_db: Option<f32>,
    rms_db: Option<f32>,
    channels: Option<Vec<ChannelLevel>>,
    channel_count: Option<usize>,
    min_db: Option<f32>,
    max_db: Option<f32>,
    zones: Option<Vec<MeterZone>>,
    ballistics: Option<BallisticsInput>,
    taper: TaperInput,
    orientation: Option<Orientation>,
    variant: LevelMeterVariant,
    segments: usize,
    size: Option<AudioSize>,
    config: AudioConfig,
    actions: Option<LevelMeterActions>,
    aria_label: Option<SharedString>,
    zone_colors: Vec<(MeterZoneName, Hsla)>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LevelMeter {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            peak_db: None,
            rms_db: None,
            channels: None,
            channel_count: None,
            min_db: None,
            max_db: None,
            zones: None,
            ballistics: None,
            taper: TaperInput::Linear,
            orientation: None,
            variant: LevelMeterVariant::Solid,
            segments: DEFAULT_SEGMENTS,
            size: None,
            config: AudioConfig::default(),
            actions: None,
            aria_label: None,
            zone_colors: Vec::new(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// A meter source; the meter paints itself on animation frames.
    pub fn source(mut self, source: impl Into<Option<FrameSource<MeterFrame>>>) -> Self {
        self.source = source.into();
        self
    }

    /// Mono peak level in dBFS, for declarative use.
    pub fn peak_db(mut self, peak_db: f32) -> Self {
        self.peak_db = Some(peak_db);
        self
    }

    /// Mono RMS level in dBFS, for declarative use.
    pub fn rms_db(mut self, rms_db: f32) -> Self {
        self.rms_db = Some(rms_db);
        self
    }

    /// Levels per channel, for declarative multi-channel use.
    pub fn channels(mut self, channels: impl Into<Vec<ChannelLevel>>) -> Self {
        self.channels = Some(channels.into());
        self
    }

    /// Tracks to render before data arrives. Default 1.
    pub fn channel_count(mut self, channel_count: usize) -> Self {
        self.channel_count = Some(channel_count);
        self
    }

    /// Bottom of the displayed range. Default −60.
    pub fn min_db(mut self, min_db: f32) -> Self {
        self.min_db = Some(min_db);
        self
    }

    /// Top of the displayed range. Default 0.
    pub fn max_db(mut self, max_db: f32) -> Self {
        self.max_db = Some(max_db);
        self
    }

    /// Colour zones. Default ok / warn from −20 / clip from −9.
    pub fn zones(mut self, zones: impl Into<Vec<MeterZone>>) -> Self {
        self.zones = Some(zones.into());
        self
    }

    /// How the meter moves. Default `Peak`.
    pub fn ballistics(mut self, ballistics: impl Into<BallisticsInput>) -> Self {
        self.ballistics = Some(ballistics.into());
        self
    }

    /// Scale law. Default `Linear`.
    pub fn taper(mut self, taper: impl Into<TaperInput>) -> Self {
        self.taper = taper.into();
        self
    }

    /// Default horizontal.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// `Segmented` is an LED ladder. Default `Solid`.
    pub fn variant(mut self, variant: LevelMeterVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Segments for the `Segmented` variant. Default 24.
    pub fn segments(mut self, segments: usize) -> Self {
        self.segments = segments;
        self
    }

    /// Track thickness: `Sm` 4px, `Default` 8px, `Lg` 12px.
    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Settings from a surrounding strip or mixer (range, zones, ballistics,
    /// orientation, size, dimmed). Explicit props win.
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    /// Imperative paint / reset (React `actionsRef`).
    pub fn actions(mut self, actions: LevelMeterActions) -> Self {
        self.actions = Some(actions);
        self
    }

    /// Accessible name (React `aria-label`).
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Overrides a zone colour (React `[--meter-ok:…]` etc.).
    pub fn zone_color(mut self, zone: MeterZoneName, color: impl Into<Hsla>) -> Self {
        self.zone_colors.push((zone, color.into()));
        self
    }
}

impl ParentElement for LevelMeter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for LevelMeter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

macro_rules! parent_styled_part {
    ($($ty:ty),*) => {$(
        impl ParentElement for $ty {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }
        impl Styled for $ty {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }
    )*};
}
parent_styled_part!(LevelMeterChannels, LevelMeterChannel, LevelMeterTrack);

/// `--meter-thickness` by size.
pub fn level_meter_thickness(size: AudioSize) -> Pixels {
    px(match size {
        AudioSize::Sm => 4.0,
        AudioSize::Default => 8.0,
        AudioSize::Lg => 12.0,
    })
}

impl RenderOnce for LevelMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Explicit props win over the surrounding mixer or strip.
        let config = &self.config;
        let ballistics = self
            .ballistics
            .clone()
            .or_else(|| config.ballistics.clone())
            .unwrap_or_default();
        let min_db = self.min_db.or(config.min_db).unwrap_or(DEFAULT_MIN_DB);
        let max_db = self.max_db.or(config.max_db).unwrap_or(DEFAULT_MAX_DB);
        let orientation = self.orientation.or(config.orientation).unwrap_or_default();
        let size = self.size.or(config.size).unwrap_or_default();
        let zones = self
            .zones
            .clone()
            .or_else(|| config.zones.clone())
            .unwrap_or_else(|| DEFAULT_ZONES.to_vec());
        let dimmed = config.dimmed;
        let reduced_motion = prefers_reduced_motion(cx);
        let taper = resolve_taper(&self.taper, min_db, max_db);
        let scale = MeterScale {
            min_db,
            max_db,
            taper: taper.clone(),
            zones: zones.clone(),
        };
        let painter_key = (resolve_ballistics(Some(&ballistics)), reduced_motion);
        let declared = declared_frame(self.channels.as_deref(), self.peak_db, self.rms_db);
        let declared_count = declared.as_ref().map(|frame| frame.channels.len());

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| LevelMeterState {
            painter: MeterPainter::new(Some(&ballistics), reduced_motion, scale.clone()),
            painter_key,
            task: create_frame_task(),
            emitter: create_frame_emitter(),
            latest: None,
            source: None,
            version: None,
            observed_count: None,
            declared: None,
            shared: Rc::default(),
            polling: Rc::default(),
            poll: None,
        });
        if let Some(actions) = &self.actions {
            *actions.state.borrow_mut() = Some(state.downgrade());
        }

        let source = self.source.clone();
        let channel_count_prop = self.channel_count;
        let (needs_frame, readings, root, frames, channel_count, shared, scale_padding) = state
            .update(cx, |state, cx| {
                if state.painter_key != painter_key {
                    // New ballistics: a new painter, from the current levels.
                    state.painter_key = painter_key;
                    state.painter =
                        MeterPainter::new(Some(&ballistics), reduced_motion, scale.clone());
                    state.task = create_frame_task();
                } else if *state.painter.scale() != scale {
                    // A new range or zones reaches the painter, and repaints a
                    // meter that had settled.
                    state.painter.set_scale(scale.clone());
                    state.task.wake();
                }
                if state.source != source {
                    state.source = source.clone();
                    state.version = None;
                    state.poll = None;
                    state.polling.set(false);
                }
                state.consume();
                if state.declared.as_ref() != Some(&declared) {
                    state.declared = Some(declared.clone());
                    if let Some(frame) = declared.clone() {
                        state.accept(frame);
                    }
                }

                let channel_count = declared_count
                    .or(state.observed_count)
                    .or(channel_count_prop)
                    .unwrap_or(1);
                // Paint the channels rendered last time (or the default tracks).
                let registered: Vec<usize> =
                    std::mem::take(&mut *state.shared.registered.borrow_mut())
                        .into_iter()
                        .collect();
                let channels: Vec<usize> = if registered.is_empty() {
                    (0..channel_count).collect()
                } else {
                    registered
                };
                let latest = state.latest.clone();
                let painter = &mut state.painter;
                let needs_frame = state.task.run(now_ms(), |now| {
                    painter.paint(now, latest.as_ref(), &channels)
                });
                if needs_frame {
                    state.poll = None;
                    state.polling.set(false);
                } else if state.source.is_some() {
                    state.ensure_poll(cx);
                }
                let readings: BTreeMap<usize, MeterChannelReading> = channels
                    .iter()
                    .filter_map(|&index| state.painter.reading(index).map(|r| (index, r)))
                    .collect();
                let scale_padding = state.shared.has_scale.replace(false);
                (
                    needs_frame,
                    readings,
                    state.painter.root().clone(),
                    state.emitter.source(),
                    channel_count,
                    state.shared.clone(),
                    scale_padding,
                )
            });
        if needs_frame {
            window.request_animation_frame();
        }

        let mut colors = LevelMeterColors::from_theme(cx);
        for (zone, color) in &self.zone_colors {
            colors.set(*zone, *color);
        }
        let fill = Rc::new(build_zone_fill(&zones, &taper, self.variant));
        let horizontal = orientation == Orientation::Horizontal;
        let context = LevelMeterContext {
            orientation,
            variant: self.variant,
            size,
            thickness: level_meter_thickness(size),
            min_db,
            max_db,
            taper,
            zones,
            channel_count,
            frames,
            declared: if self.source.is_some() {
                None
            } else {
                declared
            },
            segments: self.segments,
            colors,
            fill,
            root: root.clone(),
            channel: None,
            readings: Rc::new(readings),
            shared,
            scale_padding,
        };

        let children = if self.children.is_empty() {
            vec![
                LevelMeterChannels::new()
                    .children((0..channel_count).map(|index| {
                        LevelMeterChannel::new().index(index).child(
                            LevelMeterTrack::new()
                                .child(LevelMeterBar::new())
                                .child(LevelMeterHold::new()),
                        )
                    }))
                    .into_any_element(),
            ]
        } else {
            self.children
        };

        let mut el = div()
            .id(self.id.clone())
            .flex()
            .gap(px(8.0))
            .role(gpui::Role::Meter)
            .aria_min_numeric_value(min_db as f64)
            .aria_max_numeric_value(max_db as f64)
            .aria_numeric_value(root.value_now.unwrap_or(min_db) as f64)
            .when_some(root.value_text.clone(), |el, text| el.aria_value(text))
            .when_some(self.aria_label.clone(), |el, label| el.aria_label(label))
            .when(dimmed, |el| el.opacity(0.5))
            .map(|el| {
                if horizontal {
                    el.w_full()
                        .flex_row()
                        .items_center()
                        .when(scale_padding, |el| el.pb(px(SCALE_SIZE + METER_GAP)))
                } else {
                    el.min_h(px(128.0)).flex_col().items_center()
                }
            });
        el.style().refine(&self.style);
        provide(context, el.children(children))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::taper::linear_taper;

    const FRAME_MS: f64 = 16.0;

    fn scale() -> MeterScale {
        MeterScale {
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_MAX_DB,
            taper: resolve_taper(&TaperInput::Linear, DEFAULT_MIN_DB, DEFAULT_MAX_DB),
            zones: DEFAULT_ZONES.to_vec(),
        }
    }

    fn mono(peak_db: f32) -> MeterFrame {
        MeterFrame::mono(ChannelLevel::new(peak_db))
    }

    /// Drives the painter like the host: one paint per frame while it asks for
    /// frames, for `ms`. Returns the time reached and whether it still wants frames.
    fn advance(
        painter: &mut MeterPainter,
        task: &mut FrameTask,
        frame: &MeterFrame,
        from_ms: f64,
        ms: f64,
    ) -> (f64, bool) {
        let mut now = from_ms;
        let end = from_ms + ms;
        let mut requested = task.is_awake();
        while now < end && requested {
            now += FRAME_MS;
            requested = task.run(now, |t| painter.paint(t, Some(frame), &[0]));
        }
        (end.max(now), requested)
    }

    fn level(painter: &MeterPainter) -> f32 {
        painter.reading(0).unwrap().level
    }

    #[test]
    fn paints_declarative_values_and_reports_them() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        advance(&mut painter, &mut task, &mono(-6.0), 0.0, 400.0);
        assert!((level(&painter) - 0.9).abs() < 0.005);
        assert_eq!(painter.reading(0).unwrap().zone, MeterZoneName::Clip);
        assert_eq!(painter.root().value_now, Some(-6.0));
        assert_eq!(painter.root().value_text.as_deref(), Some("−6.0 dB"));
    }

    #[test]
    fn follows_frames_per_channel() {
        let mut painter = MeterPainter::new(None, false, scale());
        let frame = MeterFrame::stereo(ChannelLevel::new(-12.0), ChannelLevel::new(-24.0));
        let mut now = 0.0;
        for _ in 0..25 {
            now += FRAME_MS;
            painter.paint(now, Some(&frame), &[0, 1]);
        }
        assert!((painter.reading(1).unwrap().level - 0.6).abs() < 0.005);
        assert!((painter.reading(0).unwrap().level - 0.8).abs() < 0.005);
    }

    #[test]
    fn marks_clipping() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        advance(&mut painter, &mut task, &mono(0.0), 0.0, 100.0);
        assert!(painter.root().clipping);
    }

    #[test]
    fn sleeps_once_settled_and_a_new_value_wakes_it() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        let (now, requested) = advance(&mut painter, &mut task, &mono(-12.0), 0.0, 100.0);
        assert!((level(&painter) - 0.8).abs() < 0.001);
        assert!(!requested, "a settled meter requests no frames");

        task.wake();
        let (_, requested) = advance(&mut painter, &mut task, &mono(-40.0), now, 10_000.0);
        assert!((level(&painter) - 1.0 / 3.0).abs() < 0.01);
        assert_eq!(painter.root().value_text.as_deref(), Some("−40.0 dB"));
        assert!(!requested);
    }

    #[test]
    fn falls_smoothly_after_a_long_sleep_instead_of_jumping() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        advance(&mut painter, &mut task, &mono(-12.0), 0.0, 10_000.0);
        // Asleep for a minute, then a new value.
        task.wake();
        advance(&mut painter, &mut task, &mono(-40.0), 70_000.0, 20.0);
        assert!(level(&painter) > 0.6);
        assert!(level(&painter) < 0.8);
    }

    #[test]
    fn holds_a_steady_clip_without_frames_and_counts_the_light_down_after() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        let (now, requested) = advance(&mut painter, &mut task, &mono(0.0), 0.0, 100.0);
        assert!(painter.root().clipping);
        assert!(!requested);

        task.wake();
        let (now, _) = advance(&mut painter, &mut task, &mono(-30.0), now, 1000.0);
        assert!(painter.root().clipping);
        let (_, requested) = advance(&mut painter, &mut task, &mono(-30.0), now, 10_000.0);
        assert!(!painter.root().clipping);
        assert!(!requested);
    }

    #[test]
    fn a_rebuilt_painter_starts_without_a_clip() {
        let mut painter = MeterPainter::new(None, false, scale());
        let mut task = create_frame_task();
        advance(&mut painter, &mut task, &mono(0.0), 0.0, 50.0);
        assert!(painter.root().clipping);
        let mut painter = MeterPainter::new(Some(&BallisticsPreset::Vu.into()), false, scale());
        let mut task = create_frame_task();
        advance(&mut painter, &mut task, &mono(-20.0), 0.0, 2000.0);
        assert!(!painter.root().clipping);
    }

    #[test]
    fn reduced_motion_jumps_and_throttles() {
        let mut painter = MeterPainter::new(None, true, scale());
        painter.paint(0.0, Some(&mono(-30.0)), &[0]);
        assert!((level(&painter) - 0.5).abs() < 1e-4);
        // Within 250 ms: no repaint.
        painter.paint(100.0, Some(&mono(-12.0)), &[0]);
        assert!((level(&painter) - 0.5).abs() < 1e-4);
        painter.paint(200.0, Some(&mono(-12.0)), &[0]);
        painter.paint(300.0, Some(&mono(-12.0)), &[0]);
        assert!((level(&painter) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn rms_follows_its_own_input() {
        let mut painter =
            MeterPainter::new(Some(&BallisticsPreset::Instant.into()), false, scale());
        painter.paint(
            0.0,
            Some(&MeterFrame::mono(ChannelLevel::with_rms(-6.0, -30.0))),
            &[0],
        );
        let reading = painter.reading(0).unwrap();
        assert!((reading.level - 0.9).abs() < 1e-4);
        assert!((reading.rms - 0.5).abs() < 1e-4);
        assert!(reading.active);
    }

    #[test]
    fn declared_levels() {
        assert_eq!(declared_frame(None, None, None), None);
        assert_eq!(declared_frame(None, Some(-12.0), None), Some(mono(-12.0)));
        assert_eq!(
            declared_frame(None, None, Some(-20.0)),
            Some(MeterFrame::mono(ChannelLevel {
                peak_db: SILENCE_DB,
                rms_db: Some(-20.0)
            }))
        );
        let channels = [ChannelLevel::new(-12.0), ChannelLevel::new(-24.0)];
        assert_eq!(
            declared_frame(Some(&channels), Some(-1.0), None)
                .unwrap()
                .channels
                .len(),
            2
        );
    }

    #[test]
    fn builds_the_default_zone_fill() {
        let taper = linear_taper(-60.0, 0.0);
        let solid = build_zone_fill(&DEFAULT_ZONES, &taper, LevelMeterVariant::Solid);
        let ends: Vec<(f32, f32, MeterZoneName)> =
            solid.iter().map(|p| (p.start, p.end, p.from)).collect();
        assert_eq!(ends.len(), 3);
        assert_eq!(ends[0].2, MeterZoneName::Ok);
        assert!((ends[0].1 - 0.66667).abs() < 1e-5);
        assert!((ends[1].1 - 0.85).abs() < 1e-5);
        assert_eq!(ends[2], (0.85, 1.0, MeterZoneName::Clip));
        assert!(solid.iter().all(|p| p.from == p.to));

        let gradient = build_zone_fill(&DEFAULT_ZONES, &taper, LevelMeterVariant::Gradient);
        assert_eq!(gradient[0].to, MeterZoneName::Warn);
        assert_eq!(gradient[1].to, MeterZoneName::Clip);
        assert_eq!(gradient[2].from, MeterZoneName::Clip);
        assert_eq!(gradient[2].to, MeterZoneName::Clip);
    }

    #[test]
    fn sorts_zones_and_fills_before_the_first() {
        let taper = linear_taper(-60.0, 0.0);
        let zones = [
            MeterZone {
                from_db: -3.0,
                zone: MeterZoneName::Clip,
            },
            MeterZone {
                from_db: -30.0,
                zone: MeterZoneName::Warn,
            },
        ];
        let pieces = build_zone_fill(&zones, &taper, LevelMeterVariant::Solid);
        assert_eq!(pieces[0].from, MeterZoneName::Warn);
        assert_eq!(pieces[0].start, 0.0);
        assert_eq!(pieces[1].from, MeterZoneName::Warn);
        assert_eq!(pieces[2].from, MeterZoneName::Clip);
    }

    #[test]
    fn segment_spans_leave_a_gap() {
        let spans = segment_spans(240.0, Some(24));
        assert_eq!(spans.len(), 24);
        assert_eq!(spans[0], (0.0, 8.0));
        assert_eq!(spans[1], (10.0, 18.0));
        assert_eq!(segment_spans(100.0, None), [(0.0, 100.0)]);
    }
}
