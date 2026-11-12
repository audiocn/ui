//! Port of `components/ui/lcd-meter.tsx`.
//!
//! A level meter drawn on an [`LcdDisplay`](super::LcdDisplay)'s screen: block
//! segments in the display's ink, a held peak, readouts, a printed scale and a
//! CLIP flag. Metering is [`LevelMeter`]'s (the same sources, ballistics, peak
//! hold and clip hold); the size and colours come from the display.
//!
//! React lays the meter out as a three-column CSS grid (labels, tracks,
//! readouts). GPUI has no `auto minmax(0,1fr) auto` grid, so the meter
//! measures its labels and readouts and gives every row the same column
//! widths, which is what the grid computes. The `--lcd-meter-*` variables are
//! builder options on [`LcdMeter`].

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder, px, size,
};

use super::db_readout::{DbReadoutChannel, DbReadoutMeasure};
use super::db_scale::{DbScaleLabelSpan, thin_db_scale_labels};
use super::lcd_display::{
    LcdDisplayContext, LcdDisplaySize, LcdText, lcd_text_width, paint_lcd_text, paint_lit_quad,
    paint_mask_holes, use_lcd_display,
};
use super::level_meter::{
    LevelMeter, LevelMeterActions, LevelMeterChannel, LevelMeterContext, LevelMeterMeasure,
    LevelMeterValue, use_level_meter,
};
use crate::audio::ballistics::BallisticsInput;
use crate::audio::decibels::{FormatDbOptions, FormatDbSign, SILENCE_DB, format_db};
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::taper::TaperInput;
use crate::audio::types::{ChannelLevel, MeterFrame, MeterZone, MeterZoneName, Taper};
use crate::audio::zones::{CLIP_HOLD_MS, CLIP_THRESHOLD_DB};
use crate::components::clip_indicator::loudest_peak;
use crate::config::{AudioConfig, AudioSize};
use crate::signal::clip_hold::{ClipHold, ClipHoldOptions};
use crate::theme::ActiveTheme;
use crate::ui::styles::{radius_sm, ring_overlay};

/// The marks a recorder screen prints, kept to the meter's range.
const DEFAULT_TICKS: [f32; 8] = [6.0, 3.0, 0.0, -3.0, -6.0, -12.0, -24.0, -48.0];
/// Off the dot grid by less than this, in pixels, counts as on it.
const GRID_EPSILON: f32 = 0.01;
/// Two-digit levels, the widest a readout usually shows (`DbReadout`).
const WIDEST_MAGNITUDE_DB: f32 = 88.8;

/// The tick values the scale prints by default: 6, 3, 0, −3, −6, −12, −24 and
/// −48, inside the range.
pub fn lcd_meter_default_ticks(min_db: f32, max_db: f32) -> Vec<f32> {
    DEFAULT_TICKS
        .iter()
        .copied()
        .filter(|tick| *tick >= min_db && *tick <= max_db)
        .collect()
}

/// L and R for a pair, numbers for more, nothing for one.
pub fn lcd_meter_channel_label(index: usize, count: usize) -> Option<SharedString> {
    match count {
        2 => Some(if index == 0 { "L" } else { "R" }.into()),
        count if count > 2 => Some((index + 1).to_string().into()),
        _ => None,
    }
}

/// How far short of the next dot a distance ends, or 0 on a dot.
fn to_next_dot(distance: f32, pitch: f32) -> f32 {
    if pitch <= 0.0 {
        return 0.0;
    }
    let past = distance.rem_euclid(pitch);
    if past < GRID_EPSILON || pitch - past < GRID_EPSILON {
        0.0
    } else {
        pitch - past
    }
}

/// Magnitudes, as recorder screens print them; above 0 keeps its plus.
pub fn lcd_meter_print_magnitude(db: f32) -> SharedString {
    format_db(
        db,
        FormatDbOptions {
            decimals: 0,
            sign: if db > 0.0 {
                FormatDbSign::Auto
            } else {
                FormatDbSign::Never
            },
            unit: false,
            ..Default::default()
        },
    )
    .into()
}

/// Where the clip zone starts, if it starts inside the range.
fn clip_from_db(zones: &[MeterZone], max_db: f32) -> Option<f32> {
    let from = zones
        .iter()
        .filter(|zone| zone.zone == MeterZoneName::Clip)
        .map(|zone| zone.from_db)
        .fold(f32::INFINITY, f32::min);
    (from < max_db).then_some(from)
}

/// The scale's bracket start (`--lcd-meter-bracket-start`), as a position.
fn bracket_start(taper: &dyn Taper, zones: &[MeterZone], max_db: f32) -> Option<f32> {
    clip_from_db(zones, max_db).map(|from| taper.to_position(from))
}

/// The segments of a track: whole segments only, so the last is never cut.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SegmentLayout {
    /// `--lcd-meter-segment`.
    segment: f32,
    /// A segment and the gap after it (`--lcd-meter-period`).
    period: f32,
    /// The track's width after the cut (`round(down, 100% - nudge + gap, period) - gap`).
    width: f32,
}

impl SegmentLayout {
    fn new(available: f32, nudge: f32, segment: f32, gap: f32) -> Self {
        let period = (segment + gap).max(0.001);
        let width = ((available - nudge + gap) / period).floor() * period - gap;
        Self {
            segment,
            period,
            width: width.max(0.0),
        }
    }

    /// Segments on the track.
    fn count(&self) -> usize {
        ((self.width + self.period - self.segment) / self.period)
            .round()
            .max(0.0) as usize
    }

    /// Lit segments for a level: a segment lights whole or not at all
    /// (`round(up, level * 100%, period)`).
    fn lit(&self, level: f32) -> usize {
        let edge = level.clamp(0.0, 1.0) * self.width;
        (((edge / self.period) - 1e-4).ceil().max(0.0) as usize).min(self.count())
    }

    /// The held-peak segment, hidden at silence.
    fn hold(&self, hold: f32) -> Option<(usize, f32)> {
        let opacity = (hold * 1000.0).clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return None;
        }
        let index = self.lit(hold).max(1) - 1;
        Some((index, opacity))
    }

    /// The left edge of a segment, from the track's start.
    fn left(&self, index: usize) -> f32 {
        index as f32 * self.period
    }
}

/// Characters of the widest text a readout can show (`DbReadout`'s sizing).
fn readout_widest_chars(decimals: usize, unit: bool, floor_db: f32) -> usize {
    let render = |db: f32| {
        format_db(
            db,
            FormatDbOptions {
                decimals,
                unit,
                floor_db,
                ..Default::default()
            },
        )
        .chars()
        .count()
    };
    let just_above_floor = if floor_db.is_finite() {
        floor_db + 10f32.powi(-(decimals as i32))
    } else {
        -WIDEST_MAGNITUDE_DB
    };
    [
        SILENCE_DB,
        just_above_floor,
        -WIDEST_MAGNITUDE_DB,
        WIDEST_MAGNITUDE_DB,
    ]
    .into_iter()
    .map(render)
    .max()
    .unwrap_or(0)
}

/// `round(up, value, step)`.
fn round_up(value: f32, step: f32) -> f32 {
    if step <= 0.0 {
        value
    } else {
        (value / step).ceil() * step
    }
}

fn audio_size(size: LcdDisplaySize) -> AudioSize {
    match size {
        LcdDisplaySize::Sm => AudioSize::Sm,
        LcdDisplaySize::Default => AudioSize::Default,
        LcdDisplaySize::Lg => AudioSize::Lg,
    }
}

/// `--lcd-meter-thickness`: 5, 7 or 10 dots by the display's size.
pub fn lcd_meter_thickness(size: LcdDisplaySize, pitch: Pixels) -> Pixels {
    pitch
        * match size {
            LcdDisplaySize::Sm => 5.0,
            LcdDisplaySize::Default => 7.0,
            LcdDisplaySize::Lg => 10.0,
        }
}

fn require_display(display: Option<LcdDisplayContext>) -> LcdDisplayContext {
    display.unwrap_or_else(|| panic!("LcdMeter must be used inside LcdDisplay."))
}

// ---------------------------------------------------------------------------
// Variables

/// The `--lcd-meter-*` variables, resolved against the display.
#[derive(Clone, Debug, PartialEq)]
struct MeterVars {
    gap: f32,
    segment: f32,
    unlit: Hsla,
    thickness: Pixels,
}

/// What every part of one meter shares: the display, the meter's variables,
/// its column widths and the dot-grid nudges.
#[derive(Clone)]
struct MeterLayout {
    display: LcdDisplayContext,
    vars: MeterVars,
    label_width: Pixels,
    value_width: Pixels,
    nudge_y: Rc<Cell<f32>>,
}

impl MeterLayout {
    fn pitch(&self) -> f32 {
        f32::from(self.display.palette.pitch)
    }

    /// The screen's dot pitch when its mask is on, else 0 (no grid to meet).
    fn grid_pitch(&self) -> f32 {
        if self.display.matrix {
            self.pitch()
        } else {
            0.0
        }
    }

    /// The x nudge that puts a column starting at `left` on the dot grid.
    fn nudge_x(&self, left: Pixels) -> f32 {
        match self.display.grid.origin() {
            Some(origin) => to_next_dot(f32::from(left - origin.x), self.grid_pitch()),
            None => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Parts

/// A part of an [`LcdMeterChannel`]: its label, track or readout.
pub enum LcdMeterChannelPart {
    Label(LcdMeterLabel),
    Track(LcdMeterTrack),
    Value(LcdMeterValue),
}

impl From<LcdMeterLabel> for LcdMeterChannelPart {
    fn from(part: LcdMeterLabel) -> Self {
        Self::Label(part)
    }
}

impl From<LcdMeterTrack> for LcdMeterChannelPart {
    fn from(part: LcdMeterTrack) -> Self {
        Self::Track(part)
    }
}

impl From<LcdMeterValue> for LcdMeterChannelPart {
    fn from(part: LcdMeterValue) -> Self {
        Self::Value(part)
    }
}

/// A part of an [`LcdMeter`]: a channel row, the scale or the CLIP flag.
pub enum LcdMeterPart {
    Channel(Box<LcdMeterChannel>),
    Scale(LcdMeterScale),
    Clip(LcdMeterClip),
}

impl From<LcdMeterChannel> for LcdMeterPart {
    fn from(part: LcdMeterChannel) -> Self {
        Self::Channel(Box::new(part))
    }
}

impl From<LcdMeterScale> for LcdMeterPart {
    fn from(part: LcdMeterScale) -> Self {
        Self::Scale(part)
    }
}

impl From<LcdMeterClip> for LcdMeterPart {
    fn from(part: LcdMeterClip) -> Self {
        Self::Clip(part)
    }
}

/// One row of the meter: a label, a track and a readout for one channel.
///
/// ```ignore
/// LcdMeterChannel::new()
///     .child(LcdMeterLabel::new("Mic"))
///     .child(LcdMeterTrack::new().child(LcdMeterBar::new()).child(LcdMeterHold::new()))
///     .child(LcdMeterValue::new())
/// ```
#[derive(Default)]
pub struct LcdMeterChannel {
    index: usize,
    label: Option<LcdMeterLabel>,
    track: Option<LcdMeterTrack>,
    value: Option<LcdMeterValue>,
    style: StyleRefinement,
}

impl LcdMeterChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Which channel of the frames this row shows. Default 0.
    pub fn index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    pub fn child(mut self, part: impl Into<LcdMeterChannelPart>) -> Self {
        match part.into() {
            LcdMeterChannelPart::Label(label) => self.label = Some(label),
            LcdMeterChannelPart::Track(track) => self.track = Some(track),
            LcdMeterChannelPart::Value(value) => self.value = Some(value),
        }
        self
    }
}

impl Styled for LcdMeterChannel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Dim text naming a channel, in a column a whole number of dots wide.
pub struct LcdMeterLabel {
    text: SharedString,
    style: StyleRefinement,
}

impl LcdMeterLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for LcdMeterLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A part of an [`LcdMeterTrack`]: a bar or the held peak.
pub enum LcdMeterTrackPart {
    Bar(LcdMeterBar),
    Hold(LcdMeterHold),
}

impl From<LcdMeterBar> for LcdMeterTrackPart {
    fn from(part: LcdMeterBar) -> Self {
        Self::Bar(part)
    }
}

impl From<LcdMeterHold> for LcdMeterTrackPart {
    fn from(part: LcdMeterHold) -> Self {
        Self::Hold(part)
    }
}

/// The unlit segments, which the bar and the held peak light.
#[derive(Default)]
pub struct LcdMeterTrack {
    parts: Vec<LcdMeterTrackPart>,
    style: StyleRefinement,
}

impl LcdMeterTrack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, part: impl Into<LcdMeterTrackPart>) -> Self {
        self.parts.push(part.into());
        self
    }
}

impl Styled for LcdMeterTrack {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The lit segments. A segment lights whole or not at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LcdMeterBar {
    measure: LevelMeterMeasure,
}

impl LcdMeterBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Which measurement this bar shows. Layer a `Rms` and a `Peak` bar for a
    /// dual meter. Default `Peak`.
    pub fn measure(mut self, measure: LevelMeterMeasure) -> Self {
        self.measure = measure;
        self
    }
}

/// One lit segment at the held peak, hidden at silence.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LcdMeterHold;

impl LcdMeterHold {
    pub fn new() -> Self {
        Self
    }
}

/// The level in bright digits, without a unit. Inside a channel it shows that
/// channel, elsewhere the loudest one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LcdMeterValue {
    channel: Option<DbReadoutChannel>,
    measure: DbReadoutMeasure,
    unit: bool,
    decimals: Option<usize>,
    interval_ms: Option<f64>,
    hold_ms: Option<f64>,
}

impl LcdMeterValue {
    pub fn new() -> Self {
        Self::default()
    }

    /// A channel index, or `Max` for the loudest. Default: the row's channel.
    pub fn channel(mut self, channel: impl Into<DbReadoutChannel>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Default `Peak`.
    pub fn measure(mut self, measure: DbReadoutMeasure) -> Self {
        self.measure = measure;
        self
    }

    /// Append " dB". Default false.
    pub fn unit(mut self, unit: bool) -> Self {
        self.unit = unit;
        self
    }

    /// Digits after the decimal point. Default 1.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// How often the text changes. Default 250 ms.
    pub fn interval_ms(mut self, interval_ms: f64) -> Self {
        self.interval_ms = Some(interval_ms);
        self
    }

    /// Show the highest value seen within this window. Default 0.
    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.hold_ms = Some(hold_ms);
        self
    }
}

type ScaleFormat = Rc<dyn Fn(f32) -> SharedString>;

/// Dim numbers under the tracks, and a bracket over the clip zone.
pub struct LcdMeterScale {
    ticks: Option<Vec<f32>>,
    format: ScaleFormat,
    bracket: bool,
    style: StyleRefinement,
}

impl Default for LcdMeterScale {
    fn default() -> Self {
        Self {
            ticks: None,
            format: Rc::new(lcd_meter_print_magnitude),
            bracket: true,
            style: StyleRefinement::default(),
        }
    }
}

impl LcdMeterScale {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tick values. Default: 48, 24, 12, 6, 3 and 0 below, inside the range.
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = Some(ticks.into_iter().collect());
        self
    }

    /// Text for each tick. Default: the magnitude, with a plus above 0.
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Print a bracket over the clip zone. Default true.
    pub fn bracket(mut self, bracket: bool) -> Self {
        self.bracket = bracket;
        self
    }
}

impl Styled for LcdMeterScale {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

type ClippingHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A CLIP flag: an unlit ghost that inverts while the clip hold is on. Click
/// (or Enter / Space) resets it.
pub struct LcdMeterClip {
    text: Option<SharedString>,
    clipping: Option<bool>,
    threshold_db: f32,
    hold_ms: f64,
    on_clipping_change: Option<ClippingHandler>,
    style: StyleRefinement,
}

impl Default for LcdMeterClip {
    fn default() -> Self {
        Self {
            text: None,
            clipping: None,
            threshold_db: CLIP_THRESHOLD_DB,
            hold_ms: CLIP_HOLD_MS,
            on_clipping_change: None,
            style: StyleRefinement::default(),
        }
    }
}

impl LcdMeterClip {
    pub fn new() -> Self {
        Self::default()
    }

    /// The flag's text. Default "Clip".
    pub fn child(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Controlled clip state: the flag shows this instead of what it detected.
    pub fn clipping(mut self, clipping: bool) -> Self {
        self.clipping = Some(clipping);
        self
    }

    /// Levels at or above this count as a clip. Default −1 dBFS.
    pub fn threshold_db(mut self, threshold_db: f32) -> Self {
        self.threshold_db = threshold_db;
        self
    }

    /// How long the flag stays on. `f64::INFINITY` latches until reset. Default 1500 ms.
    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.hold_ms = hold_ms;
        self
    }

    pub fn on_clipping_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clipping_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for LcdMeterClip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

// ---------------------------------------------------------------------------
// LcdMeter

/// A level meter drawn on an `LcdDisplay`'s screen: block segments in the
/// display's ink, a held peak, readouts, a scale and a CLIP flag. Metering is
/// `LevelMeter`'s; the size and colours come from the display. The `Audio`
/// taper gives the loud end of the range most of the track, like a recorder.
/// Panics outside an `LcdDisplay`.
///
/// ```ignore
/// LcdDisplay::new("input").child(
///     LcdMeter::new("input-level").aria_label("Input level").source(signal.meter.clone()),
/// )
/// ```
#[derive(IntoElement)]
pub struct LcdMeter {
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
    config: AudioConfig,
    actions: Option<LevelMeterActions>,
    aria_label: Option<SharedString>,
    gap: Option<Pixels>,
    segment: Option<Pixels>,
    unlit: Option<Hsla>,
    thickness: Option<Pixels>,
    parts: Vec<LcdMeterPart>,
    style: StyleRefinement,
}

impl LcdMeter {
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
            taper: TaperInput::Audio,
            config: AudioConfig::default(),
            actions: None,
            aria_label: None,
            gap: None,
            segment: None,
            unlit: None,
            thickness: None,
            parts: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// A meter source; the meter reads it every frame it paints.
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

    /// Rows to render before data arrives. Default 1.
    pub fn channel_count(mut self, channel_count: usize) -> Self {
        self.channel_count = Some(channel_count);
        self
    }

    /// Bottom of the range. Default −60.
    pub fn min_db(mut self, min_db: f32) -> Self {
        self.min_db = Some(min_db);
        self
    }

    /// Top of the range. Default 0.
    pub fn max_db(mut self, max_db: f32) -> Self {
        self.max_db = Some(max_db);
        self
    }

    /// Zones; the scale's bracket marks the clip zone. Default ok / warn from
    /// −20 / clip from −9.
    pub fn zones(mut self, zones: impl Into<Vec<MeterZone>>) -> Self {
        self.zones = Some(zones.into());
        self
    }

    /// How the meter moves. Default `peak`.
    pub fn ballistics(mut self, ballistics: impl Into<BallisticsInput>) -> Self {
        self.ballistics = Some(ballistics.into());
        self
    }

    /// Scale law. Default `Audio`.
    pub fn taper(mut self, taper: impl Into<TaperInput>) -> Self {
        self.taper = taper.into();
        self
    }

    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    pub fn actions(mut self, actions: LevelMeterActions) -> Self {
        self.actions = Some(actions);
        self
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// `--lcd-meter-gap`: between segments. Default one dot (the display's pitch).
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = Some(gap);
        self
    }

    /// `--lcd-meter-segment`: a segment's width. Default three dots.
    pub fn segment(mut self, segment: Pixels) -> Self {
        self.segment = Some(segment);
        self
    }

    /// `--lcd-meter-unlit`: unlit segments and the CLIP ghost. Default: lit at 14%.
    pub fn unlit(mut self, color: impl Into<Hsla>) -> Self {
        self.unlit = Some(color.into());
        self
    }

    /// `--lcd-meter-thickness`. Default 5, 7 or 10 dots by the display's size.
    pub fn thickness(mut self, thickness: Pixels) -> Self {
        self.thickness = Some(thickness);
        self
    }

    /// Custom rows instead of the default ones.
    pub fn child(mut self, part: impl Into<LcdMeterPart>) -> Self {
        self.parts.push(part.into());
        self
    }

    pub fn children<P: Into<LcdMeterPart>>(mut self, parts: impl IntoIterator<Item = P>) -> Self {
        self.parts.extend(parts.into_iter().map(Into::into));
        self
    }
}

impl Styled for LcdMeter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct LcdMeterState {
    nudge_y: Rc<Cell<f32>>,
}

impl RenderOnce for LcdMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let display = require_display(use_lcd_display());
        let pitch = display.palette.pitch;
        let vars = MeterVars {
            gap: f32::from(self.gap.unwrap_or(pitch)),
            segment: f32::from(self.segment.unwrap_or(pitch * 3.0)),
            unlit: self
                .unlit
                .unwrap_or_else(|| display.palette.lit.opacity(0.14)),
            thickness: self
                .thickness
                .unwrap_or_else(|| lcd_meter_thickness(display.size, pitch)),
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| LcdMeterState {
            nudge_y: Rc::new(Cell::new(0.0)),
        });
        let nudge_y = state.read(cx).nudge_y.clone();
        let layout_seed = (display.clone(), vars, nudge_y.clone());

        let mut meter = LevelMeter::new(self.id.clone())
            .taper(self.taper)
            .orientation(crate::audio::types::Orientation::Horizontal)
            .size(audio_size(display.size))
            .config(self.config);
        if let Some(source) = self.source {
            meter = meter.source(source);
        }
        if let Some(peak_db) = self.peak_db {
            meter = meter.peak_db(peak_db);
        }
        if let Some(rms_db) = self.rms_db {
            meter = meter.rms_db(rms_db);
        }
        if let Some(channels) = self.channels {
            meter = meter.channels(channels);
        }
        if let Some(count) = self.channel_count {
            meter = meter.channel_count(count);
        }
        if let Some(min_db) = self.min_db {
            meter = meter.min_db(min_db);
        }
        if let Some(max_db) = self.max_db {
            meter = meter.max_db(max_db);
        }
        if let Some(zones) = self.zones {
            meter = meter.zones(zones);
        }
        if let Some(ballistics) = self.ballistics {
            meter = meter.ballistics(ballistics);
        }
        if let Some(actions) = self.actions {
            meter = meter.actions(actions);
        }
        if let Some(label) = self.aria_label {
            meter = meter.aria_label(label);
        }
        let parts = (!self.parts.is_empty()).then_some(self.parts);
        let mut meter = meter
            .relative()
            .flex_col()
            .items_stretch()
            .gap_0()
            // pt-[calc(0.75rem + nudge-y)] pb-3
            .pt(px(12.0 + nudge_y.get()))
            .pb(px(12.0))
            .child(LcdMeterBody {
                id: self.id,
                parts,
                display: layout_seed.0,
                vars: layout_seed.1,
                nudge_y: layout_seed.2,
            });
        meter.style().refine(&self.style);
        meter
    }
}

/// The rows, rendered inside the `LevelMeter` so they can read it.
#[derive(IntoElement)]
struct LcdMeterBody {
    id: ElementId,
    parts: Option<Vec<LcdMeterPart>>,
    display: LcdDisplayContext,
    vars: MeterVars,
    nudge_y: Rc<Cell<f32>>,
}

/// The default rows: one per channel, then the scale and the CLIP flag.
fn default_parts(channel_count: usize) -> Vec<LcdMeterPart> {
    let mut parts: Vec<LcdMeterPart> = (0..channel_count)
        .map(|index| {
            let mut channel = LcdMeterChannel::new().index(index);
            if let Some(label) = lcd_meter_channel_label(index, channel_count) {
                channel = channel.child(LcdMeterLabel::new(label));
            }
            channel
                .child(
                    LcdMeterTrack::new()
                        .child(LcdMeterBar::new())
                        .child(LcdMeterHold::new()),
                )
                .child(LcdMeterValue::new())
                .into()
        })
        .collect();
    parts.push(LcdMeterScale::new().into());
    parts.push(LcdMeterClip::new().into());
    parts
}

/// One grid row: a channel spanning every column, or what the auto-placement
/// put in the second and third columns.
#[allow(clippy::large_enum_variant)]
enum GridRow {
    Channel(Box<LcdMeterChannel>),
    Cells {
        scale: Option<LcdMeterScale>,
        clip: Option<LcdMeterClip>,
    },
}

/// CSS grid auto-placement of the parts: channels take a row each; the scale
/// (column 2) and the flag (column 3) share a row when they follow in order.
fn place(parts: Vec<LcdMeterPart>) -> Vec<GridRow> {
    let mut rows: Vec<GridRow> = Vec::new();
    // The column the placement cursor is past, in the last row.
    let mut cursor = 0;
    for part in parts {
        match part {
            LcdMeterPart::Channel(channel) => {
                rows.push(GridRow::Channel(channel));
                cursor = 3;
            }
            LcdMeterPart::Scale(scale) => match rows.last_mut() {
                Some(GridRow::Cells { scale: slot, .. }) if cursor < 2 && slot.is_none() => {
                    *slot = Some(scale);
                    cursor = 2;
                }
                _ => {
                    rows.push(GridRow::Cells {
                        scale: Some(scale),
                        clip: None,
                    });
                    cursor = 2;
                }
            },
            LcdMeterPart::Clip(clip) => match rows.last_mut() {
                Some(GridRow::Cells { clip: slot, .. }) if cursor < 3 && slot.is_none() => {
                    *slot = Some(clip);
                    cursor = 3;
                }
                _ => {
                    rows.push(GridRow::Cells {
                        scale: None,
                        clip: Some(clip),
                    });
                    cursor = 3;
                }
            },
        }
    }
    rows
}

impl RenderOnce for LcdMeterBody {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let meter = use_level_meter("LcdMeter");
        let parts = self
            .parts
            .unwrap_or_else(|| default_parts(meter.channel_count));
        let display = self.display;
        let p = &display.palette;
        let font_size = display.text_size();
        let tracking = display.tracking();
        let pitch = f32::from(p.pitch);

        // Column widths, as the grid sizes its auto columns.
        let label_min = px(round_up(f32::from(font_size) * 1.5, pitch));
        let ch = lcd_text_width("0", p, font_size, px(0.0), window);
        let clip_font = font_size * 0.75;
        let mut label_width = px(0.0);
        let mut value_width = px(0.0);
        for part in &parts {
            match part {
                LcdMeterPart::Channel(channel) => {
                    if let Some(label) = &channel.label {
                        let text = lcd_text_width(&label.text, p, font_size, tracking, window);
                        label_width = label_width.max(text.max(label_min));
                    }
                    if let Some(value) = &channel.value {
                        let decimals = value.decimals.unwrap_or(1);
                        let chars = readout_widest_chars(decimals, value.unit, meter.min_db);
                        let min = ch * (chars as f32 * 1.2);
                        let text =
                            lcd_text_width(&"8".repeat(chars), p, font_size, tracking, window);
                        value_width = value_width.max(min.max(text));
                    }
                }
                LcdMeterPart::Clip(clip) => {
                    let text = clip.text.clone().unwrap_or_else(|| "Clip".into());
                    let width = lcd_text_width(&text, p, clip_font, tracking, window);
                    value_width = value_width.max(width + px(8.0));
                }
                LcdMeterPart::Scale(_) => {}
            }
        }

        let layout = MeterLayout {
            display: display.clone(),
            vars: self.vars,
            label_width,
            value_width,
            nudge_y: self.nudge_y,
        };
        let row_gap = px(pitch * 2.0);
        let col_gap = px(pitch * 4.0);

        let mut first_track = true;
        let mut rows = div().flex().flex_col().gap(row_gap);
        for row in place(parts) {
            match row {
                GridRow::Channel(channel) => {
                    let measure = first_track && channel.track.is_some();
                    if channel.track.is_some() {
                        first_track = false;
                    }
                    rows = rows.child(render_channel(
                        &self.id, *channel, &layout, &meter, measure, col_gap, cx,
                    ));
                }
                GridRow::Cells { scale, clip } => {
                    let mut row = div().flex().items_center().gap(col_gap);
                    row = row.child(div().flex_shrink_0().w(layout.label_width));
                    row = row.child(match scale {
                        Some(scale) => render_scale(scale, &layout, &meter).into_any_element(),
                        None => div().flex_1().min_w_0().into_any_element(),
                    });
                    let cell = div()
                        .flex_shrink_0()
                        .w(layout.value_width)
                        .self_stretch()
                        .flex()
                        .justify_end()
                        .items_end();
                    row = row.child(match clip {
                        Some(clip) => cell.child(LcdMeterClipElement {
                            id: (self.id.clone(), "clip").into(),
                            clip,
                            layout: layout.clone(),
                            frames: meter.frames.clone(),
                        }),
                        None => cell,
                    });
                    rows = rows.child(row);
                }
            }
        }

        // The screen's dot mask over the whole meter.
        let grid = display.grid.clone();
        div().relative().child(rows).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| grid.paint_holes(bounds, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

fn render_channel(
    meter_id: &ElementId,
    channel: LcdMeterChannel,
    layout: &MeterLayout,
    meter: &LevelMeterContext,
    measure_nudge: bool,
    col_gap: Pixels,
    cx: &App,
) -> AnyElement {
    let display = &layout.display;
    let p = &display.palette;
    let index = channel.index;
    let mut label_cell = div().flex_shrink_0().w(layout.label_width).flex();
    if let Some(label) = channel.label {
        let mut text = div()
            .flex()
            .child(LcdText::new(label.text, display, p.ink_dim).holes(None));
        text.style().refine(&label.style);
        label_cell = label_cell.child(text);
    }
    let track: AnyElement = match channel.track {
        Some(track) => render_track(track, index, layout, meter, measure_nudge).into_any_element(),
        None => div().flex_1().min_w_0().into_any_element(),
    };
    let mut value_cell = div()
        .flex_shrink_0()
        .w(layout.value_width)
        .flex()
        .justify_end();
    if let Some(value) = channel.value {
        value_cell = value_cell.child(render_value(meter_id, value, index, layout, cx));
    }
    let mut row = LevelMeterChannel::new()
        .index(index)
        .items_center()
        .gap(col_gap)
        .child(label_cell)
        .child(track)
        .child(value_cell);
    row.style().refine(&channel.style);
    row.into_any_element()
}

fn render_value(
    meter_id: &ElementId,
    value: LcdMeterValue,
    index: usize,
    layout: &MeterLayout,
    _cx: &App,
) -> impl IntoElement {
    let display = layout.display.clone();
    let lit = display.palette.lit;
    let channel = value.channel.unwrap_or(DbReadoutChannel::Index(index));
    let id: ElementId = (
        meter_id.clone(),
        SharedString::from(format!("value-{index}")),
    )
        .into();
    let mut readout = LevelMeterValue::new()
        .id(id)
        .channel(channel)
        .measure(value.measure)
        .unit(value.unit)
        .decorate(move |shown, _| {
            div()
                .flex()
                .justify_end()
                .child(LcdText::new(shown.text.clone(), &display, lit).holes(None))
                .into_any_element()
        });
    if let Some(decimals) = value.decimals {
        readout = readout.decimals(decimals);
    }
    if let Some(interval) = value.interval_ms {
        readout = readout.interval_ms(interval);
    }
    if let Some(hold) = value.hold_ms {
        readout = readout.hold_ms(hold);
    }
    readout
}

fn render_track(
    track: LcdMeterTrack,
    index: usize,
    layout: &MeterLayout,
    meter: &LevelMeterContext,
    measure_nudge: bool,
) -> impl IntoElement {
    let reading = meter.reading(index);
    let layout = layout.clone();
    let thickness = layout.vars.thickness;
    let bars: Vec<f32> = track
        .parts
        .iter()
        .filter_map(|part| match part {
            LcdMeterTrackPart::Bar(bar) => Some(match bar.measure {
                LevelMeterMeasure::Rms => reading.rms,
                LevelMeterMeasure::Peak => reading.level,
            }),
            LcdMeterTrackPart::Hold(_) => None,
        })
        .collect();
    let holds = track
        .parts
        .iter()
        .any(|part| matches!(part, LcdMeterTrackPart::Hold(_)));
    let hold = reading.hold;
    let prepaint_layout = layout.clone();
    let mut el = div().flex_1().min_w_0().h(thickness).child(
        canvas(
            move |bounds, window, _| {
                // `useDotGrid`: the first track's distance from the grid sets
                // the meter's top padding, so its segments start on a dot.
                if !measure_nudge {
                    return;
                }
                let layout = &prepaint_layout;
                let Some(origin) = layout.display.grid.origin() else {
                    return;
                };
                let current = layout.nudge_y.get();
                let next = to_next_dot(
                    f32::from(bounds.origin.y - origin.y) - current,
                    layout.grid_pitch(),
                );
                if (next - current).abs() > GRID_EPSILON {
                    layout.nudge_y.set(next);
                    window.request_animation_frame();
                }
            },
            move |bounds, _, window, _| {
                paint_track(bounds, &layout, &bars, holds.then_some(hold), window);
            },
        )
        .size_full(),
    );
    el.style().refine(&track.style);
    el
}

fn paint_track(
    bounds: Bounds<Pixels>,
    layout: &MeterLayout,
    bars: &[f32],
    hold: Option<f32>,
    window: &mut Window,
) {
    let p = &layout.display.palette;
    let vars = &layout.vars;
    let nudge = layout.nudge_x(bounds.origin.x);
    let segments = SegmentLayout::new(f32::from(bounds.size.width), nudge, vars.segment, vars.gap);
    let left = f32::from(bounds.origin.x) + nudge;
    let top = bounds.origin.y;
    let height = bounds.size.height;
    let segment_bounds = |from: usize, to: usize| {
        Bounds::new(
            point(px(left + segments.left(from)), top),
            size(
                px(segments.left(to) - segments.left(from) + vars.segment),
                height,
            ),
        )
    };
    let count = segments.count();
    let solid = vars.gap <= 0.0;
    // The unlit segments.
    if solid {
        if count > 0 {
            window.paint_quad(gpui::fill(segment_bounds(0, count - 1), vars.unlit));
        }
    } else {
        for i in 0..count {
            window.paint_quad(gpui::fill(segment_bounds(i, i), vars.unlit));
        }
    }
    // The bars, then the held peak, all in the lit ink.
    for &level in bars {
        let lit = segments.lit(level);
        if lit == 0 {
            continue;
        }
        if solid {
            paint_lit_quad(
                segment_bounds(0, lit - 1),
                px(0.0),
                p.lit,
                Some(p.glow),
                window,
            );
        } else {
            for i in 0..lit {
                paint_lit_quad(segment_bounds(i, i), px(0.0), p.lit, Some(p.glow), window);
            }
        }
    }
    if let Some((index, opacity)) = hold.and_then(|hold| segments.hold(hold)) {
        paint_lit_quad(
            segment_bounds(index, index),
            px(0.0),
            p.lit.opacity(opacity),
            Some(p.glow),
            window,
        );
    }
    // A screen without its own mask: the track masks its own dots.
    if !layout.display.matrix {
        let track = Bounds::new(point(px(left), top), size(px(segments.width), height));
        paint_mask_holes(track, track.origin, p.pitch, p.glass, window);
    }
}

fn render_scale(
    scale: LcdMeterScale,
    layout: &MeterLayout,
    meter: &LevelMeterContext,
) -> impl IntoElement {
    let p = &layout.display.palette;
    let pitch = f32::from(p.pitch);
    let ticks = scale
        .ticks
        .clone()
        .unwrap_or_else(|| lcd_meter_default_ticks(meter.min_db, meter.max_db));
    let start = scale
        .bracket
        .then(|| bracket_start(&meter.taper, &meter.zones, meter.max_db))
        .flatten();
    let label_size = layout.display.text_size() * 0.75;
    let label_height = round_up(f32::from(label_size), pitch);
    let bracket_height = pitch * 2.0;
    let height = if start.is_some() {
        bracket_height + pitch + label_height
    } else {
        label_height
    };
    let labels: Vec<ScaleLabel> = ticks
        .iter()
        .map(|&tick| ScaleLabel {
            position: meter.taper.to_position(tick),
            value: tick,
            text: (scale.format)(tick),
        })
        .collect();
    let layout = layout.clone();
    let mut el = div().flex_1().min_w_0().h(px(height)).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paint_scale(bounds, &layout, start, &labels, label_size, window);
            },
        )
        .size_full(),
    );
    el.style().refine(&scale.style);
    el
}

fn paint_scale(
    bounds: Bounds<Pixels>,
    layout: &MeterLayout,
    bracket: Option<f32>,
    labels: &[ScaleLabel],
    label_size: Pixels,
    window: &mut Window,
) {
    let p = &layout.display.palette;
    let pitch = f32::from(p.pitch);
    let vars = &layout.vars;
    let nudge = layout.nudge_x(bounds.origin.x);
    let segments = SegmentLayout::new(f32::from(bounds.size.width), nudge, vars.segment, vars.gap);
    let left = f32::from(bounds.origin.x) + nudge;
    let width = segments.width;
    let mut y = f32::from(bounds.origin.y);
    if let Some(start) = bracket {
        // Starts where the first segment of the zone does.
        let x0 = (start * width / segments.period).floor() * segments.period;
        let x1 = width;
        if x1 > x0 {
            let line = |x: f32, top: f32, w: f32, h: f32| {
                Bounds::new(point(px(left + x), px(top)), size(px(w), px(h)))
            };
            let glow = Some(p.glow);
            paint_lit_quad(
                line(x0, y, x1 - x0, pitch),
                px(0.0),
                p.ink_dim,
                glow,
                window,
            );
            paint_lit_quad(
                line(x0, y + pitch, pitch, pitch),
                px(0.0),
                p.ink_dim,
                glow,
                window,
            );
            paint_lit_quad(
                line(x1 - pitch, y + pitch, pitch, pitch),
                px(0.0),
                p.ink_dim,
                glow,
                window,
            );
        }
        y += pitch * 3.0;
    }
    // Labels at their positions, kept inside the scale at the ends, thinned
    // where they would touch.
    let tracking = layout.display.tracking();
    let spans: Vec<DbScaleLabelSpan> = labels
        .iter()
        .map(|label| {
            let w = f32::from(lcd_text_width(&label.text, p, label_size, tracking, window));
            let at = label.position * width;
            let start = if label.position < 0.02 {
                at
            } else if label.position > 0.98 {
                at - w
            } else {
                at - w / 2.0
            };
            DbScaleLabelSpan {
                value: label.value,
                start: left + start,
                end: left + start + w,
            }
        })
        .collect();
    let hidden = thin_db_scale_labels(&spans);
    for ((span, label), hidden) in spans.iter().zip(labels).zip(hidden) {
        if hidden {
            continue;
        }
        paint_lcd_text(
            &label.text,
            p,
            label_size,
            tracking,
            point(px(span.start), px(y)),
            p.ink_dim,
            Some(p.glow),
            window,
        );
    }
}

/// One printed tick.
struct ScaleLabel {
    position: f32,
    value: f32,
    text: SharedString,
}

// ---------------------------------------------------------------------------
// The CLIP flag

struct ClipState {
    hold: ClipHold,
    version: Option<u64>,
    focus: FocusHandle,
}

#[derive(IntoElement)]
struct LcdMeterClipElement {
    id: ElementId,
    clip: LcdMeterClip,
    layout: MeterLayout,
    frames: FrameSource<MeterFrame>,
}

impl RenderOnce for LcdMeterClipElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let options = ClipHoldOptions {
            threshold_db: self.clip.threshold_db,
            hold_ms: self.clip.hold_ms,
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| ClipState {
            hold: ClipHold::new(options),
            version: None,
            focus: cx.focus_handle().tab_stop(true),
        });
        let frames = self.frames.clone();
        let now = now_ms();
        let (detected, change, releasing, focus) = state.update(cx, |s, _| {
            if *s.hold.options() != options {
                s.hold.set_options(options, now);
            }
            let mut change = s.hold.update(now);
            let version = frames.version();
            if s.version != Some(version) {
                s.version = Some(version);
                if let Some(db) = frames.with_latest(|frame| frame.map(loudest_peak))
                    && let Some(changed) = s.hold.report(db, now)
                {
                    change = Some(changed);
                }
            }
            (
                s.hold.clipping(),
                change,
                s.hold.release_at_ms().is_some(),
                s.focus.clone(),
            )
        });
        if releasing {
            window.request_animation_frame();
        }
        if let (Some(clipping), Some(handler)) = (change, self.clip.on_clipping_change.clone()) {
            handler(clipping, window, cx);
        }
        let clipping = self.clip.clipping.unwrap_or(detected);

        let display = &self.layout.display;
        let p = &display.palette;
        let font_size = display.text_size() * 0.75;
        let color = if clipping {
            p.glass
        } else {
            self.layout.vars.unlit
        };
        let radius = radius_sm(cx.theme());
        let focus_visible = focus.is_focused(window) && window.last_input_was_keyboard();
        let lit = p.lit;
        let glow = p.glow;
        let text = self.clip.text.clone().unwrap_or_else(|| "Clip".into());

        let reset = {
            let state = state.clone();
            let handler = self.clip.on_clipping_change.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                let change = state.update(cx, |s, cx| {
                    cx.notify();
                    s.hold.reset()
                });
                if let (Some(clipping), Some(handler)) = (change, handler.as_ref()) {
                    handler(clipping, window, cx);
                }
            })
        };
        let click = reset.clone();
        let keys = reset;
        let mut el = div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .rounded(radius)
            .px(px(4.0))
            .py(px(2.0))
            .role(gpui::Role::Button)
            .aria_label(if clipping {
                "Clipping. Reset clip indicator"
            } else {
                "Clip indicator"
            })
            .track_focus(&focus)
            .when(clipping, |el| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            paint_lit_quad(bounds, radius, lit, Some(glow), window);
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
            .child(
                LcdText::new(text, display, color)
                    .font_size(font_size)
                    .holes(None)
                    .glow((clipping || color.a > 0.2).then_some(p.glow)),
            )
            .when(focus_visible, |el| el.child(ring_overlay(radius, 1.0, lit)))
            .on_click(move |_, window, cx| click(window, cx))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    keys(window, cx);
                    cx.stop_propagation();
                }
            });
        el.style().refine(&self.clip.style);
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decibels::DEFAULT_MIN_DB;
    use crate::audio::taper::resolve_taper;
    use crate::audio::zones::DEFAULT_ZONES;
    use crate::components::db_readout::read_channel;

    const MINUS: &str = "\u{2212}";

    #[test]
    #[should_panic(expected = "LcdMeter must be used inside LcdDisplay.")]
    fn lcd_meter_must_sit_on_an_lcd_display() {
        require_display(None);
    }

    #[test]
    fn lcd_meter_labels_a_pair_l_and_r_more_by_number_and_one_not_at_all() {
        let labels = |count: usize| -> Vec<Option<SharedString>> {
            (0..count)
                .map(|index| lcd_meter_channel_label(index, count))
                .collect()
        };
        assert_eq!(labels(2), vec![Some("L".into()), Some("R".into())]);
        assert_eq!(
            labels(4),
            vec![
                Some("1".into()),
                Some("2".into()),
                Some("3".into()),
                Some("4".into())
            ]
        );
        assert_eq!(labels(1), vec![None]);
        // The default rows: one track per channel, then the scale and the flag.
        assert_eq!(default_parts(1).len(), 3);
        assert!(matches!(
            &default_parts(1)[0],
            LcdMeterPart::Channel(channel) if channel.label.is_none() && channel.track.is_some()
        ));
    }

    #[test]
    fn lcd_meter_shows_each_channels_level_without_a_unit() {
        let frame = MeterFrame {
            channels: vec![ChannelLevel::new(-12.0), ChannelLevel::new(-24.0)],
        };
        let texts: Vec<String> = (0..2)
            .map(|index| {
                let db = read_channel(
                    &frame,
                    DbReadoutMeasure::Peak,
                    DbReadoutChannel::Index(index),
                );
                format_db(
                    db,
                    FormatDbOptions {
                        unit: false,
                        floor_db: DEFAULT_MIN_DB,
                        ..Default::default()
                    },
                )
            })
            .collect();
        assert_eq!(texts, vec![format!("{MINUS}12.0"), format!("{MINUS}24.0")]);
        // Five characters wide, so the column fits 6ch with the tracking.
        assert_eq!(readout_widest_chars(1, false, DEFAULT_MIN_DB), 5);
    }

    #[test]
    fn lcd_meter_paints_on_the_audio_taper() {
        // 1 − √(15 / 60): a quarter of the range in dB is half the track.
        let taper = resolve_taper(&TaperInput::Audio, -60.0, 0.0);
        assert!((taper.to_position(-15.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn lcd_meter_prints_magnitudes_on_the_scale_with_a_plus_above_0() {
        let texts = |min: f32, max: f32| -> Vec<SharedString> {
            lcd_meter_default_ticks(min, max)
                .into_iter()
                .map(lcd_meter_print_magnitude)
                .collect()
        };
        assert_eq!(texts(-60.0, 0.0), vec!["0", "3", "6", "12", "24", "48"]);
        assert_eq!(texts(-60.0, 6.0)[..3], ["+6", "+3", "0"]);
    }

    #[test]
    fn lcd_meter_starts_the_bracket_where_the_clip_zone_does() {
        let linear = resolve_taper(&TaperInput::Linear, -60.0, 0.0);
        let start = bracket_start(&linear, &DEFAULT_ZONES, 0.0).unwrap();
        assert_eq!(format!("{start:.4}"), "0.8500");
        let ok_only = [MeterZone {
            from_db: f32::NEG_INFINITY,
            zone: MeterZoneName::Ok,
        }];
        assert_eq!(bracket_start(&linear, &ok_only, 0.0), None);
    }

    #[test]
    fn lcd_meter_takes_its_size_from_the_display() {
        let pitch = px(2.0);
        assert_eq!(lcd_meter_thickness(LcdDisplaySize::Lg, pitch), px(20.0));
        assert_eq!(
            lcd_meter_thickness(LcdDisplaySize::Default, pitch),
            px(14.0)
        );
        assert_eq!(lcd_meter_thickness(LcdDisplaySize::Sm, pitch), px(10.0));
        assert_eq!(audio_size(LcdDisplaySize::Lg), AudioSize::Lg);
    }

    #[test]
    fn lcd_meter_lights_whole_segments_and_cuts_the_track_to_them() {
        // 100px, 6px segments, 2px gaps: 12 whole segments in 94px.
        let layout = SegmentLayout::new(100.0, 0.0, 6.0, 2.0);
        assert_eq!(layout.width, 94.0);
        assert_eq!(layout.count(), 12);
        assert_eq!(layout.lit(0.0), 0);
        assert_eq!(layout.lit(0.01), 1);
        assert_eq!(layout.lit(0.5), 6);
        assert_eq!(layout.lit(1.0), 12);
        // The held peak is the segment the level reaches; hidden at silence.
        assert_eq!(layout.hold(0.5), Some((5, 1.0)));
        assert_eq!(layout.hold(0.0), None);
        // A nudge onto the grid shortens the track.
        assert_eq!(SegmentLayout::new(100.0, 1.0, 6.0, 2.0).count(), 12);
        assert_eq!(SegmentLayout::new(95.0, 1.5, 6.0, 2.0).count(), 11);
    }

    #[test]
    fn lcd_meter_moves_its_tracks_onto_the_dot_grid() {
        assert_eq!(to_next_dot(4.0, 2.0), 0.0);
        assert_eq!(to_next_dot(5.0, 2.0), 1.0);
        assert!((to_next_dot(5.5, 2.0) - 0.5).abs() < 1e-6);
        assert_eq!(to_next_dot(4.005, 2.0), 0.0);
        // No grid when the screen's mask is off.
        assert_eq!(to_next_dot(5.0, 0.0), 0.0);
    }

    #[test]
    fn lcd_meter_places_the_scale_and_flag_on_one_row() {
        let rows = place(default_parts(2));
        assert_eq!(rows.len(), 3);
        assert!(matches!(
            rows[2],
            GridRow::Cells {
                scale: Some(_),
                clip: Some(_)
            }
        ));
        // Custom rows without a scale or flag render only the channel.
        let rows = place(vec![LcdMeterChannel::new().index(1).into()]);
        assert_eq!(rows.len(), 1);
    }
}
