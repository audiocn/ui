//! Gallery page for `quick-audio-popover`. One variant per docs example: `components/examples/quick-audio-popover-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "quick-audio-popover",
    title: "Quick Audio Popover",
    description: "Microphone and system audio controls behind one toolbar button that shows a live level.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("quick-audio-popover", "default", cx),
        _ => return None,
    })
}
