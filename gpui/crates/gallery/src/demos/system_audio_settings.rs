//! Gallery page for `system-audio-settings`. One variant per docs example: `components/examples/system-audio-settings-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "system-audio-settings",
    title: "System Audio Settings",
    description: "Turn system audio capture on, set its level and see what the browser captures.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("system-audio-settings", "default", cx),
        _ => return None,
    })
}
