//! Gallery page for `bar-visualizer`. One variant per docs example: `components/examples/bar-visualizer-demo.tsx`, `components/examples/bar-visualizer-align.tsx`, `components/examples/bar-visualizer-mini.tsx`, `components/examples/bar-visualizer-mirrored.tsx`, `components/examples/bar-visualizer-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "bar-visualizer",
    title: "Bar Visualizer",
    description: "A row of bars driven by frequency bands, with idle, loading and mirrored modes.",
    section: Section::Components,
    variants: &["default", "align", "mini", "mirrored", "states"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("bar-visualizer", "default", cx),
        "align" => pending("bar-visualizer", "align", cx),
        "mini" => pending("bar-visualizer", "mini", cx),
        "mirrored" => pending("bar-visualizer", "mirrored", cx),
        "states" => pending("bar-visualizer", "states", cx),
        _ => return None,
    })
}
