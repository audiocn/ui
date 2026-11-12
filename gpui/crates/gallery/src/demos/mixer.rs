//! Gallery page for `mixer`. One variant per docs example: `components/examples/mixer-demo.tsx`, `components/examples/mixer-console.tsx`, `components/examples/mixer-empty.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "mixer",
    title: "Mixer",
    description: "The container for channel strips, with shared meter settings and keyboard navigation.",
    section: Section::Components,
    variants: &["default", "console", "empty"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("mixer", "default", cx),
        "console" => pending("mixer", "console", cx),
        "empty" => pending("mixer", "empty", cx),
        _ => return None,
    })
}
