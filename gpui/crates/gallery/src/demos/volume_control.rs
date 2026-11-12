//! Gallery page for `volume-control`. One variant per docs example: `components/examples/volume-control-demo.tsx`, `components/examples/volume-control-popover.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "volume-control",
    title: "Volume Control",
    description: "A simple volume slider with a mute button, for players.",
    section: Section::Components,
    variants: &["default", "popover"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("volume-control", "default", cx),
        "popover" => pending("volume-control", "popover", cx),
        _ => return None,
    })
}
