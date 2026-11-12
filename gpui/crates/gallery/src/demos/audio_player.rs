//! Gallery page for `audio-player`. One variant per docs example: `components/examples/audio-player-demo.tsx`, `components/examples/audio-player-compact.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "audio-player",
    title: "Audio Player",
    description: "A composable audio player with transport, seeking, time, volume, rate, loop and keyboard shortcuts.",
    section: Section::Components,
    variants: &["default", "compact"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("audio-player", "default", cx),
        "compact" => pending("audio-player", "compact", cx),
        _ => return None,
    })
}
