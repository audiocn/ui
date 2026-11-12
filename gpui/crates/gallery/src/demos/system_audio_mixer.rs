//! Gallery page for `system-audio-mixer`. One variant per docs example:
//! `components/examples/system-audio-mixer-demo.tsx`,
//! `components/examples/system-audio-mixer-console.tsx`.
//!
//! The docs play synthesised tracks and sound pads (`useDemoTracks`,
//! `useDemoSounds`). Here those are the same titles, artists, hotkeys, accents
//! and durations, and the block's built-in simulated players.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::theme::oklch;
use audiocn::{MixerSound, MixerTrack, Orientation, SystemAudioMixer};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "system-audio-mixer",
    title: "System Audio Mixer",
    description: "The complete mixer, with microphone, system audio, music, sound pads and a master, running on Web Audio.",
    variants: &["default", "console"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => mixer(variant, false),
        "console" => mixer(variant, true),
        _ => return None,
    })
}

/// `DEMO_TRACKS` of `lib/docs/demo-audio.ts`.
fn tracks() -> Vec<MixerTrack> {
    vec![
        MixerTrack::new("night-drive", "Night Drive")
            .artist("audiocn")
            .src("night-drive")
            .duration(19.2),
        MixerTrack::new("low-tide", "Low Tide")
            .artist("audiocn")
            .src("low-tide")
            .duration(24.0),
        MixerTrack::new("arcade", "Arcade")
            .artist("audiocn")
            .src("arcade")
            .duration(13.7),
    ]
}

/// `DEMO_SOUNDS` of `lib/docs/demo-audio.ts`, with each sound's length.
fn sounds() -> Vec<MixerSound> {
    let sound = |id, label, hotkey, accent: (f32, f32, f32), seconds| {
        MixerSound::new(id, label)
            .src(id)
            .hotkey(hotkey)
            .accent(oklch(accent.0, accent.1, accent.2))
            .duration(seconds)
    };
    vec![
        sound("airhorn", "Airhorn", "1", (0.7, 0.2, 30.0), 1.4),
        sound("applause", "Applause", "2", (0.75, 0.15, 85.0), 2.4),
        sound("ding", "Ding", "3", (0.7, 0.15, 250.0), 1.7),
        sound("laser", "Laser", "4", (0.7, 0.2, 320.0), 0.5),
        sound("success", "Success", "5", (0.72, 0.17, 150.0), 1.3),
        sound("fail", "Fail", "6", (0.65, 0.2, 20.0), 2.0),
        sound("whoosh", "Whoosh", "7", (0.7, 0.1, 200.0), 1.2),
        sound("drumroll", "Drum roll", "8", (0.68, 0.16, 60.0), 2.8),
    ]
}

/// `system-audio-mixer-demo.tsx` (`w-full`) and `system-audio-mixer-console.tsx`
/// (`h-[30rem] w-full`, vertical).
fn mixer(variant: &str, console: bool) -> AnyElement {
    let mut mixer = SystemAudioMixer::new(id("system-audio-mixer", variant))
        .sounds(sounds())
        .tracks(tracks())
        .w_full();
    if console {
        mixer = mixer
            .default_orientation(Orientation::Vertical)
            .h(px(480.0));
    }
    mixer.into_any_element()
}
