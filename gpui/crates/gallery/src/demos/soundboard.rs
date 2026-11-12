//! Gallery page for `soundboard`. One variant per docs example: `components/examples/soundboard-demo.tsx`.
//!
//! `useDemoSounds` synthesises the eight docs clips before the board mounts.
//! Here those clips are [`SoundboardSound`]s with the same ids, labels, hotkeys,
//! accents and lengths; the board's built-in simulator plays them.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::theme::oklch;
use audiocn::{Soundboard, SoundboardSound};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "soundboard",
    title: "Soundboard",
    description: "Sound pads with hotkeys, modes, volume, stop all and drag-and-drop.",
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(),
        _ => return None,
    })
}

/// `DEMO_SOUNDS` plus each clip's length from `lib/docs/demo-audio.ts`.
fn demo_sounds() -> Vec<SoundboardSound> {
    let clips = [
        ("airhorn", "Airhorn", "1", (0.7, 0.2, 30.0), 1.4),
        ("applause", "Applause", "2", (0.75, 0.15, 85.0), 2.4),
        ("ding", "Ding", "3", (0.7, 0.15, 250.0), 1.7),
        ("laser", "Laser", "4", (0.7, 0.2, 320.0), 0.5),
        ("success", "Success", "5", (0.72, 0.17, 150.0), 1.3),
        ("fail", "Fail", "6", (0.65, 0.2, 20.0), 2.0),
        ("whoosh", "Whoosh", "7", (0.7, 0.1, 200.0), 1.2),
        ("drumroll", "Drum roll", "8", (0.68, 0.16, 60.0), 2.8),
    ];
    clips
        .into_iter()
        .map(|(id, label, hotkey, (l, c, h), seconds)| {
            SoundboardSound::new(id, label, id)
                .hotkey(hotkey)
                .accent(oklch(l, c, h))
                .duration(seconds)
        })
        .collect()
}

/// `soundboard-demo.tsx`: `w-full max-w-2xl`, the eight demo sounds.
fn demo() -> AnyElement {
    Soundboard::new(id("soundboard", "default"))
        .w_full()
        .max_w(px(672.0))
        .default_sounds(demo_sounds())
        .into_any_element()
}
