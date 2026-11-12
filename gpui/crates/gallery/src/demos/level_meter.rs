//! Gallery page for `level-meter`. One variant per docs example: `components/examples/level-meter-demo.tsx`, `components/examples/level-meter-ballistics.tsx`, `components/examples/level-meter-css-level.tsx`, `components/examples/level-meter-custom-colors.tsx`, `components/examples/level-meter-dual.tsx`, `components/examples/level-meter-microphone.tsx`, `components/examples/level-meter-simple.tsx`, `components/examples/level-meter-values.tsx`, `components/examples/level-meter-variants.tsx`, `components/examples/level-meter-vertical.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "level-meter",
    title: "Level Meter",
    description: "A peak and RMS level meter with zones, peak hold, a scale, a readout and a clip light.",
    section: Section::Components,
    variants: &[
        "default",
        "ballistics",
        "css-level",
        "custom-colors",
        "dual",
        "microphone",
        "simple",
        "values",
        "variants",
        "vertical",
    ],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("level-meter", "default", cx),
        "ballistics" => pending("level-meter", "ballistics", cx),
        "css-level" => pending("level-meter", "css-level", cx),
        "custom-colors" => pending("level-meter", "custom-colors", cx),
        "dual" => pending("level-meter", "dual", cx),
        "microphone" => pending("level-meter", "microphone", cx),
        "simple" => pending("level-meter", "simple", cx),
        "values" => pending("level-meter", "values", cx),
        "variants" => pending("level-meter", "variants", cx),
        "vertical" => pending("level-meter", "vertical", cx),
        _ => return None,
    })
}
