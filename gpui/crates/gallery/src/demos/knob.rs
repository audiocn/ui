//! Gallery page for `knob`. One variant per docs example: `components/examples/knob-demo.tsx`, `components/examples/knob-drag-directions.tsx`, `components/examples/knob-metal.tsx`, `components/examples/knob-sizes.tsx`, `components/examples/knob-volume.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "knob",
    title: "Knob",
    description: "A rotary control for dense layouts, drawn in SVG.",
    section: Section::Components,
    variants: &["default", "drag-directions", "metal", "sizes", "volume"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("knob", "default", cx),
        "drag-directions" => pending("knob", "drag-directions", cx),
        "metal" => pending("knob", "metal", cx),
        "sizes" => pending("knob", "sizes", cx),
        "volume" => pending("knob", "volume", cx),
        _ => return None,
    })
}
