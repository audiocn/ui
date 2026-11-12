//! Gallery page for `spectrum`. One variant per docs example: `components/examples/spectrum-demo.tsx`, `components/examples/spectrum-microphone.tsx`, `components/examples/spectrum-variants.tsx`.

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::frame_source::{FrameEmitter, create_frame_emitter};
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::types::VisualFrame;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{Spectrum, SpectrumVariant};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "spectrum",
    title: "Spectrum",
    description: "A frequency spectrum analyser with axes, a grid and peak hold.",
    variants: &["default", "microphone", "variants"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "microphone" => microphone(window, cx),
        "variants" => variants(window, cx),
        _ => return None,
    })
}

fn signal_id(variant: &str) -> String {
    format!("{}/signal", id("spectrum", variant))
}

/// `spectrum-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("default"),
        DemoSignalOptions::kind(DemoSignalKind::Music).with_bands(48),
        window,
        cx,
    );
    Spectrum::new(id("spectrum", "default"))
        .max_w(px(512.0))
        .min_db(-60.0)
        .max_db(0.0)
        .peak_hold(true)
        .source(signal.visual)
        .into_any_element()
}

/// The "microphone": no device access in the gallery, so while listening a
/// demo signal is relayed into the analyser's visual source. Stopping leaves
/// the last frame drawn, like the analyser's relay.
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
struct Microphone {
    listening: bool,
    visual: FrameEmitter<VisualFrame>,
}

/// `spectrum-microphone.tsx`.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    #[cfg(all(feature = "device", not(target_family = "wasm")))]
    {
        microphone_device(window, cx)
    }
    #[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
    {
        microphone_demo(window, cx)
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
fn microphone_device(window: &mut Window, cx: &mut App) -> AnyElement {
    use audiocn::io::{AnalyserTapOptions, MicrophoneOptions, MicrophoneStatus, use_microphone};

    let base = id("spectrum", "microphone");
    let mic = use_microphone(
        format!("{base}/mic"),
        MicrophoneOptions {
            analyser: AnalyserTapOptions {
                bands: 64,
                fft_size: 4096,
                ..Default::default()
            },
            ..Default::default()
        },
        window,
        cx,
    );
    let listening = mic.status == MicrophoneStatus::Active;
    let visual = mic.visual.clone();
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(512.0))
        .gap(px(16.0))
        .child(Spectrum::new(base.clone()).peak_hold(true).source(visual))
        .child(
            div().flex().child(
                Button::new(format!("{base}/toggle"))
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .child(if listening {
                        "Stop microphone"
                    } else {
                        "Use my microphone"
                    })
                    .on_click(move |_, _, cx| {
                        if listening {
                            mic.stop(cx);
                        } else {
                            mic.start(cx);
                        }
                    }),
            ),
        )
        .into_any_element()
}

/// Demo-signal stand-in for the wasm gallery (no device access).
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
fn microphone_demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("spectrum", "microphone");
    let state = window.use_keyed_state(format!("{base}/microphone"), cx, |_, _| Microphone {
        listening: false,
        visual: create_frame_emitter(),
    });
    let (listening, visual) = {
        let mic = state.read(cx);
        (mic.listening, mic.visual.clone())
    };
    if listening {
        // `useAudioAnalyser(stream, { bands: 64, fftSize: 4096 })`.
        let signal = use_demo_signal(
            signal_id("microphone"),
            DemoSignalOptions::kind(DemoSignalKind::Speech).with_bands(64),
            window,
            cx,
        );
        if let Some(frame) = signal.visual.latest() {
            visual.emit(frame);
        }
    }
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(512.0))
        .gap(px(16.0))
        .child(
            Spectrum::new(base.clone())
                .peak_hold(true)
                .source(visual.source()),
        )
        .child(
            div().flex().child(
                Button::new(format!("{base}/toggle"))
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .child(if listening {
                        "Stop microphone"
                    } else {
                        "Use my microphone"
                    })
                    .on_click(move |_, _, cx| {
                        state.update(cx, |mic, cx| {
                            mic.listening = !mic.listening;
                            cx.notify();
                        });
                    }),
            ),
        )
        .into_any_element()
}

/// `spectrum-variants.tsx`.
fn variants(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("variants"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_bands(64),
        window,
        cx,
    );
    let base = id("spectrum", "variants");
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(512.0))
        .gap(px(24.0))
        .child(
            Spectrum::new(format!("{base}/line"))
                .h(px(112.0))
                .color(cx.theme().meter_ok)
                .grid(false)
                .source(signal.visual.clone())
                .variant(SpectrumVariant::Line),
        )
        .child(
            Spectrum::new(format!("{base}/area"))
                .h(px(112.0))
                .source(signal.visual)
                .variant(SpectrumVariant::Area),
        )
        .into_any_element()
}
