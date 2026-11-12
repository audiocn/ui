//! Gallery page for `waveform`. One variant per docs example: `components/examples/waveform-demo.tsx`, `components/examples/waveform-regions.tsx`, `components/examples/waveform-variants.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "waveform",
    title: "Waveform",
    description: "A static waveform of a clip with a smooth playhead, seeking, hover time, regions and markers.",
    section: Section::Components,
    variants: &["default", "regions", "variants"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("waveform", "default", cx),
        "regions" => pending("waveform", "regions", cx),
        "variants" => pending("waveform", "variants", cx),
        _ => return None,
    })
}
