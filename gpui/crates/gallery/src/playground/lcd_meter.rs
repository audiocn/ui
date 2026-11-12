//! Playground page for `LcdMeter`, drawn inside an `LcdDisplay`: data, range,
//! taper, zones, ballistics, the `--lcd-meter-*` options (segment, gap,
//! thickness, unlit) and every part (channel rows with label / track / bars /
//! hold / value, the scale and the CLIP flag), plus the imperative actions.

use audiocn::components::{
    DbReadoutChannel, LcdDisplay, LcdDisplayLabel, LcdDisplayRow, LcdDisplaySeparator,
    LcdDisplayValue, LcdMeter, LcdMeterBar, LcdMeterChannel, LcdMeterClip, LcdMeterHold,
    LcdMeterLabel, LcdMeterScale, LcdMeterTrack, LcdMeterValue, LevelMeterActions,
    LevelMeterMeasure, lcd_meter_channel_label,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::lcd_display::{apply_look, look_controls, status, status_controls};
use super::level_meter::{
    CLIP_STATES, Data, MEASURES, action_row, ballistics, ballistics_controls, clip_hold,
    clip_state, data_controls, measure, meter_action_buttons, range_controls, taper, zone_controls,
    zones,
};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "lcd-meter",
    component: "LcdMeter",
    source: SourceUse::Meter,
    controls,
    render,
};

const VALUE_CHANNELS: &[(&str, &str)] = &[
    ("row's channel", ""),
    ("max", "DbReadoutChannel::Max"),
    ("0", "DbReadoutChannel::Index(0)"),
    ("1", "DbReadoutChannel::Index(1)"),
];

fn controls() -> Vec<Control> {
    let mut out = data_controls();
    // Taper default `Audio`.
    out.extend(range_controls(1));
    out.extend(zone_controls(false));
    out.extend(ballistics_controls());
    out.extend([
        // `--lcd-meter-*`
        Control::num("segment", "Segment px (−1 = 3 dots)", -1.0, -1.0, 24.0, 1.0)
            .group("Segments"),
        Control::num(
            "gap",
            "Gap px (−1 = 1 dot, 0 = solid)",
            -1.0,
            -1.0,
            12.0,
            1.0,
        )
        .group("Segments"),
        Control::num(
            "thickness",
            "Thickness px (−1 = by size)",
            -1.0,
            -1.0,
            40.0,
            1.0,
        )
        .group("Segments"),
        Control::color("unlit", "Unlit colour").group("Segments"),
        Control::text("aria_label", "Aria label", "Input level").group("Segments"),
        // Parts
        Control::choice(
            "parts",
            "Children",
            0,
            &[("none (default rows)", ""), ("composed parts", "")],
        )
        .group("Parts")
        .no_code()
        .help("Default: a row per channel (L / R labels, peak bar, hold, value), scale, CLIP."),
        Control::choice(
            "label",
            "Row label",
            1,
            &[
                ("none", ""),
                ("auto (L / R / 1, 2, ..)", ""),
                ("custom text", ""),
            ],
        )
        .group("Parts: rows")
        .no_code(),
        Control::text("label_text", "Custom label", "Mic")
            .group("Parts: rows")
            .no_code(),
        Control::bool("bar_peak", "Peak bar", true)
            .group("Parts: rows")
            .no_code(),
        Control::bool("bar_rms", "RMS bar", false)
            .group("Parts: rows")
            .no_code(),
        Control::bool("hold", "Hold segment", true)
            .group("Parts: rows")
            .no_code(),
        Control::bool("value", "Value", true)
            .group("Parts: value")
            .no_code(),
        Control::choice("value_channel", "Channel", 0, VALUE_CHANNELS)
            .group("Parts: value")
            .no_code(),
        Control::choice("value_measure", "Measure", 0, MEASURES)
            .group("Parts: value")
            .no_code(),
        Control::bool("value_unit", "Unit", false)
            .group("Parts: value")
            .no_code(),
        Control::num("value_decimals", "Decimals", 1.0, 0.0, 3.0, 1.0)
            .group("Parts: value")
            .no_code(),
        Control::num("value_interval_ms", "Interval ms", 250.0, 16.0, 2000.0, 1.0)
            .group("Parts: value")
            .no_code(),
        Control::num("value_hold_ms", "Hold ms", 0.0, 0.0, 3000.0, 10.0)
            .group("Parts: value")
            .no_code(),
        Control::bool("scale", "Scale", true)
            .group("Parts: scale")
            .no_code(),
        Control::choice(
            "scale_ticks",
            "Ticks",
            0,
            &[
                ("default (48 24 12 6 3 0)", ""),
                ("every 6 dB", ""),
                ("sparse 0 / 12 / 40", ""),
            ],
        )
        .group("Parts: scale")
        .no_code(),
        Control::choice(
            "scale_format",
            "Format",
            0,
            &[
                ("default (magnitude)", ""),
                ("signed", ""),
                ("dB suffix", ""),
            ],
        )
        .group("Parts: scale")
        .no_code(),
        Control::bool("scale_bracket", "Bracket over clip zone", true)
            .group("Parts: scale")
            .no_code(),
        Control::bool("clip", "CLIP flag", true)
            .group("Parts: clip")
            .no_code(),
        Control::text("clip_text", "Text (default \"Clip\")", "")
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
    ]);
    out.extend(look_controls("Display", false));
    out.push(
        Control::bool("input_row", "Input row + separator", true)
            .group("Display")
            .no_code(),
    );
    out.extend(status_controls("Display"));
    out.push(
        Control::bool("actions", "Action buttons", true)
            .group("Actions")
            .no_code()
            .help("LevelMeterActions paint / reset / clipping."),
    );
    out
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let data = Data::read(p);
    let (min_db, max_db) = (p.num("min_db"), p.num("max_db"));
    let actions = LevelMeterActions::new();

    let mut meter = LcdMeter::new(ctx.id("meter"))
        .config(ctx.config())
        .min_db(min_db)
        .max_db(max_db)
        .taper(taper(p, "taper", min_db, max_db))
        .ballistics(ballistics(p))
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
    for (key, set) in [
        (
            "segment",
            LcdMeter::segment as fn(LcdMeter, gpui::Pixels) -> LcdMeter,
        ),
        ("gap", LcdMeter::gap),
        ("thickness", LcdMeter::thickness),
    ] {
        if p.num(key) >= 0.0 {
            meter = set(meter, px(p.num(key)));
        }
    }
    if let Some(color) = p.color("unlit", cx) {
        meter = meter.unlit(color);
    }
    if p.index("parts") == 1 {
        meter = compose(meter, p, ctx, &data);
    }

    let mut display = apply_look(LcdDisplay::new(ctx.id("display")).w_full(), p);
    if p.bool("input_row") {
        display = display
            .child(
                LcdDisplayRow::new()
                    .child(LcdDisplayLabel::new("Input"))
                    .child(LcdDisplayValue::new(ctx.id("input"), "Line In")),
            )
            .child(LcdDisplaySeparator::new());
    }
    display = display.child(meter);
    if let Some(status) = status(p) {
        display = display.child(status);
    }

    let mut stage = div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(display);
    if p.bool("actions") {
        stage = stage.child(action_row(meter_action_buttons(ctx, &actions)));
    }
    stage.into_any_element()
}

fn compose(mut meter: LcdMeter, p: &Props, ctx: &Ctx, data: &Data) -> LcdMeter {
    let rows = data.tracks(ctx, p);
    for index in 0..rows {
        let mut row = LcdMeterChannel::new().index(index);
        let label = match p.index("label") {
            1 => lcd_meter_channel_label(index, rows),
            2 => Some(p.text("label_text")),
            _ => None,
        };
        if let Some(label) = label {
            row = row.child(LcdMeterLabel::new(label));
        }
        let mut track = LcdMeterTrack::new();
        if p.bool("bar_rms") {
            track = track.child(LcdMeterBar::new().measure(LevelMeterMeasure::Rms));
        }
        if p.bool("bar_peak") {
            track = track.child(LcdMeterBar::new().measure(LevelMeterMeasure::Peak));
        }
        if p.bool("hold") {
            track = track.child(LcdMeterHold::new());
        }
        row = row.child(track);
        if p.bool("value") {
            let mut value = LcdMeterValue::new()
                .measure(measure(p, "value_measure"))
                .unit(p.bool("value_unit"))
                .decimals(p.num("value_decimals") as usize)
                .interval_ms(p.num("value_interval_ms") as f64)
                .hold_ms(p.num("value_hold_ms") as f64);
            value = match p.index("value_channel") {
                1 => value.channel(DbReadoutChannel::Max),
                2 => value.channel(0),
                3 => value.channel(1),
                _ => value,
            };
            row = row.child(value);
        }
        meter = meter.child(row);
    }
    if p.bool("scale") {
        let mut scale = LcdMeterScale::new().bracket(p.bool("scale_bracket"));
        scale = match p.index("scale_ticks") {
            1 => scale.ticks((0..=10).map(|i| -6.0 * i as f32)),
            2 => scale.ticks([0.0, -12.0, -40.0]),
            _ => scale,
        };
        scale = match p.index("scale_format") {
            1 => scale.format(|db| format!("{db:+.0}").into()),
            2 => scale.format(|db| format!("{db:.0}dB").into()),
            _ => scale,
        };
        meter = meter.child(scale);
    }
    if p.bool("clip") {
        let log = ctx.clone();
        let mut clip = LcdMeterClip::new()
            .threshold_db(p.num("clip_threshold_db"))
            .hold_ms(clip_hold(p, "clip_hold_ms", "clip_latching"))
            .on_clipping_change(move |clipping, _, cx| {
                log.log("on_clipping_change", clipping.to_string(), cx)
            });
        if let Some(clipping) = clip_state(p, "clip_clipping") {
            clip = clip.clipping(clipping);
        }
        let text = p.text("clip_text");
        if !text.is_empty() {
            clip = clip.child(text);
        }
        meter = meter.child(clip);
    }
    meter
}
