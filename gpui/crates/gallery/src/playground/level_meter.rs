//! Playground page for `LevelMeter`: every prop, every variant, every part
//! (channels, track, peak / RMS bars, hold, scale, value, clip), zones and
//! zone colours, ballistics presets and custom times, tapers, static levels
//! and the imperative actions.
//!
//! The data / range / zones / ballistics helpers here are shared with the
//! `LcdMeter`, `DbReadout`, `DbScale` and `ClipIndicator` pages.

use std::cell::RefCell;
use std::sync::Arc;

use audiocn::audio::ballistics::{BallisticsInput, BallisticsPreset, PartialBallisticsOptions};
use audiocn::audio::taper::{Taper, TaperInput};
use audiocn::components::{
    ClipIndicatorActions, DbReadoutChannel, DbReadoutMeasure, DbScaleSide, DbScaleTick, LevelMeter,
    LevelMeterActions, LevelMeterBar, LevelMeterChannel, LevelMeterChannels, LevelMeterClip,
    LevelMeterHold, LevelMeterMeasure, LevelMeterScale, LevelMeterTrack, LevelMeterValue,
    LevelMeterVariant,
};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{AudioSize, ChannelLevel, MeterFrame, MeterZone, MeterZoneName, Orientation};
use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "level-meter",
    component: "LevelMeter",
    source: SourceUse::Meter,
    controls,
    render,
};

// ---------------------------------------------------------------------------
// Shared option lists and helpers (also used by the other meter pages)

pub(super) const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

pub(super) const ORIENTATIONS: &[(&str, &str)] = &[
    ("unset", ""),
    ("horizontal", "Orientation::Horizontal"),
    ("vertical", "Orientation::Vertical"),
];

pub(super) const TAPERS: &[(&str, &str)] = &[
    ("linear", "TaperInput::Linear"),
    ("audio", "TaperInput::Audio"),
    ("log", "TaperInput::Log"),
    (
        "custom (square root)",
        "TaperInput::custom(|db| ((db - min) / (max - min)).sqrt(), |p| min + p * p * (max - min))",
    ),
];

pub(super) const MEASURES: &[(&str, &str)] = &[
    ("peak", "DbReadoutMeasure::Peak"),
    ("rms", "DbReadoutMeasure::Rms"),
];

pub(super) const READOUT_CHANNELS: &[(&str, &str)] = &[
    ("max", "DbReadoutChannel::Max"),
    ("0", "DbReadoutChannel::Index(0)"),
    ("1", "DbReadoutChannel::Index(1)"),
    ("2", "DbReadoutChannel::Index(2)"),
];

pub(super) fn size(p: &Props, key: &str) -> Option<AudioSize> {
    match p.choice(key) {
        "sm" => Some(AudioSize::Sm),
        "default" => Some(AudioSize::Default),
        "lg" => Some(AudioSize::Lg),
        _ => None,
    }
}

pub(super) fn orientation(p: &Props, key: &str) -> Option<Orientation> {
    match p.choice(key) {
        "horizontal" => Some(Orientation::Horizontal),
        "vertical" => Some(Orientation::Vertical),
        _ => None,
    }
}

pub(super) fn measure(p: &Props, key: &str) -> DbReadoutMeasure {
    match p.choice(key) {
        "rms" => DbReadoutMeasure::Rms,
        _ => DbReadoutMeasure::Peak,
    }
}

pub(super) fn readout_channel(p: &Props, key: &str) -> DbReadoutChannel {
    match p.choice(key) {
        "0" => DbReadoutChannel::Index(0),
        "1" => DbReadoutChannel::Index(1),
        "2" => DbReadoutChannel::Index(2),
        _ => DbReadoutChannel::Max,
    }
}

/// A square-root taper over `min..max`, kept per range: custom tapers compare
/// by identity, so a fresh one every render would wake the meter forever.
struct SqrtTaper {
    min: f32,
    max: f32,
}

impl Taper for SqrtTaper {
    fn to_position(&self, value: f32) -> f32 {
        if value.is_nan() || value <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min))
            .clamp(0.0, 1.0)
            .sqrt()
    }

    fn to_value(&self, position: f32) -> f32 {
        let p = position.clamp(0.0, 1.0);
        self.min + p * p * (self.max - self.min)
    }
}

/// The cached custom taper and the range it was made for.
type CachedTaper = Option<((f32, f32), Arc<dyn Taper>)>;

thread_local! {
    static SQRT_TAPER: RefCell<CachedTaper> = const { RefCell::new(None) };
}

/// The taper picked in `key` (one of [`TAPERS`]) for the range `min..max`.
pub(super) fn taper(p: &Props, key: &str, min: f32, max: f32) -> TaperInput {
    match p.choice(key) {
        "audio" => TaperInput::Audio,
        "log" => TaperInput::Log,
        "linear" => TaperInput::Linear,
        _ => SQRT_TAPER.with(|cell| {
            let mut cell = cell.borrow_mut();
            match cell.as_ref() {
                Some((range, taper)) if *range == (min, max) => TaperInput::from(taper.clone()),
                _ => {
                    let taper: Arc<dyn Taper> = Arc::new(SqrtTaper { min, max });
                    *cell = Some(((min, max), taper.clone()));
                    TaperInput::from(taper)
                }
            }
        }),
    }
}

/// Range controls: `min_db`, `max_db` and `taper`.
pub(super) fn range_controls(taper_default: usize) -> Vec<Control> {
    vec![
        Control::num("min_db", "Min dB", -60.0, -120.0, -6.0, 1.0).group("Range"),
        Control::num("max_db", "Max dB", 0.0, -12.0, 24.0, 1.0).group("Range"),
        Control::choice("taper", "Taper", taper_default, TAPERS).group("Range"),
    ]
}

/// Zone controls: a `zones` set, the custom thresholds, and (optionally) the
/// three zone colours.
pub(super) fn zone_controls(colors: bool) -> Vec<Control> {
    let mut out = vec![
        Control::choice(
            "zones",
            "Zones",
            0,
            &[
                ("default (warn −20, clip −9)", ""),
                ("custom thresholds", "vec![MeterZone { .. }, ..]"),
                (
                    "ok only",
                    "vec![MeterZone { from_db: f32::NEG_INFINITY, zone: MeterZoneName::Ok }]",
                ),
                (
                    "broadcast (warn −18, clip −6)",
                    "vec![MeterZone { .. }, ..]",
                ),
            ],
        )
        .group("Zones"),
        Control::num("warn_db", "Warn from (custom)", -12.0, -60.0, 6.0, 1.0)
            .group("Zones")
            .no_code(),
        Control::num("clip_db", "Clip from (custom)", -3.0, -60.0, 6.0, 1.0)
            .group("Zones")
            .no_code(),
    ];
    if colors {
        out.extend([
            Control::color("ok_color", "Ok colour")
                .group("Zones")
                .no_code()
                .help("zone_color(MeterZoneName::Ok, ..)"),
            Control::color("warn_color", "Warn colour")
                .group("Zones")
                .no_code()
                .help("zone_color(MeterZoneName::Warn, ..)"),
            Control::color("clip_color", "Clip colour")
                .group("Zones")
                .no_code()
                .help("zone_color(MeterZoneName::Clip, ..)"),
        ]);
    }
    out
}

fn zone_set(warn: f32, clip: f32) -> Vec<MeterZone> {
    vec![
        MeterZone {
            from_db: f32::NEG_INFINITY,
            zone: MeterZoneName::Ok,
        },
        MeterZone {
            from_db: warn,
            zone: MeterZoneName::Warn,
        },
        MeterZone {
            from_db: clip,
            zone: MeterZoneName::Clip,
        },
    ]
}

/// The zones picked in the `zones` control; `None` keeps the default.
pub(super) fn zones(p: &Props) -> Option<Vec<MeterZone>> {
    match p.index("zones") {
        1 => Some(zone_set(p.num("warn_db"), p.num("clip_db"))),
        2 => Some(vec![MeterZone {
            from_db: f32::NEG_INFINITY,
            zone: MeterZoneName::Ok,
        }]),
        3 => Some(zone_set(-18.0, -6.0)),
        _ => None,
    }
}

/// Ballistics controls: a preset or custom attack / release / hold times.
pub(super) fn ballistics_controls() -> Vec<Control> {
    vec![
        Control::choice(
            "ballistics",
            "Ballistics",
            0,
            &[
                ("peak", "BallisticsPreset::Peak"),
                ("instant", "BallisticsPreset::Instant"),
                ("vu", "BallisticsPreset::Vu"),
                (
                    "custom",
                    "PartialBallisticsOptions { attack_ms: Some(..), release_ms: Some(..), peak_hold_ms: Some(..), peak_release_ms: Some(..) }",
                ),
            ],
        )
        .group("Ballistics"),
        Control::num("attack_ms", "Attack ms (custom)", 15.0, 0.0, 2000.0, 1.0)
            .group("Ballistics")
            .no_code(),
        Control::num("release_ms", "Release ms (custom)", 350.0, 0.0, 3000.0, 5.0)
            .group("Ballistics")
            .no_code(),
        Control::num(
            "peak_hold_ms",
            "Peak hold ms (custom, 0 = off)",
            1200.0,
            0.0,
            5000.0,
            50.0,
        )
        .group("Ballistics")
        .no_code(),
        Control::num(
            "peak_release_ms",
            "Peak release ms (custom)",
            600.0,
            0.0,
            3000.0,
            10.0,
        )
        .group("Ballistics")
        .no_code(),
    ]
}

pub(super) fn ballistics(p: &Props) -> BallisticsInput {
    match p.choice("ballistics") {
        "instant" => BallisticsPreset::Instant.into(),
        "vu" => BallisticsPreset::Vu.into(),
        "custom" => PartialBallisticsOptions {
            attack_ms: Some(p.num("attack_ms")),
            release_ms: Some(p.num("release_ms")),
            peak_hold_ms: Some(p.num("peak_hold_ms")),
            peak_release_ms: Some(p.num("peak_release_ms")),
        }
        .into(),
        _ => BallisticsPreset::Peak.into(),
    }
}

/// Data controls: the picked source, or static `peak_db` / `rms_db`, or
/// static per-channel levels, plus `channel_count`.
pub(super) fn data_controls() -> Vec<Control> {
    vec![
        Control::choice(
            "data",
            "Data",
            0,
            &[
                ("source (picker)", ""),
                ("static peak_db / rms_db", ""),
                ("static channels", ""),
            ],
        )
        .group("Data")
        .no_code()
        .help("Static levels ignore the source picker."),
        Control::num("peak_db", "Peak dB (static)", -12.0, -80.0, 6.0, 0.1).group("Data"),
        Control::num("rms_db", "RMS dB (static)", -20.0, -80.0, 6.0, 0.1).group("Data"),
        Control::num(
            "static_channels",
            "Channels (static channels)",
            2.0,
            1.0,
            8.0,
            1.0,
        )
        .group("Data")
        .no_code()
        .help("Each channel 4 dB below the previous one."),
        Control::num(
            "channel_count",
            "Channel count (before data)",
            1.0,
            1.0,
            8.0,
            1.0,
        )
        .group("Data")
        .help("Tracks to render until frames arrive (source off)."),
    ]
}

/// What the data controls asked for.
pub(super) enum Data {
    Source,
    Mono { peak: f32, rms: f32 },
    Channels(Vec<ChannelLevel>),
}

impl Data {
    pub(super) fn read(p: &Props) -> Self {
        let (peak, rms) = (p.num("peak_db"), p.num("rms_db"));
        match p.index("data") {
            1 => Data::Mono { peak, rms },
            2 => Data::Channels(
                (0..p.num("static_channels").max(1.0) as usize)
                    .map(|i| ChannelLevel::with_rms(peak - 4.0 * i as f32, rms - 4.0 * i as f32))
                    .collect(),
            ),
            _ => Data::Source,
        }
    }

    /// How many tracks the composed parts render.
    pub(super) fn tracks(&self, ctx: &Ctx, p: &Props) -> usize {
        match self {
            Data::Mono { .. } => 1,
            Data::Channels(levels) => levels.len(),
            Data::Source if ctx.meter().is_some() => ctx.channels().max(1),
            Data::Source => p.num("channel_count").max(1.0) as usize,
        }
    }
}

/// The clip hold in ms: `hold_key`, or infinity while `latch_key` is on.
pub(super) fn clip_hold(p: &Props, hold_key: &str, latch_key: &str) -> f64 {
    if p.bool(latch_key) {
        f64::INFINITY
    } else {
        p.num(hold_key) as f64
    }
}

/// `clipping(..)`: detected (None), forced on or forced off.
pub(super) const CLIP_STATES: &[(&str, &str)] = &[
    ("detected", ""),
    ("forced on", "true"),
    ("forced off", "false"),
];

pub(super) fn clip_state(p: &Props, key: &str) -> Option<bool> {
    match p.index(key) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// A small outline button for the stage's action rows.
pub(super) fn action_button(
    id: SharedString,
    label: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .variant(ButtonVariant::Outline)
        .size(ButtonSize::Sm)
        .child(label)
        .on_click(move |_, window, cx| on_click(window, cx))
}

/// A row of action buttons under the component.
pub(super) fn action_row(buttons: Vec<Button>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap_2()
        .children(buttons)
        .into_any_element()
}

/// The meter's imperative actions as buttons that log what they did.
pub(super) fn meter_action_buttons(ctx: &Ctx, actions: &LevelMeterActions) -> Vec<Button> {
    let (a1, a2, a3, a4) = (
        actions.clone(),
        actions.clone(),
        actions.clone(),
        actions.clone(),
    );
    let (c1, c2, c3, c4) = (ctx.clone(), ctx.clone(), ctx.clone(), ctx.clone());
    vec![
        action_button(ctx.id("act-paint-0"), "paint 0 dB", move |_, cx| {
            a1.paint(MeterFrame::mono(ChannelLevel::with_rms(0.0, -6.0)), cx);
            c1.log("actions.paint", "mono 0 dB / rms −6 dB", cx);
        }),
        action_button(ctx.id("act-paint-st"), "paint stereo", move |_, cx| {
            a2.paint(
                MeterFrame::stereo(
                    ChannelLevel::with_rms(-6.0, -14.0),
                    ChannelLevel::with_rms(-18.0, -26.0),
                ),
                cx,
            );
            c2.log("actions.paint", "L −6 dB, R −18 dB", cx);
        }),
        action_button(ctx.id("act-reset"), "reset", move |_, cx| {
            a3.reset(cx);
            c3.log("actions.reset", "", cx);
        }),
        action_button(ctx.id("act-clipping"), "read clipping", move |_, cx| {
            let clipping = a4.clipping(cx);
            c4.log("actions.clipping", clipping.to_string(), cx);
        }),
    ]
}

// ---------------------------------------------------------------------------
// Controls

const TICKS: &[(&str, &str)] = &[
    ("default", ""),
    ("every 6 dB", "(0..=20).map(|i| -6.0 * i as f32)"),
    ("sparse 0 / −12 / −24 / −48", "[0.0, -12.0, -24.0, -48.0]"),
];

const SCALE_FORMATS: &[(&str, &str)] = &[
    ("default (whole dB)", ""),
    ("with unit", "|db| format!(\"{db:.0} dB\").into()"),
    ("magnitude", "|db| format!(\"{:.0}\", db.abs()).into()"),
];

fn controls() -> Vec<Control> {
    let mut out = data_controls();
    out.extend(range_controls(0));
    out.extend(zone_controls(true));
    out.extend(ballistics_controls());
    out.extend([
        // Look
        Control::choice(
            "variant",
            "Variant",
            0,
            &[
                ("solid", "LevelMeterVariant::Solid"),
                ("segmented", "LevelMeterVariant::Segmented"),
                ("gradient", "LevelMeterVariant::Gradient"),
            ],
        )
        .group("Look"),
        Control::num("segments", "Segments (segmented)", 24.0, 2.0, 64.0, 1.0).group("Look"),
        Control::choice("orientation", "Orientation", 0, ORIENTATIONS).group("Look"),
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::num(
            "length",
            "Height when vertical (px)",
            224.0,
            96.0,
            480.0,
            4.0,
        )
        .group("Look")
        .no_code(),
        Control::text("aria_label", "Aria label", "Program level").group("Look"),
        // Parts
        Control::choice(
            "parts",
            "Children",
            1,
            &[("none (default tracks)", ""), ("composed parts", "")],
        )
        .group("Parts")
        .no_code()
        .help("Composed: Channels > Channel > Track > Bar / Hold, plus Scale, Value, Clip."),
        Control::bool("bar_peak", "Peak bar", true)
            .group("Parts: track")
            .no_code(),
        Control::num("peak_opacity", "Peak bar opacity", 1.0, 0.0, 1.0, 0.05)
            .group("Parts: track")
            .no_code(),
        Control::bool("bar_rms", "RMS bar (dual meter)", false)
            .group("Parts: track")
            .no_code(),
        Control::num("rms_opacity", "RMS bar opacity", 1.0, 0.0, 1.0, 0.05)
            .group("Parts: track")
            .no_code(),
        Control::bool("hold", "Hold marker", true)
            .group("Parts: track")
            .no_code(),
        Control::color("hold_color", "Hold colour")
            .group("Parts: track")
            .no_code(),
        // Scale
        Control::bool("scale", "Scale", true)
            .group("Parts: scale")
            .no_code(),
        Control::choice(
            "scale_side",
            "Side",
            1,
            &[("start", "DbScaleSide::Start"), ("end", "DbScaleSide::End")],
        )
        .group("Parts: scale")
        .no_code(),
        Control::bool("scale_labels", "Labels", true)
            .group("Parts: scale")
            .no_code(),
        Control::choice("scale_ticks", "Ticks", 0, TICKS)
            .group("Parts: scale")
            .no_code(),
        Control::choice("scale_format", "Format", 0, SCALE_FORMATS)
            .group("Parts: scale")
            .no_code(),
        Control::bool("scale_custom", "Custom ticks (children)", false)
            .group("Parts: scale")
            .no_code()
            .help("DbScaleTick children: 0 \"Unity\", −18 \"Ref\", minor −10."),
        // Value
        Control::bool("value", "Value", true)
            .group("Parts: value")
            .no_code(),
        Control::choice("value_measure", "Measure", 0, MEASURES)
            .group("Parts: value")
            .no_code(),
        Control::choice("value_channel", "Channel", 0, READOUT_CHANNELS)
            .group("Parts: value")
            .no_code(),
        Control::num("value_interval_ms", "Interval ms", 250.0, 16.0, 2000.0, 1.0)
            .group("Parts: value")
            .no_code(),
        Control::num("value_hold_ms", "Hold ms", 0.0, 0.0, 3000.0, 10.0)
            .group("Parts: value")
            .no_code(),
        Control::num("value_decimals", "Decimals", 1.0, 0.0, 3.0, 1.0)
            .group("Parts: value")
            .no_code(),
        Control::bool("value_unit", "Unit", true)
            .group("Parts: value")
            .no_code(),
        Control::bool("value_set_floor", "Own floor (else min_db)", false)
            .group("Parts: value")
            .no_code(),
        Control::num("value_floor_db", "Floor dB", -48.0, -120.0, 0.0, 1.0)
            .group("Parts: value")
            .no_code(),
        Control::bool("value_zones", "Zones = meter's zones", false)
            .group("Parts: value")
            .no_code()
            .help("Feeds the decorate's zone; else the readout's default zones."),
        Control::bool("value_decorate", "Decorate by zone", false)
            .group("Parts: value")
            .no_code()
            .help("Colours the text by zone; dims silence."),
        Control::choice(
            "value_format",
            "Format",
            0,
            &[("default", ""), ("custom: +0.00 dBFS", "")],
        )
        .group("Parts: value")
        .no_code(),
        // Clip
        Control::bool("clip", "Clip", true)
            .group("Parts: clip")
            .no_code(),
        Control::choice("clip_clipping", "Clipping", 0, CLIP_STATES)
            .group("Parts: clip")
            .no_code(),
        Control::num("clip_threshold_db", "Threshold dB", -1.0, -24.0, 6.0, 0.5)
            .group("Parts: clip")
            .no_code(),
        Control::num("clip_hold_ms", "Hold ms", 1500.0, 0.0, 10000.0, 50.0)
            .group("Parts: clip")
            .no_code(),
        Control::bool("clip_latching", "Latching (hold = ∞)", false)
            .group("Parts: clip")
            .no_code(),
        Control::bool("clip_show_count", "Show count", false)
            .group("Parts: clip")
            .no_code(),
        Control::text("clip_text", "Text (replaces the light)", "")
            .group("Parts: clip")
            .no_code(),
        // Actions
        Control::bool("actions", "Action buttons", true)
            .group("Actions")
            .no_code()
            .help("LevelMeterActions paint / reset / clipping; clip reset."),
    ]);
    out
}

// ---------------------------------------------------------------------------
// Render

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let data = Data::read(p);
    let (min_db, max_db) = (p.num("min_db"), p.num("max_db"));
    let actions = LevelMeterActions::new();
    let clip_actions = ClipIndicatorActions::new();

    let mut meter = LevelMeter::new(ctx.id("meter"))
        .config(ctx.config())
        .min_db(min_db)
        .max_db(max_db)
        .taper(taper(p, "taper", min_db, max_db))
        .ballistics(ballistics(p))
        .variant(match p.choice("variant") {
            "segmented" => LevelMeterVariant::Segmented,
            "gradient" => LevelMeterVariant::Gradient,
            _ => LevelMeterVariant::Solid,
        })
        .segments(p.num("segments").max(1.0) as usize)
        .channel_count(p.num("channel_count").max(1.0) as usize)
        .aria_label(p.text("aria_label"))
        .actions(actions.clone());
    meter = match &data {
        Data::Source => meter.source(ctx.meter()),
        Data::Mono { peak, rms } => meter.peak_db(*peak).rms_db(*rms),
        Data::Channels(levels) => meter.channels(levels.clone()),
    };
    if let Some(zones) = zones(p) {
        meter = meter.zones(zones);
    }
    for (key, zone) in [
        ("ok_color", MeterZoneName::Ok),
        ("warn_color", MeterZoneName::Warn),
        ("clip_color", MeterZoneName::Clip),
    ] {
        if let Some(color) = p.color(key, cx) {
            meter = meter.zone_color(zone, color);
        }
    }
    if let Some(size) = size(p, "size") {
        meter = meter.size(size);
    }
    if let Some(o) = orientation(p, "orientation") {
        meter = meter.orientation(o);
    }
    let vertical = orientation(p, "orientation")
        .or(ctx.config().orientation)
        .is_some_and(|o| o == Orientation::Vertical);
    if vertical {
        meter = meter.h(px(p.num("length")));
    }

    if p.index("parts") == 1 {
        meter = compose(meter, p, ctx, &data, &clip_actions, cx);
    }

    let mut stage = div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(meter);
    if p.bool("actions") {
        let mut buttons = meter_action_buttons(ctx, &actions);
        let (c1, c2) = (clip_actions.clone(), clip_actions);
        let (l1, l2) = (ctx.clone(), ctx.clone());
        buttons.push(action_button(
            ctx.id("act-clip-reset"),
            "clip: reset",
            move |window, cx| {
                let count = c1.count(cx);
                c1.reset(window, cx);
                l1.log("clip actions.reset", format!("count was {count}"), cx);
            },
        ));
        buttons.push(action_button(
            ctx.id("act-clip-report"),
            "clip: report 0 dB",
            move |window, cx| {
                c2.report(0.0, window, cx);
                l2.log("clip actions.report", "0 dB", cx);
            },
        ));
        stage = stage.child(action_row(buttons));
    }
    stage.into_any_element()
}

/// The meter built from its parts.
fn compose(
    mut meter: LevelMeter,
    p: &Props,
    ctx: &Ctx,
    data: &Data,
    clip_actions: &ClipIndicatorActions,
    cx: &mut App,
) -> LevelMeter {
    let tracks = data.tracks(ctx, p);
    let hold_color = p.color("hold_color", cx);
    let track = |index: usize| {
        let mut track = LevelMeterTrack::new();
        if p.bool("bar_peak") {
            track = track.child(
                LevelMeterBar::new()
                    .measure(LevelMeterMeasure::Peak)
                    .opacity(p.num("peak_opacity")),
            );
        }
        if p.bool("bar_rms") {
            track = track.child(
                LevelMeterBar::new()
                    .measure(LevelMeterMeasure::Rms)
                    .opacity(p.num("rms_opacity")),
            );
        }
        if p.bool("hold") {
            let mut hold = LevelMeterHold::new();
            if let Some(color) = hold_color {
                hold = hold.color(color);
            }
            track = track.child(hold);
        }
        LevelMeterChannel::new().index(index).child(track)
    };
    let mut channels = LevelMeterChannels::new().children((0..tracks).map(track));

    if p.bool("scale") {
        let mut scale = LevelMeterScale::new()
            .side(if p.index("scale_side") == 0 {
                DbScaleSide::Start
            } else {
                DbScaleSide::End
            })
            .labels(p.bool("scale_labels"));
        scale = match p.index("scale_ticks") {
            1 => scale.ticks((0..=20).map(|i| -6.0 * i as f32)),
            2 => scale.ticks([0.0, -12.0, -24.0, -48.0]),
            _ => scale,
        };
        scale = match p.index("scale_format") {
            1 => scale.format(|db| format!("{db:.0} dB").into()),
            2 => scale.format(|db| format!("{:.0}", db.abs()).into()),
            _ => scale,
        };
        if p.bool("scale_custom") {
            scale = scale
                .child(DbScaleTick::new(0.0).label("Unity"))
                .child(DbScaleTick::new(-18.0).label("Ref"))
                .child(DbScaleTick::new(-10.0).major(false));
        }
        channels = channels.child(scale);
    }
    meter = meter.child(channels);

    if p.bool("value") {
        let mut value = LevelMeterValue::new()
            .id(ctx.id("value"))
            .measure(measure(p, "value_measure"))
            .channel(readout_channel(p, "value_channel"))
            .interval_ms(p.num("value_interval_ms") as f64)
            .hold_ms(p.num("value_hold_ms") as f64)
            .decimals(p.num("value_decimals") as usize)
            .unit(p.bool("value_unit"));
        if p.bool("value_set_floor") {
            value = value.floor_db(p.num("value_floor_db"));
        }
        if p.bool("value_zones") {
            value = value.zones(zones(p).unwrap_or_else(|| zone_set(-20.0, -9.0)));
        }
        if p.index("value_format") == 1 {
            value = value.format(|db| {
                if db.is_finite() {
                    format!("{db:+.2} dBFS").into()
                } else {
                    "−∞ dBFS".into()
                }
            });
        }
        if p.bool("value_decorate") {
            let theme = cx.theme();
            let (ok, warn, clip, muted) = (
                theme.meter_ok,
                theme.meter_warn,
                theme.meter_clip,
                theme.muted_foreground.opacity(0.5),
            );
            value = value.decorate(move |display, el| {
                let color = if display.silent {
                    muted
                } else {
                    match display.zone {
                        MeterZoneName::Ok => ok,
                        MeterZoneName::Warn => warn,
                        MeterZoneName::Clip => clip,
                    }
                };
                el.text_color(color).into_any_element()
            });
        }
        meter = meter.child(value);
    }

    if p.bool("clip") {
        let log = ctx.clone();
        let mut clip = LevelMeterClip::new()
            .id(ctx.id("clip"))
            .threshold_db(p.num("clip_threshold_db"))
            .hold_ms(clip_hold(p, "clip_hold_ms", "clip_latching"))
            .show_count(p.bool("clip_show_count"))
            .actions(clip_actions.clone())
            .on_clipping_change(move |clipping, _, cx| {
                log.log("on_clipping_change", clipping.to_string(), cx)
            });
        if let Some(clipping) = clip_state(p, "clip_clipping") {
            clip = clip.clipping(clipping);
        }
        let text = p.text("clip_text");
        clip = clip.when(!text.is_empty(), |clip| clip.child(text));
        meter = meter.child(clip);
    }
    meter
}
