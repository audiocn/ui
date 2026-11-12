//! Gallery page for `pan-control`. One variant per docs example: `components/examples/pan-control-demo.tsx`, `components/examples/pan-control-knob.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "pan-control",
    title: "Pan Control",
    description: "Left and right balance with a fill from the centre and a centre detent.",
    section: Section::Components,
    variants: &["default", "knob"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("pan-control", "default", cx),
        "knob" => pending("pan-control", "knob", cx),
        _ => return None,
    })
}
