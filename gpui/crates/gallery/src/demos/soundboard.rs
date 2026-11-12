//! Gallery page for `soundboard`. One variant per docs example: `components/examples/soundboard-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "soundboard",
    title: "Soundboard",
    description: "Sound pads with hotkeys, modes, volume, stop all and drag-and-drop.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("soundboard", "default", cx),
        _ => return None,
    })
}
