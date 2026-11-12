//! Gallery page for `tempo-control`. One variant per docs example: `components/examples/tempo-control-demo.tsx`, `components/examples/tempo-control-controlled.tsx`, `components/examples/tempo-control-pad.tsx`, `components/examples/tempo-control-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "tempo-control",
    title: "Tempo Control",
    description: "An editable BPM input with tap tempo, keyboard stepping and a rolling average.",
    section: Section::Components,
    variants: &["default", "controlled", "pad", "states"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("tempo-control", "default", cx),
        "controlled" => pending("tempo-control", "controlled", cx),
        "pad" => pending("tempo-control", "pad", cx),
        "states" => pending("tempo-control", "states", cx),
        _ => return None,
    })
}
