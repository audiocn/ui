//! Gallery page for `frame-source`. One variant per docs example: `components/examples/frame-source-demo.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "frame-source",
    title: "Frame Source",
    description: "Push frames from any engine: a socket, a worker, an audio thread or a test.",
    section: Section::Concepts,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("frame-source", "default", cx),
        _ => return None,
    })
}
