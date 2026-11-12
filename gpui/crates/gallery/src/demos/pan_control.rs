//! Gallery page for `pan-control`. One variant per docs example: `components/examples/pan-control-demo.tsx`, `components/examples/pan-control-knob.tsx`.

use gpui::{AnyElement, App, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::theme::ActiveTheme;
use audiocn::{
    Knob, KnobDial, KnobLabel, KnobPointer, KnobRange, KnobTrack, KnobValue, PanControl,
    format_pan, parse_pan,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "pan-control",
    title: "Pan Control",
    description: "Left and right balance with a fill from the centre and a centre detent.",
    variants: &["default", "knob"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "knob" => knob(window, cx),
        _ => return None,
    })
}

/// `pan-control-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("pan-control", "default");
    let state = window.use_keyed_state(base.clone(), cx, |_, _| -0.3_f32);
    let pan = *state.read(cx);
    let theme = cx.theme();
    div()
        .grid()
        .w_full()
        .max_w(px(320.0))
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .child(div().font_weight(FontWeight::MEDIUM).child("Pan"))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .font_family(theme.font_mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(format_pan(pan)),
                ),
        )
        .child(
            PanControl::new(format!("{base}/pan"))
                .on_value_change(move |value, _, cx| {
                    state.update(cx, |pan, cx| {
                        *pan = value;
                        cx.notify();
                    })
                })
                .value(pan),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_color(theme.muted_foreground)
                .text_size(px(12.0))
                .line_height(px(16.0))
                .child("L")
                .child("R"),
        )
        .into_any_element()
}

/// `pan-control-knob.tsx`: a controlled knob reading and writing pan text.
fn knob(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("pan-control", "knob");
    let state = window.use_keyed_state(base.clone(), cx, |_, _| 0.0_f32);
    let pan = *state.read(cx);
    // `aria-valuetext={describePan(pan)}` has no GPUI equivalent.
    Knob::new(format!("{base}/pan"))
        .format(format_pan)
        .large_step(0.25)
        .max(1.0)
        .min(-1.0)
        .on_value_change(move |value, _, _, cx| {
            state.update(cx, |pan, cx| {
                *pan = value;
                cx.notify();
            })
        })
        .origin(0.0)
        .parse(parse_pan)
        .reset_value(0.0)
        .step(0.05)
        .value(pan)
        .child(
            KnobDial::new()
                .child(KnobTrack::new())
                .child(KnobRange::new())
                .child(KnobPointer::new()),
        )
        .child(KnobValue::new())
        .child(KnobLabel::new("Pan"))
        .into_any_element()
}
