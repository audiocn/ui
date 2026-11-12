//! Playground page for `DbScale`: range, ticks, taper, orientation, side,
//! labels, marks, format, custom ticks and config, with an optional meter on
//! the same range to check the alignment.

use audiocn::components::{DbScale, DbScaleSide, DbScaleTick, LevelMeter};
use audiocn::{AudioSize, Orientation};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::level_meter::{ORIENTATIONS, orientation, range_controls, taper};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "db-scale",
    component: "DbScale",
    source: SourceUse::None,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    let mut out = range_controls(0);
    out.extend([
        Control::choice(
            "ticks",
            "Ticks",
            0,
            &[
                ("default (common values in range)", ""),
                ("every 6 dB", "(0..=20).map(|i| -6.0 * i as f32)"),
                ("every 3 dB", "(0..=40).map(|i| -3.0 * i as f32)"),
                ("sparse 0 / −12 / −24 / −48", "[0.0, -12.0, -24.0, -48.0]"),
                (
                    "with headroom +6 / +3",
                    "[6.0, 3.0, 0.0, -6.0, -12.0, -24.0, -48.0]",
                ),
            ],
        )
        .group("Ticks"),
        Control::choice(
            "children",
            "Custom ticks (children)",
            0,
            &[
                ("none", ""),
                ("labelled + minor", ""),
                ("minor every 2 dB", ""),
            ],
        )
        .group("Ticks")
        .no_code()
        .help("child(DbScaleTick::new(..).label(..).major(..)): replace the default ticks."),
        Control::choice(
            "format",
            "Format",
            0,
            &[
                ("default (whole dB)", ""),
                ("with unit", "|db| format!(\"{db:.0} dB\").into()"),
                ("magnitude", "|db| format!(\"{:.0}\", db.abs()).into()"),
            ],
        )
        .group("Ticks"),
        Control::choice("orientation", "Orientation", 0, ORIENTATIONS).group("Look"),
        Control::choice(
            "side",
            "Side",
            1,
            &[("start", "DbScaleSide::Start"), ("end", "DbScaleSide::End")],
        )
        .group("Look"),
        Control::bool("labels", "Labels", true).group("Look"),
        Control::bool("marks", "Marks", true).group("Look"),
        Control::num(
            "length",
            "Height when vertical (px)",
            240.0,
            80.0,
            480.0,
            4.0,
        )
        .group("Look")
        .no_code(),
        Control::bool("meter", "Meter on the same range", true)
            .group("Look")
            .no_code()
            .help("A LevelMeter at −12 dB, to check the ticks line up."),
    ]);
    out
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let (min_db, max_db) = (p.num("min_db"), p.num("max_db"));
    let mut scale = DbScale::new()
        .config(ctx.config())
        .min_db(min_db)
        .max_db(max_db)
        .taper(taper(p, "taper", min_db, max_db))
        .side(if p.index("side") == 0 {
            DbScaleSide::Start
        } else {
            DbScaleSide::End
        })
        .labels(p.bool("labels"))
        .marks(p.bool("marks"));
    scale = match p.index("ticks") {
        1 => scale.ticks((0..=20).map(|i| -6.0 * i as f32)),
        2 => scale.ticks((0..=40).map(|i| -3.0 * i as f32)),
        3 => scale.ticks([0.0, -12.0, -24.0, -48.0]),
        4 => scale.ticks([6.0, 3.0, 0.0, -6.0, -12.0, -24.0, -48.0]),
        _ => scale,
    };
    scale = match p.index("children") {
        1 => scale
            .child(DbScaleTick::new(0.0).label("Unity"))
            .child(DbScaleTick::new(-18.0).label("Ref"))
            .child(DbScaleTick::new(-40.0))
            .child(DbScaleTick::new(-10.0).major(false))
            .child(DbScaleTick::new(-30.0).major(false)),
        2 => scale.children(
            (0..=60)
                .step_by(2)
                .map(|i| -(i as f32))
                .map(|db| DbScaleTick::new(db).major(db % 12.0 == 0.0)),
        ),
        _ => scale,
    };
    scale = match p.index("format") {
        1 => scale.format(|db| format!("{db:.0} dB").into()),
        2 => scale.format(|db| format!("{:.0}", db.abs()).into()),
        _ => scale,
    };
    if let Some(o) = orientation(p, "orientation") {
        scale = scale.orientation(o);
    }
    let vertical = orientation(p, "orientation")
        .or(ctx.config().orientation)
        .is_some_and(|o| o == Orientation::Vertical);
    let length = px(p.num("length"));

    let meter = p.bool("meter").then(|| {
        let mut meter = LevelMeter::new(ctx.id("meter"))
            .min_db(min_db)
            .max_db(max_db)
            .taper(taper(p, "taper", min_db, max_db))
            .size(AudioSize::Lg)
            .peak_db(-12.0);
        if vertical {
            meter = meter.orientation(Orientation::Vertical).h(length);
        }
        meter
    });

    if vertical {
        div()
            .flex()
            .justify_center()
            .gap_2()
            .h(length)
            .children(meter)
            .child(scale)
            .into_any_element()
    } else {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_1()
            .children(meter)
            .child(scale)
            .into_any_element()
    }
}
