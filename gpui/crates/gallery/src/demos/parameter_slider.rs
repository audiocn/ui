//! Gallery page for `parameter-slider`. One variant per docs example: `components/examples/parameter-slider-demo.tsx`, `components/examples/parameter-slider-frequency.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "parameter-slider",
    title: "Parameter Slider",
    description: "A labelled slider with a numeric input, unit, marks and reset, for everything that is not a volume.",
    section: Section::Components,
    variants: &["default", "frequency"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("parameter-slider", "default", cx),
        "frequency" => pending("parameter-slider", "frequency", cx),
        _ => return None,
    })
}
