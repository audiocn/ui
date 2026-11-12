//! Gallery page for `mic-setup`. One variant per docs example: `components/examples/mic-setup-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "mic-setup",
    title: "Mic Setup",
    description: "Choose and check a microphone, with a live preview, a meter, gain, mute and a level check.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("mic-setup", "default", cx),
        _ => return None,
    })
}
