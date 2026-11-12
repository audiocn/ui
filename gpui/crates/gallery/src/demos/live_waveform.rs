//! Gallery page for `live-waveform`. One variant per docs example:
//! `components/examples/live-waveform-demo.tsx`,
//! `components/examples/live-waveform-idle.tsx`,
//! `components/examples/live-waveform-microphone.tsx`,
//! `components/examples/live-waveform-variants.tsx`.
//!
//! The microphone example has no microphone here: "Use my microphone" feeds
//! the waveform from the demo speech signal instead.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{LiveWaveform, LiveWaveformMode, LiveWaveformVariant};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "live-waveform",
    title: "Live Waveform",
    description: "A canvas waveform of a live signal, as scrolling history or the current frame.",
    section: Section::Components,
    variants: &["default", "idle", "microphone", "variants"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "idle" => idle(window, cx),
        "microphone" => microphone(window, cx),
        "variants" => variants(window, cx),
        _ => return None,
    })
}

fn speech() -> DemoSignalOptions {
    DemoSignalOptions::kind(DemoSignalKind::Speech).with_history_size(120)
}

/// `live-waveform-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("live-waveform", "default");
    let signal = use_demo_signal(format!("{base}/signal"), speech(), window, cx);
    LiveWaveform::new(format!("{base}/waveform"))
        .h(px(80.0))
        .max_w(px(448.0))
        .mode(LiveWaveformMode::Scrolling)
        .source(signal.visual)
        .into_any_element()
}

/// `live-waveform-idle.tsx`: the dotted idle line until "Listen".
fn idle(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("live-waveform", "idle");
    let active = window.use_keyed_state(format!("{base}/active"), cx, |_, _| false);
    let is_active = *active.read(cx);
    let signal = use_demo_signal(format!("{base}/signal"), speech(), window, cx);
    let t = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .w_full()
        .max_w(px(448.0))
        .child(
            div()
                .bg(t.muted.opacity(0.2))
                .rounded(t.radius_lg())
                .border_1()
                .border_color(t.border)
                .child(
                    LiveWaveform::new(format!("{base}/waveform"))
                        .active(is_active)
                        .h(px(64.0))
                        .mode(LiveWaveformMode::Scrolling)
                        .source(signal.visual),
                ),
        )
        .child(
            Button::new(format!("{base}/toggle"))
                .self_start()
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    active.update(cx, |a, cx| {
                        *a = !*a;
                        cx.notify();
                    });
                })
                .child(if is_active { "Pause" } else { "Listen" }),
        )
        .into_any_element()
}

/// `live-waveform-microphone.tsx`, fed by the demo signal.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("live-waveform", "microphone");
    let listening = window.use_keyed_state(format!("{base}/listening"), cx, |_, _| false);
    let is_listening = *listening.read(cx);
    let signal = use_demo_signal(format!("{base}/signal"), speech(), window, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .w_full()
        .max_w(px(448.0))
        .child(
            LiveWaveform::new(format!("{base}/waveform"))
                .active(is_listening)
                .h(px(80.0))
                .mode(LiveWaveformMode::Scrolling)
                .source(signal.visual),
        )
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

/// `live-waveform-variants.tsx`: static bars, mirror and line, scrolling line.
fn variants(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("live-waveform", "variants");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Music).with_history_size(120),
        window,
        cx,
    );
    let t = cx.theme().clone();
    let row = |label: &'static str, waveform: LiveWaveform| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_color(t.muted_foreground)
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .child(label),
            )
            .child(waveform.h(px(56.0)))
    };
    let visual = signal.visual;
    div()
        .flex()
        .flex_col()
        .gap(px(24.0))
        .w_full()
        .max_w(px(448.0))
        .child(row(
            "static bars",
            LiveWaveform::new(format!("{base}/bars")).source(visual.clone()),
        ))
        .child(row(
            "static mirror",
            LiveWaveform::new(format!("{base}/mirror"))
                .text_color(t.primary)
                .source(visual.clone())
                .variant(LiveWaveformVariant::Mirror),
        ))
        .child(row(
            "static line",
            LiveWaveform::new(format!("{base}/line"))
                .source(visual.clone())
                .variant(LiveWaveformVariant::Line),
        ))
        .child(row(
            "scrolling line",
            LiveWaveform::new(format!("{base}/history"))
                .text_color(t.meter_ok)
                .mode(LiveWaveformMode::Scrolling)
                .source(visual)
                .variant(LiveWaveformVariant::Line),
        ))
        .into_any_element()
}
