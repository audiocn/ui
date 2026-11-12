//! Gallery page for `smooth-waveform`. One variant per docs example:
//! `components/examples/smooth-waveform-demo.tsx`,
//! `components/examples/smooth-waveform-microphone.tsx`,
//! `components/examples/smooth-waveform-modes.tsx`,
//! `components/examples/smooth-waveform-states.tsx`.
//!
//! The microphone example has no microphone here: "Use my microphone" feeds
//! the scope from the demo speech signal instead.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{SmoothWaveform, SmoothWaveformMode};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "smooth-waveform",
    title: "Smooth Waveform",
    description: "One clean line across the width that follows the sound, as a flowing wave or an oscilloscope trace.",
    variants: &["default", "microphone", "modes", "states"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "microphone" => microphone(window, cx),
        "modes" => modes(window, cx),
        "states" => states(cx),
        _ => return None,
    })
}

/// `smooth-waveform-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("smooth-waveform", "default");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    let primary = cx.theme().primary;
    SmoothWaveform::new(format!("{base}/waveform"))
        .text_color(primary)
        .h(px(128.0))
        .max_w(px(512.0))
        .source(signal.visual)
        .into_any_element()
}

/// `smooth-waveform-microphone.tsx`, fed by the demo signal.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("smooth-waveform", "microphone");
    let listening = window.use_keyed_state(format!("{base}/listening"), cx, |_, _| false);
    let is_listening = *listening.read(cx);
    let signal = use_demo_signal(
        format!("{base}/signal"),
        // Stopped, the "microphone" falls silent.
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_playing(is_listening),
        window,
        cx,
    );
    let primary = cx.theme().primary;
    let waveform = SmoothWaveform::new(format!("{base}/waveform"))
        .text_color(primary)
        .h(px(112.0))
        .mode(SmoothWaveformMode::Scope)
        .source(signal.visual);
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .w_full()
        .max_w(px(512.0))
        .child(waveform)
        .child(
            Button::new(format!("{base}/toggle"))
                .self_start()
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    listening.update(cx, |l, cx| {
                        *l = !*l;
                        cx.notify();
                    });
                })
                .child(if is_listening {
                    "Stop microphone"
                } else {
                    "Use my microphone"
                }),
        )
        .into_any_element()
}

/// `smooth-waveform-modes.tsx`: the same voice as a wave and as a scope.
fn modes(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("smooth-waveform", "modes");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(4.0),
        window,
        cx,
    );
    let t = cx.theme().clone();
    let modes = [
        ("wave", SmoothWaveformMode::Wave),
        ("scope", SmoothWaveformMode::Scope),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(24.0))
        .w_full()
        .max_w(px(512.0))
        .children(modes.into_iter().map(|(name, mode)| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    SmoothWaveform::new(format!("{base}/{name}"))
                        .text_color(t.primary)
                        .h(px(96.0))
                        .mode(mode)
                        .source(signal.visual.clone()),
                )
                .child(
                    div()
                        .text_color(t.muted_foreground)
                        .font_family(t.font_mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_center()
                        .child(name),
                )
        }))
        .into_any_element()
}

/// `smooth-waveform-states.tsx`: idle and loading, side by side (`sm:grid-cols-2`).
fn states(cx: &mut App) -> AnyElement {
    let base = id("smooth-waveform", "states");
    let t = cx.theme().clone();
    let cell = |name: &'static str, loading: bool| {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(8.0))
            .child(
                SmoothWaveform::new(format!("{base}/{name}"))
                    .text_color(t.primary)
                    .h(px(80.0))
                    .loading(loading),
            )
            .child(
                div()
                    .text_color(t.muted_foreground)
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_center()
                    .child(name),
            )
    };
    div()
        .flex()
        .gap(px(24.0))
        .w_full()
        .max_w(px(512.0))
        .child(cell("idle", false))
        .child(cell("loading", true))
        .into_any_element()
}
