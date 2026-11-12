//! Gallery page for `db-readout`. One variant per docs example: `components/examples/db-readout-demo.tsx`, `components/examples/db-readout-zones.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "db-readout",
    title: "dB Readout",
    description: "A numeric level label that updates at a readable rate and never shifts the layout.",
    section: Section::Components,
    variants: &["default", "zones"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("db-readout", "default", cx),
        "zones" => pending("db-readout", "zones", cx),
        _ => return None,
    })
}
