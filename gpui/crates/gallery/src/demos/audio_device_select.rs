//! Gallery page for `audio-device-select`. One variant per docs example: `components/examples/audio-device-select-demo.tsx`, `components/examples/audio-device-select-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "audio-device-select",
    title: "Audio Device Select",
    description: "A microphone, speaker or source picker with permission, loading and disconnected states.",
    section: Section::Components,
    variants: &["default", "states"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("audio-device-select", "default", cx),
        "states" => pending("audio-device-select", "states", cx),
        _ => return None,
    })
}
