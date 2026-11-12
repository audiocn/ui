//! Gallery page for `electric-bar-visualizer`. One variant per docs example: `components/examples/electric-bar-visualizer-demo.tsx`, `components/examples/electric-bar-visualizer-colors.tsx`, `components/examples/electric-bar-visualizer-intensity.tsx`, `components/examples/electric-bar-visualizer-microphone.tsx`, `components/examples/electric-bar-visualizer-mirrored.tsx`, `components/examples/electric-bar-visualizer-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "electric-bar-visualizer",
    title: "Electric Bar Visualizer",
    description: "Frequency-band bars drawn as crackling filaments, with arcs between loud neighbours and sparks on sudden rises.",
    section: Section::Components,
    variants: &[
        "default",
        "colors",
        "intensity",
        "microphone",
        "mirrored",
        "states",
    ],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("electric-bar-visualizer", "default", cx),
        "colors" => pending("electric-bar-visualizer", "colors", cx),
        "intensity" => pending("electric-bar-visualizer", "intensity", cx),
        "microphone" => pending("electric-bar-visualizer", "microphone", cx),
        "mirrored" => pending("electric-bar-visualizer", "mirrored", cx),
        "states" => pending("electric-bar-visualizer", "states", cx),
        _ => return None,
    })
}
