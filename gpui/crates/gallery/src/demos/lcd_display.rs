//! Gallery page for `lcd-display`. One variant per docs example: `components/examples/lcd-display-demo.tsx`, `components/examples/lcd-display-font.tsx`, `components/examples/lcd-display-variants.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "lcd-display",
    title: "LCD Display",
    description: "A dot-matrix screen for labelled readouts and settings, backlit in the light theme and glowing in the dark one, with dropdown values.",
    section: Section::Components,
    variants: &["default", "font", "variants"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("lcd-display", "default", cx),
        "font" => pending("lcd-display", "font", cx),
        "variants" => pending("lcd-display", "variants", cx),
        _ => return None,
    })
}
