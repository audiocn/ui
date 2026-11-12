//! Gallery page for `live-waveform`. One variant per docs example: `components/examples/live-waveform-demo.tsx`, `components/examples/live-waveform-idle.tsx`, `components/examples/live-waveform-microphone.tsx`, `components/examples/live-waveform-variants.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "live-waveform",
    title: "Live Waveform",
    description: "A canvas waveform of a live signal, as scrolling history or the current frame.",
    section: Section::Components,
    variants: &["default", "idle", "microphone", "variants"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("live-waveform", "default", cx),
        "idle" => pending("live-waveform", "idle", cx),
        "microphone" => pending("live-waveform", "microphone", cx),
        "variants" => pending("live-waveform", "variants", cx),
        _ => return None,
    })
}
