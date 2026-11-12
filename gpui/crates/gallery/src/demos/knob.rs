//! Gallery page for `knob`. One variant per docs example: `components/examples/knob-demo.tsx`, `components/examples/knob-drag-directions.tsx`, `components/examples/knob-metal.tsx`, `components/examples/knob-sizes.tsx`, `components/examples/knob-volume.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::audio::decibels::{FormatDbOptions, format_db};
use audiocn::theme::ActiveTheme;
use audiocn::{
    AudioSize, Knob, KnobCap, KnobCapVariant, KnobDial, KnobDragDirection, KnobLabel, KnobPointer,
    KnobRange, KnobScale, KnobScaleType, KnobTrack, KnobValue, format_pan, parse_pan,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "knob",
    title: "Knob",
    description: "A rotary control for dense layouts, drawn in SVG.",
    variants: &["default", "drag-directions", "metal", "sizes", "volume"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(cx),
        "drag-directions" => drag_directions(),
        "metal" => metal(cx),
        "sizes" => sizes(cx),
        "volume" => volume(),
        _ => return None,
    })
}

fn format_db_decimals(db: f32, decimals: usize) -> String {
    format_db(
        db,
        FormatDbOptions {
            decimals,
            ..Default::default()
        },
    )
}

fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{}", hz.round())
    }
}

fn format_percent(percent: f32) -> String {
    format!("{}%", percent.round())
}

fn knob_id(variant: &str, name: &str) -> String {
    format!("{}/{name}", id("knob", variant))
}

/// The three knobs of `knob-demo.tsx` and `knob-metal.tsx`: gain, pan and low cut.
fn gain_pan_low_cut(variant: &str, cap: Option<KnobCap>, cx: &App) -> [Knob; 3] {
    let dial = |range: KnobRange| {
        let dial = KnobDial::new().child(KnobTrack::new()).child(range);
        match &cap {
            Some(cap) => dial.child(cap.clone()),
            None => dial.child(KnobPointer::new()),
        }
    };
    [
        Knob::new(knob_id(variant, "gain"))
            .default_value(0.0)
            .format(|db| format_db_decimals(db, 0))
            .max(24.0)
            .min(-24.0)
            .origin(0.0)
            .child(dial(KnobRange::new()))
            .child(KnobValue::new())
            .child(KnobLabel::new("Gain")),
        Knob::new(knob_id(variant, "pan"))
            .default_value(0.0)
            .format(format_pan)
            .max(1.0)
            .min(-1.0)
            .origin(0.0)
            .parse(parse_pan)
            .step(0.05)
            .child(dial(KnobRange::new()))
            .child(KnobValue::new())
            .child(KnobLabel::new("Pan")),
        Knob::new(knob_id(variant, "low-cut"))
            .default_value(120.0)
            .format(format_hz)
            .max(20_000.0)
            .min(20.0)
            .scale(KnobScaleType::Log)
            .child(dial(KnobRange::new().color(cx.theme().meter_ok)))
            .child(KnobValue::new())
            .child(KnobLabel::new("Low cut")),
    ]
}

/// `knob-demo.tsx`.
fn demo(cx: &mut App) -> AnyElement {
    div()
        .flex()
        .items_start()
        .gap(px(32.0))
        .children(gain_pan_low_cut("default", None, cx))
        .into_any_element()
}

/// `knob-drag-directions.tsx`.
fn drag_directions() -> AnyElement {
    let directions = [
        ("Vertical", KnobDragDirection::Vertical, "vertical"),
        ("Horizontal", KnobDragDirection::Horizontal, "horizontal"),
        ("Circular", KnobDragDirection::Circular, "circular"),
    ];
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .justify_center()
        .gap(px(32.0))
        .children(directions.into_iter().map(|(label, direction, key)| {
            Knob::new(knob_id("drag-directions", key))
                .default_value(0.0)
                .drag_direction(direction)
                .format(|db| format_db_decimals(db, 1))
                .max(24.0)
                .min(-24.0)
                .origin(0.0)
                .child(
                    KnobDial::new()
                        .child(KnobTrack::new())
                        .child(KnobRange::new())
                        .child(KnobPointer::new()),
                )
                .child(KnobValue::new())
                .child(KnobLabel::new(label))
        }))
        .into_any_element()
}

/// `knob-metal.tsx`.
fn metal(cx: &mut App) -> AnyElement {
    let mini = KnobCap::new().variant(KnobCapVariant::Mini);
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .justify_center()
        .gap(px(32.0))
        .children(gain_pan_low_cut("metal", Some(mini.clone()), cx))
        .child(
            Knob::new(knob_id("metal", "mix"))
                .default_value(65.0)
                .format(format_percent)
                .child(
                    KnobDial::new()
                        .child(KnobTrack::new())
                        .child(KnobRange::new())
                        .child(mini),
                )
                .child(KnobValue::new())
                .child(KnobLabel::new("Mix")),
        )
        .into_any_element()
}

/// `knob-sizes.tsx`.
fn sizes(cx: &mut App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let dial = || {
        KnobDial::new()
            .child(KnobTrack::new())
            .child(KnobRange::new())
            .child(KnobPointer::new())
    };
    let sizes = [
        (AudioSize::Sm, "sm"),
        (AudioSize::Default, "default"),
        (AudioSize::Lg, "lg"),
    ];
    div()
        .flex()
        .flex_wrap()
        .items_end()
        .justify_center()
        .gap_x(px(32.0))
        .gap_y(px(24.0))
        .children(sizes.into_iter().map(|(size, name)| {
            Knob::new(knob_id("sizes", name))
                .default_value(65.0)
                .size(size)
                .child(dial())
                .child(KnobLabel::new(name).text_color(muted))
        }))
        .child(
            Knob::new(knob_id("sizes", "disabled"))
                .default_value(30.0)
                .disabled(true)
                .child(dial())
                .child(KnobLabel::new("disabled").text_color(muted)),
        )
        .into_any_element()
}

/// `knob-volume.tsx`.
fn volume() -> AnyElement {
    Knob::new(knob_id("volume", "volume"))
        .knob_size(px(224.0))
        .click_sound(true)
        .default_value(33.0)
        .child(
            KnobDial::new()
                .child(KnobScale::new().label_every(10).major_every(5).ticks(100))
                .child(KnobCap::new()),
        )
        .child(KnobLabel::new("Volume").sr_only())
        .into_any_element()
}
