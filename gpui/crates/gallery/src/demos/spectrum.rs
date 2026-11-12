//! Gallery page for `spectrum`. One variant per docs example: `components/examples/spectrum-demo.tsx`, `components/examples/spectrum-microphone.tsx`, `components/examples/spectrum-variants.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "spectrum",
    title: "Spectrum",
    description: "A frequency spectrum analyser with axes, a grid and peak hold.",
    section: Section::Components,
    variants: &["default", "microphone", "variants"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("spectrum", "default", cx),
        "microphone" => pending("spectrum", "microphone", cx),
        "variants" => pending("spectrum", "variants", cx),
        _ => return None,
    })
}
