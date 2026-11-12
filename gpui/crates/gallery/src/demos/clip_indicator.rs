//! Gallery page for `clip-indicator`. One variant per docs example:
//! `components/examples/clip-indicator-demo.tsx`, `clip-indicator-latching.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{ClipIndicator, ClipIndicatorActions};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "clip-indicator",
    title: "Clip Indicator",
    description: "A clip light that holds after the signal clips, with an optional count and click to reset.",
    section: Section::Components,
    variants: &["default", "latching"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "latching" => latching(window, cx),
        _ => return None,
    })
}

/// `clip-indicator-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        id("clip-indicator", "default/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Music).with_seed(11),
        window,
        cx,
    );
    div()
        .flex()
        .items_center()
        .gap(px(24.0))
        .child(
            ClipIndicator::new(id("clip-indicator", "default/light")).source(signal.meter.clone()),
        )
        .child(
            ClipIndicator::new(id("clip-indicator", "default/count"))
                .show_count(true)
                .source(signal.meter.clone()),
        )
        .child(
            ClipIndicator::new(id("clip-indicator", "default/text"))
                .source(signal.meter)
                .child("Clip"),
        )
        .into_any_element()
}

/// `clip-indicator-latching.tsx`.
fn latching(window: &mut Window, cx: &mut App) -> AnyElement {
    // `useRef<ClipIndicatorActions>`.
    let clip = window
        .use_keyed_state(id("clip-indicator", "latching/actions"), cx, |_, _| {
            ClipIndicatorActions::new()
        })
        .read(cx)
        .clone();
    let simulate = clip.clone();
    let reset = clip.clone();
    div()
        .flex()
        .items_center()
        .gap(px(16.0))
        .child(
            ClipIndicator::new(id("clip-indicator", "latching"))
                .actions(clip)
                .hold_ms(f64::INFINITY)
                .show_count(true),
        )
        .child(
            Button::new(id("clip-indicator", "latching/simulate"))
                .on_click(move |_, window, cx| simulate.report(0.0, window, cx))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .child("Simulate a clip"),
        )
        .child(
            Button::new(id("clip-indicator", "latching/reset"))
                .on_click(move |_, window, cx| reset.reset(window, cx))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .child("Reset"),
        )
        .into_any_element()
}
