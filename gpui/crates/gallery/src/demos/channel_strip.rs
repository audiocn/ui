//! Gallery page for `channel-strip`. One variant per docs example: `components/examples/channel-strip-demo.tsx`, `components/examples/channel-strip-console.tsx`, `components/examples/channel-strip-notices.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "channel-strip",
    title: "Channel Strip",
    description: "One channel of a mixer, as a row or a console strip, composed from the parts you need.",
    section: Section::Components,
    variants: &["default", "console", "notices"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("channel-strip", "default", cx),
        "console" => pending("channel-strip", "console", cx),
        "notices" => pending("channel-strip", "notices", cx),
        _ => return None,
    })
}
