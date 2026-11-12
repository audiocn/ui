//! Gallery page for `db-scale`. One variant per docs example: `components/examples/db-scale-demo.tsx`, `components/examples/db-scale-vertical.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "db-scale",
    title: "dB Scale",
    description: "Tick marks and labels for a decibel range, shared by meters and faders.",
    section: Section::Components,
    variants: &["default", "vertical"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("db-scale", "default", cx),
        "vertical" => pending("db-scale", "vertical", cx),
        _ => return None,
    })
}
