//! Gallery page for `music-player`. One variant per docs example: `components/examples/music-player-demo.tsx`, `components/examples/music-player-ducking.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "music-player",
    title: "Music Player",
    description: "A playlist player with a waveform seek bar, shuffle, repeat and ducking under the microphone.",
    section: Section::Blocks,
    variants: &["default", "ducking"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("music-player", "default", cx),
        "ducking" => pending("music-player", "ducking", cx),
        _ => return None,
    })
}
