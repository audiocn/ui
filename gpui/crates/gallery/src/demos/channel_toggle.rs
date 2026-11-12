//! Gallery page for `channel-toggle`. One variant per docs example: `components/examples/channel-toggle-demo.tsx`, `components/examples/channel-toggle-variants.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "channel-toggle",
    title: "Channel Toggle",
    description: "Mute, solo and monitor toggle buttons with their own pressed colours.",
    section: Section::Components,
    variants: &["default", "variants"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("channel-toggle", "default", cx),
        "variants" => pending("channel-toggle", "variants", cx),
        _ => return None,
    })
}
