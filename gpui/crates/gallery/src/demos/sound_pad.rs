//! Gallery page for `sound-pad`. One variant per docs example: `components/examples/sound-pad-demo.tsx`, `components/examples/sound-pad-grid.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "sound-pad",
    title: "Sound Pad",
    description: "A trigger pad with modes, a hotkey, playback progress and an accent colour.",
    section: Section::Components,
    variants: &["default", "grid"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("sound-pad", "default", cx),
        "grid" => pending("sound-pad", "grid", cx),
        _ => return None,
    })
}
