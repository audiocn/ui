//! Gallery page for `electric-bar-visualizer`. One variant per docs example: `components/examples/electric-bar-visualizer-demo.tsx`, `components/examples/electric-bar-visualizer-colors.tsx`, `components/examples/electric-bar-visualizer-intensity.tsx`, `components/examples/electric-bar-visualizer-microphone.tsx`, `components/examples/electric-bar-visualizer-mirrored.tsx`, `components/examples/electric-bar-visualizer-states.tsx`.

use audiocn::audio::bar_levels::BarIdle;
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::frame_source::{FrameEmitter, create_frame_emitter};
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::types::VisualFrame;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{ElectricBarAlign, ElectricBarVisualizer};
use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "electric-bar-visualizer",
    title: "Electric Bar Visualizer",
    description: "Frequency-band bars drawn as crackling filaments, with arcs between loud neighbours and sparks on sudden rises.",
    variants: &[
        "default",
        "colors",
        "intensity",
        "microphone",
        "mirrored",
        "states",
    ],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "colors" => colors(window, cx),
        "intensity" => intensity(window, cx),
        "microphone" => microphone(window, cx),
        "mirrored" => mirrored(window, cx),
        "states" => states(cx),
        _ => return None,
    })
}

fn base(variant: &str) -> SharedString {
    id("electric-bar-visualizer", variant)
}

/// `text-muted-foreground text-center text-xs`, in the mono font when `mono`.
fn caption(text: &'static str, mono: bool, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    let label = div()
        .text_color(theme.muted_foreground)
        .text_center()
        .text_xs()
        .line_height(px(16.0));
    if mono {
        label.font_family(theme.font_mono.clone()).child(text)
    } else {
        label.child(text)
    }
}

/// One `grid gap-2` cell: the visualizer over its caption.
fn cell(visualizer: ElectricBarVisualizer, label: &'static str, mono: bool, cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(visualizer)
        .child(caption(label, mono, cx))
}

/// `electric-bar-visualizer-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        format!("{}/signal", base("default")),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    ElectricBarVisualizer::new(base("default"))
        .aria_label("Voice activity")
        .text_color(cx.theme().primary)
        .h(px(112.0))
        .max_w(px(384.0))
        .source(signal.visual)
        .into_any_element()
}

/// `electric-bar-visualizer-colors.tsx`.
fn colors(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = base("colors");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(3),
        window,
        cx,
    );
    let theme = cx.theme();
    let colors: [(Hsla, &str); 4] = [
        (theme.primary, "primary"),
        (theme.meter_ok, "meter-ok"),
        (theme.channel_solo, "channel-solo"),
        (theme.destructive, "destructive"),
    ];
    div()
        .grid()
        .grid_cols(4)
        .w_full()
        .max_w(px(512.0))
        .gap(px(24.0))
        .children(colors.map(|(color, label)| {
            cell(
                ElectricBarVisualizer::new(format!("{base}/{label}"))
                    .align(ElectricBarAlign::End)
                    .aria_label(format!("Bars in {label}"))
                    .bar_count(9)
                    .h(px(80.0))
                    .text_color(color)
                    .source(signal.visual.clone()),
                label,
                true,
                cx,
            )
        }))
        .into_any_element()
}

/// `electric-bar-visualizer-intensity.tsx`.
fn intensity(window: &mut Window, cx: &mut App) -> AnyElement {
    const SETTINGS: [(bool, f32, &str, bool); 3] = [
        (false, 0.2, "calm", false),
        (true, 0.6, "default", true),
        (true, 1.0, "charged", true),
    ];
    let base = base("intensity");
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(5),
        window,
        cx,
    );
    let primary = cx.theme().primary;
    div()
        .grid()
        .grid_cols(3)
        .w_full()
        .max_w(px(448.0))
        .gap(px(24.0))
        .children(SETTINGS.map(|(arcs, intensity, label, sparks)| {
            cell(
                ElectricBarVisualizer::new(format!("{base}/{label}"))
                    .arcs(arcs)
                    .aria_label(format!("Visualizer, {label}"))
                    .bar_count(10)
                    .text_color(primary)
                    .h(px(80.0))
                    .intensity(intensity)
                    .source(signal.visual.clone())
                    .sparks(sparks),
                label,
                true,
                cx,
            )
        }))
        .into_any_element()
}

/// The "microphone": no device access in the gallery, so while listening a
/// demo signal is relayed into the analyser's visual source.
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
struct Microphone {
    listening: bool,
    visual: FrameEmitter<VisualFrame>,
}

/// `electric-bar-visualizer-microphone.tsx`.
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
    use audiocn::io::{MicrophoneOptions, MicrophoneStatus, use_microphone};

    let base = base("microphone");
    let mic = use_microphone(
        format!("{base}/mic"),
        MicrophoneOptions::default(),
        window,
        cx,
    );
    let listening = mic.status == MicrophoneStatus::Active;
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(16.0))
        .child(
            ElectricBarVisualizer::new(base.clone())
                .align(ElectricBarAlign::End)
                .aria_label("Microphone")
                .text_color(cx.theme().primary)
                .h(px(96.0))
                .idle(BarIdle::Wave)
                .source(mic.visual.clone()),
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
        .max_w(px(384.0))
        .gap(px(16.0))
        .child(
            ElectricBarVisualizer::new(base.clone())
                .align(ElectricBarAlign::End)
                .aria_label("Microphone")
                .text_color(cx.theme().primary)
                .h(px(96.0))
                .idle(BarIdle::Wave)
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

/// `electric-bar-visualizer-mirrored.tsx`.
fn mirrored(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        format!("{}/signal", base("mirrored")),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(8),
        window,
        cx,
    );
    ElectricBarVisualizer::new(base("mirrored"))
        .aria_label("Assistant speaking")
        .bar_count(21)
        .bar_gap(3.0)
        .bar_width(4.0)
        .text_color(cx.theme().foreground)
        .h(px(112.0))
        .max_w(px(448.0))
        .mirrored(true)
        .source(signal.visual)
        .into_any_element()
}

/// `electric-bar-visualizer-states.tsx`.
fn states(cx: &mut App) -> AnyElement {
    let base = base("states");
    let primary = cx.theme().primary;
    let bars = |name: &str, label: &'static str| {
        ElectricBarVisualizer::new(format!("{base}/{name}"))
            .aria_label(label)
            .bar_count(9)
            .text_color(primary)
            .h(px(64.0))
    };
    div()
        .grid()
        .grid_cols(3)
        .w_full()
        .max_w(px(448.0))
        .gap(px(24.0))
        .child(cell(
            bars("pulse", "Idle pulse").idle(BarIdle::Pulse),
            "idle pulse",
            false,
            cx,
        ))
        .child(cell(
            bars("wave", "Idle wave").idle(BarIdle::Wave),
            "idle wave",
            false,
            cx,
        ))
        .child(cell(
            bars("loading", "Connecting").loading(true),
            "loading",
            false,
            cx,
        ))
        .into_any_element()
}
