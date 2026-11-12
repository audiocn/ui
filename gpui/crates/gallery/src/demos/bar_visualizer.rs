//! Gallery page for `bar-visualizer`. One variant per docs example: `components/examples/bar-visualizer-demo.tsx`, `components/examples/bar-visualizer-align.tsx`, `components/examples/bar-visualizer-mini.tsx`, `components/examples/bar-visualizer-mirrored.tsx`, `components/examples/bar-visualizer-states.tsx`.

use audiocn::audio::bar_levels::BarIdle;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::{Icon, IconName};
use audiocn::ui::{Badge, BadgeVariant};
use audiocn::{BarVisualizer, BarVisualizerAlign};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "bar-visualizer",
    title: "Bar Visualizer",
    description: "A row of bars driven by frequency bands, with idle, loading and mirrored modes.",
    section: Section::Components,
    variants: &["default", "align", "mini", "mirrored", "states"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "align" => align(window, cx),
        "mini" => mini(window, cx),
        "mirrored" => mirrored(window, cx),
        "states" => states(cx),
        _ => return None,
    })
}

fn signal_id(variant: &str) -> String {
    format!("{}/signal", id("bar-visualizer", variant))
}

/// `text-muted-foreground text-center text-xs`.
fn caption(text: &'static str, cx: &App) -> gpui::Div {
    div()
        .text_color(cx.theme().muted_foreground)
        .text_center()
        .text_size(px(12.0))
        .line_height(px(16.0))
        .child(text)
}

/// `bar-visualizer-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("default"),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    BarVisualizer::new(id("bar-visualizer", "default"))
        .aria_label("Voice activity")
        .source(signal.visual)
        .text_color(cx.theme().primary)
        .h(px(96.0))
        .max_w(px(384.0))
        .into_any_element()
}

/// `bar-visualizer-align.tsx`.
fn align(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("align"),
        DemoSignalOptions::kind(DemoSignalKind::Music),
        window,
        cx,
    );
    let theme = cx.theme();
    let alignments = [
        (BarVisualizerAlign::Center, "center"),
        (BarVisualizerAlign::End, "end"),
        (BarVisualizerAlign::Start, "start"),
    ];
    div()
        .flex()
        .w_full()
        .max_w(px(448.0))
        .gap(px(24.0))
        .children(alignments.into_iter().map(|(align, name)| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .bg(theme.muted.opacity(0.4))
                        .rounded(theme.radius_lg())
                        .p(px(8.0))
                        .child(
                            BarVisualizer::new(format!("{}/{name}", id("bar-visualizer", "align")))
                                .align(align)
                                .aria_label(format!("Bars aligned to {name}"))
                                .bar_count(12)
                                .h(px(64.0))
                                .source(signal.visual.clone()),
                        ),
                )
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .text_center()
                        .font_family(theme.font_mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(name),
                )
        }))
        .into_any_element()
}

/// `bar-visualizer-mini.tsx`.
fn mini(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("mini"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(12),
        window,
        cx,
    );
    Badge::new()
        .variant(BadgeVariant::Secondary)
        .h(px(28.0))
        .child(Icon::new(IconName::Microphone).size(px(12.0)))
        .child("Live")
        .child(
            BarVisualizer::new(id("bar-visualizer", "mini"))
                .aria_hidden(true)
                .bar_count(5)
                .h(px(14.0))
                .w(px(28.0))
                .bar_gap(px(2.0))
                .bar_width(px(3.0))
                .min_level(0.15)
                .source(signal.visual),
        )
        .into_any_element()
}

/// `bar-visualizer-mirrored.tsx`.
fn mirrored(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        signal_id("mirrored"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(8),
        window,
        cx,
    );
    BarVisualizer::new(id("bar-visualizer", "mirrored"))
        .aria_label("Assistant speaking")
        .bar_count(31)
        .text_color(cx.theme().foreground)
        .h(px(112.0))
        .max_w(px(448.0))
        .bar_gap(px(2.0))
        .bar_width(px(4.0))
        .mirrored(true)
        .source(signal.visual)
        .into_any_element()
}

/// `bar-visualizer-states.tsx`.
fn states(cx: &mut App) -> AnyElement {
    let base = id("bar-visualizer", "states");
    let state = |name: &'static str, label: &'static str, caption_text: &'static str| {
        let bars = BarVisualizer::new(format!("{base}/{name}"))
            .aria_label(label)
            .bar_count(9)
            .h(px(64.0));
        let bars = match name {
            "pulse" => bars.idle(BarIdle::Pulse),
            "wave" => bars.idle(BarIdle::Wave),
            _ => bars.loading(true),
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(8.0))
            .child(bars)
            .child(caption(caption_text, cx))
    };
    div()
        .flex()
        .w_full()
        .max_w(px(448.0))
        .gap(px(24.0))
        .child(state("pulse", "Idle pulse", "idle pulse"))
        .child(state("wave", "Idle wave", "idle wave"))
        .child(state("loading", "Connecting", "loading"))
        .into_any_element()
}
