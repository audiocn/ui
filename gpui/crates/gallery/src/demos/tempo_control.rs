//! Gallery page for `tempo-control`. One variant per docs example:
//! `components/examples/tempo-control-demo.tsx`,
//! `components/examples/tempo-control-controlled.tsx`,
//! `components/examples/tempo-control-pad.tsx`,
//! `components/examples/tempo-control-states.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::theme::ActiveTheme;
use audiocn::{
    AudioSize, SoundPad, SoundPadLabel, TempoControl, TempoControlInput, TempoControlTap,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "tempo-control",
    title: "Tempo Control",
    description: "An editable BPM input with tap tempo, keyboard stepping and a rolling average.",
    variants: &["default", "controlled", "pad", "states"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => TempoControl::new(id("tempo-control", "default"))
            .default_value(120.0)
            .into_any_element(),
        "controlled" => controlled(window, cx),
        "pad" => pad(window, cx),
        "states" => states(cx),
        _ => return None,
    })
}

/// `tempo-control-controlled.tsx`.
fn controlled(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("tempo-control", "controlled");
    let bpm = window.use_keyed_state(format!("{base}/bpm"), cx, |_, _| 96.0_f32);
    let value = *bpm.read(cx);
    let muted = cx.theme().muted_foreground;
    let mono = cx.theme().font_mono.clone();
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.0))
        .child(
            TempoControl::new(base)
                .value(value)
                .on_value_change(move |v, _, _, cx| {
                    bpm.update(cx, |b, cx| {
                        *b = v;
                        cx.notify();
                    })
                })
                .child(TempoControlInput::new())
                .child(TempoControlTap::new().child("Tap tempo")),
        )
        .child(
            div()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .text_color(muted)
                .flex()
                .child("Beat duration: ")
                // `tabular-nums`
                .child(
                    div()
                        .font_family(mono)
                        .child(format!("{}", (60_000.0 / value).round() as i64)),
                )
                .child(" ms"),
        )
        .into_any_element()
}

/// `tempo-control-pad.tsx`: a sound pad as the only tap trigger.
fn pad(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("tempo-control", "pad");
    let bpm = window.use_keyed_state(format!("{base}/bpm"), cx, |_, _| 128.0_f32);
    let value = *bpm.read(cx);
    let mono = cx.theme().font_mono.clone();
    TempoControl::new(base)
        .w(px(144.0))
        .flex_col()
        .value(value)
        .on_value_change(move |v, _, _, cx| {
            bpm.update(cx, |b, cx| {
                *b = v;
                cx.notify();
            })
        })
        .child(
            div()
                .font_family(mono)
                .text_size(px(18.0))
                .line_height(px(28.0))
                .child(format!("{} BPM", value as i64)),
        )
        .child(
            TempoControlTap::new()
                .child(SoundPadLabel::new().child("Tap tempo"))
                .render(|props, _, _| {
                    let tap = props.tap.clone();
                    SoundPad::new("pad")
                        .disabled(props.disabled)
                        .on_trigger(move |window, cx| tap(window, cx))
                        .w_full()
                        .children(props.children)
                        .into_any_element()
                }),
        )
        .into_any_element()
}

/// `tempo-control-states.tsx`.
fn states(cx: &mut App) -> AnyElement {
    let base = id("tempo-control", "states");
    let muted = cx.theme().muted_foreground;
    let row = |label: &'static str, control: TempoControl| {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(muted)
                    .child(label),
            )
            .child(control)
    };
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(320.0))
        .gap(px(16.0))
        .child(row(
            "Small",
            TempoControl::new(format!("{base}/sm"))
                .aria_label("Small tempo")
                .default_value(80.0)
                .size(AudioSize::Sm),
        ))
        .child(row(
            "Large",
            TempoControl::new(format!("{base}/lg"))
                .aria_label("Large tempo")
                .default_value(128.0)
                .size(AudioSize::Lg),
        ))
        .child(row(
            "Disabled",
            TempoControl::new(format!("{base}/disabled"))
                .aria_label("Disabled tempo")
                .default_value(100.0)
                .disabled(true),
        ))
        .child(row(
            "Read-only",
            TempoControl::new(format!("{base}/read-only"))
                .aria_label("Read-only tempo")
                .read_only(true)
                .value(140.0),
        ))
        .into_any_element()
}
