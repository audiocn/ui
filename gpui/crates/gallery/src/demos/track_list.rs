//! Gallery page for `track-list`. One variant per docs example: `components/examples/track-list-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "track-list",
    title: "Track List",
    description: "A list of tracks with active and playing states and arrow-key navigation.",
    section: Section::Components,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("track-list", "default", cx),
        _ => return None,
    })
}
