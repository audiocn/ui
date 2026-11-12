//! Gallery page for `system-audio-mixer`. One variant per docs example: `components/examples/system-audio-mixer-demo.tsx`, `components/examples/system-audio-mixer-console.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "system-audio-mixer",
    title: "System Audio Mixer",
    description: "The complete mixer, with microphone, system audio, music, sound pads and a master, running on Web Audio.",
    section: Section::Blocks,
    variants: &["default", "console"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("system-audio-mixer", "default", cx),
        "console" => pending("system-audio-mixer", "console", cx),
        _ => return None,
    })
}
