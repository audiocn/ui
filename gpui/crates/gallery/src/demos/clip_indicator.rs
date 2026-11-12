//! Gallery page for `clip-indicator`. One variant per docs example: `components/examples/clip-indicator-demo.tsx`, `components/examples/clip-indicator-latching.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "clip-indicator",
    title: "Clip Indicator",
    description: "A clip light that holds after the signal clips, with an optional count and click to reset.",
    section: Section::Components,
    variants: &["default", "latching"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("clip-indicator", "default", cx),
        "latching" => pending("clip-indicator", "latching", cx),
        _ => return None,
    })
}
