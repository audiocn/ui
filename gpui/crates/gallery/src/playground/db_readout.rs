//! Playground page for `DbReadout`: source or static value, measure, channel,
//! interval, hold, decimals, unit, floor, zones, format and decorate.

use audiocn::components::{DbReadout, LevelMeter};
use audiocn::theme::ActiveTheme;
use audiocn::{MeterZone, MeterZoneName};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::level_meter::{MEASURES, READOUT_CHANNELS, measure, readout_channel};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "db-readout",
    component: "DbReadout",
    source: SourceUse::Meter,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    vec![
        Control::choice(
            "data",
            "Data",
            0,
            &[
                ("source (picker)", ""),
                ("static value", ""),
                ("value_opt(None)", ""),
            ],
        )
        .group("Data")
        .no_code(),
        Control::num("value", "Value dB (static)", -12.0, -100.0, 12.0, 0.1).group("Data"),
        Control::choice("measure", "Measure", 0, MEASURES).group("Reading"),
        Control::choice("channel", "Channel", 0, READOUT_CHANNELS).group("Reading"),
        Control::num("interval_ms", "Interval ms", 250.0, 0.0, 2000.0, 10.0).group("Reading"),
        Control::num("hold_ms", "Hold ms", 0.0, 0.0, 5000.0, 10.0)
            .group("Reading")
            .help("Shows the highest value within this window."),
        Control::num("decimals", "Decimals", 1.0, 0.0, 4.0, 1.0).group("Text"),
        Control::bool("unit", "Unit (\" dB\")", true).group("Text"),
        Control::num(
            "floor_db",
            "Floor dB (−∞ at or below)",
            -60.0,
            -120.0,
            0.0,
            1.0,
        )
        .group("Text"),
        Control::choice(
            "format",
            "Format",
            0,
            &[
                ("default", ""),
                ("+0.00 dBFS", "|db| format!(\"{db:+.2} dBFS\").into()"),
                (
                    "percent of full scale",
                    "|db| format!(\"{:.0}%\", 10f32.powf(db / 20.0) * 100.0).into()",
                ),
            ],
        )
        .group("Text"),
        Control::choice(
            "zones",
            "Zones",
            0,
            &[
                ("default (warn −20, clip −9)", ""),
                ("strict (warn −12, clip −3)", "vec![MeterZone { .. }, ..]"),
            ],
        )
        .group("Zones")
        .help("Only visible through decorate (data-zone)."),
        Control::choice(
            "decorate",
            "Decorate",
            1,
            &[
                ("none", ""),
                ("colour by zone, dim silence", "|display, el| .."),
                ("CLIP badge in the clip zone", "|display, el| .."),
            ],
        )
        .group("Zones"),
        Control::num("text_size", "Text size (px)", 14.0, 10.0, 48.0, 1.0)
            .group("Look")
            .no_code(),
        Control::bool("meter", "Show a meter beside it", true)
            .group("Look")
            .no_code(),
    ]
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let mut readout = DbReadout::new(ctx.id("readout"))
        .measure(measure(p, "measure"))
        .channel(readout_channel(p, "channel"))
        .interval_ms(p.num("interval_ms") as f64)
        .hold_ms(p.num("hold_ms") as f64)
        .decimals(p.num("decimals") as usize)
        .unit(p.bool("unit"))
        .floor_db(p.num("floor_db"))
        .text_size(px(p.num("text_size")));
    let source = match p.index("data") {
        0 => ctx.meter(),
        _ => None,
    };
    readout = match p.index("data") {
        0 => readout.source(source.clone()),
        1 => readout.value(p.num("value")),
        _ => readout.value_opt(None),
    };
    if p.index("zones") == 1 {
        readout = readout.zones(vec![
            MeterZone {
                from_db: f32::NEG_INFINITY,
                zone: MeterZoneName::Ok,
            },
            MeterZone {
                from_db: -12.0,
                zone: MeterZoneName::Warn,
            },
            MeterZone {
                from_db: -3.0,
                zone: MeterZoneName::Clip,
            },
        ]);
    }
    readout = match p.index("format") {
        1 => readout.format(|db| format!("{db:+.2} dBFS").into()),
        2 => readout.format(|db| format!("{:.0}%", 10f32.powf(db / 20.0) * 100.0).into()),
        _ => readout,
    };
    let theme = cx.theme();
    let (ok, warn, clip, muted, clip_fg) = (
        theme.meter_ok,
        theme.meter_warn,
        theme.meter_clip,
        theme.muted_foreground.opacity(0.5),
        theme.meter_clip_foreground,
    );
    readout = match p.index("decorate") {
        1 => readout.decorate(move |display, el| {
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
        }),
        2 => readout.decorate(move |display, el| {
            let clipping = display.zone == MeterZoneName::Clip && !display.silent;
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(el)
                .children(clipping.then(|| {
                    div()
                        .px_1()
                        .rounded_sm()
                        .bg(clip)
                        .text_color(clip_fg)
                        .text_xs()
                        .child("CLIP")
                }))
                .into_any_element()
        }),
        _ => readout,
    };

    let mut row = div()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .gap_4()
        .child(readout);
    if p.bool("meter") {
        let mut meter = LevelMeter::new(ctx.id("meter")).config(ctx.config());
        meter = match p.index("data") {
            0 => meter.source(source),
            1 => meter.peak_db(p.num("value")),
            _ => meter,
        };
        row = row.child(div().flex_1().max_w(px(320.0)).child(meter));
    }
    row.into_any_element()
}
