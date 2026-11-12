//! Gallery page for `quick-audio-popover`. One variant per docs example:
//! `components/examples/quick-audio-popover-demo.tsx`.
//!
//! The React demo renders the popover closed. This page opens it and turns
//! system audio on so the centred gallery frame shows the same panel as
//! `.github/readme/quick-audio-popover.png`. The microphone stays off; bars
//! are the idle level until "Turn on", which then uses the speech demo signal.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::QuickAudioPopover;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "quick-audio-popover",
    title: "Quick Audio Popover",
    description: "Microphone and system audio controls behind one toolbar button that shows a live level.",
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(),
        _ => return None,
    })
}

/// `quick-audio-popover-demo.tsx`, opened so the panel is in the frame.
fn demo() -> AnyElement {
    // The variant shot is 760×520 with the example centred. The panel opens
    // downward, so the button sits at the top of a box tall enough to hold it.
    div()
        .w(px(400.0))
        .h(px(420.0))
        .flex()
        .flex_col()
        .items_center()
        .child(
            QuickAudioPopover::new(id("quick-audio-popover", "default"))
                .default_open(true)
                .default_system_active(true)
                .child(
                    Button::new("quick-audio-popover/more")
                        .w_full()
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .child("More audio settings"),
                ),
        )
        .into_any_element()
}
