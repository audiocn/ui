//! Gallery page for `electric-waveform`. One variant per docs example: `components/examples/electric-waveform-demo.tsx`, `components/examples/electric-waveform-intensity.tsx`, `components/examples/electric-waveform-microphone.tsx`, `components/examples/electric-waveform-modes.tsx`, `components/examples/electric-waveform-states.tsx`.

use audiocn::ElectricWaveform;
use audiocn::audio::frame_source::{FrameEmitter, create_frame_emitter};
use audiocn::audio::types::VisualFrame;
use audiocn::audio::wave_line::WaveLineMode;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "electric-waveform",
    title: "Electric Waveform",
    description: "One electric line across the width, with a crackling white-hot core, a tall glow, forks off the peaks and sparks.",
    section: Section::Components,
    variants: &["default", "intensity", "microphone", "modes", "states"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "intensity" => intensity(window, cx),
        "microphone" => microphone(window, cx),
        "modes" => modes(window, cx),
        "states" => states(cx),
        _ => return None,
    })
}

fn base(variant: &str) -> SharedString {
    id("electric-waveform", variant)
}

/// `text-muted-foreground text-center text-xs`, in the mono font when `mono`.
fn caption(text: &'static str, mono: bool, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    let label = div()
        .text_color(theme.muted_foreground)
        .text_center()
        .text_xs();
    if mono {
        label.font_family(theme.font_mono.clone()).child(text)
    } else {
        label.child(text)
    }
}

/// `electric-waveform-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        format!("{}/signal", base("default")),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    ElectricWaveform::new(base("default"))
        .aria_label("Voice activity")
        .text_color(cx.theme().primary)
        .h(px(128.0))
        .max_w(px(512.0))
        .source(signal.visual)
        .into_any_element()
}

/// `electric-waveform-intensity.tsx`.
fn intensity(window: &mut Window, cx: &mut App) -> AnyElement {
    const SETTINGS: [(bool, f32, &str, bool); 3] = [
        (false, 0.15, "calm", false),
        (true, 0.6, "default", true),
        (true, 1.0, "charged", true),
    ];
    let base = base("intensity");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(6),
        window,
        cx,
    );
    let primary = cx.theme().primary;
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(512.0))
        .gap(px(16.0))
        .children(SETTINGS.map(|(arcs, intensity, label, sparks)| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    ElectricWaveform::new(format!("{base}/{label}"))
                        .arcs(arcs)
                        .aria_label(format!("Voice, {label}"))
                        .text_color(primary)
                        .h(px(80.0))
                        .intensity(intensity)
                        .source(signal.visual.clone())
                        .sparks(sparks),
                )
                .child(caption(label, true, cx))
        }))
        .into_any_element()
}

/// The "microphone": no device access in the gallery, so while listening a
/// demo signal is relayed into the analyser's visual source.
struct Microphone {
    listening: bool,
    visual: FrameEmitter<VisualFrame>,
}

/// `electric-waveform-microphone.tsx`.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = base("microphone");
    let state = window.use_keyed_state(format!("{base}/microphone"), cx, |_, _| Microphone {
        listening: false,
        visual: create_frame_emitter(),
    });
    let (listening, visual) = {
        let mic = state.read(cx);
        (mic.listening, mic.visual.clone())
    };
    if listening {
        let signal = use_demo_signal(
            format!("{base}/signal"),
            DemoSignalOptions::kind(DemoSignalKind::Speech),
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
            ElectricWaveform::new(base.clone())
                .aria_label("Microphone")
                .text_color(cx.theme().primary)
                .h(px(112.0))
                .mode(WaveLineMode::Scope)
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

/// `electric-waveform-modes.tsx`.
fn modes(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = base("modes");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(4),
        window,
        cx,
    );
    let primary = cx.theme().primary;
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(512.0))
        .gap(px(24.0))
        .children(
            [(WaveLineMode::Wave, "wave"), (WaveLineMode::Scope, "scope")].map(|(mode, label)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        ElectricWaveform::new(format!("{base}/{label}"))
                            .aria_label(format!("Voice, {label}"))
                            .text_color(primary)
                            .h(px(96.0))
                            .mode(mode)
                            .source(signal.visual.clone()),
                    )
                    .child(caption(label, true, cx))
            }),
        )
        .into_any_element()
}

/// `electric-waveform-states.tsx`.
fn states(cx: &mut App) -> AnyElement {
    let base = base("states");
    let primary = cx.theme().primary;
    div()
        .grid()
        .grid_cols(2)
        .w_full()
        .max_w(px(512.0))
        .gap(px(24.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    ElectricWaveform::new(format!("{base}/idle"))
                        .aria_label("Idle")
                        .text_color(primary)
                        .h(px(80.0)),
                )
                .child(caption("idle", false, cx)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    ElectricWaveform::new(format!("{base}/loading"))
                        .aria_label("Connecting")
                        .text_color(primary)
                        .h(px(80.0))
                        .loading(true),
                )
                .child(caption("loading", false, cx)),
        )
        .into_any_element()
}
