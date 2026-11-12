//! Gallery page for `sound-pad`. One variant per docs example: `components/examples/sound-pad-demo.tsx`, `components/examples/sound-pad-grid.tsx`.
//!
//! The docs examples play synthesised sound effects (`useDemoSounds` +
//! `useSound`); here each sound is a `SimulatedPlayer` of the same length, so
//! pads show playing state and progress exactly like the real thing.

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, px};

use audiocn::theme::oklch;
use audiocn::ui::icons::IconName;
use audiocn::{
    AudioPlayerController, SimulatedPlayer, SoundPad, SoundPadGrid, SoundPadIcon, SoundPadLabel,
    SoundPadMode, SoundPadProgress, SoundPadProgressVariant, SoundPadShortcut,
    UseAudioPlayerOptions, use_simulated_player,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "sound-pad",
    title: "Sound Pad",
    description: "A trigger pad with modes, a hotkey, playback progress and an accent colour.",
    variants: &["default", "grid"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "grid" => grid(window, cx),
        _ => return None,
    })
}

/// `DEMO_SOUNDS` of `lib/docs/demo-audio.ts`, with each sound's length.
struct DemoSound {
    id: &'static str,
    label: &'static str,
    hotkey: &'static str,
    /// `oklch(l c h)`.
    accent: (f32, f32, f32),
    seconds: f64,
}

const DEMO_SOUNDS: [DemoSound; 8] = [
    DemoSound {
        id: "airhorn",
        label: "Airhorn",
        hotkey: "1",
        accent: (0.7, 0.2, 30.0),
        seconds: 1.4,
    },
    DemoSound {
        id: "applause",
        label: "Applause",
        hotkey: "2",
        accent: (0.75, 0.15, 85.0),
        seconds: 2.4,
    },
    DemoSound {
        id: "ding",
        label: "Ding",
        hotkey: "3",
        accent: (0.7, 0.15, 250.0),
        seconds: 1.7,
    },
    DemoSound {
        id: "laser",
        label: "Laser",
        hotkey: "4",
        accent: (0.7, 0.2, 320.0),
        seconds: 0.5,
    },
    DemoSound {
        id: "success",
        label: "Success",
        hotkey: "5",
        accent: (0.72, 0.17, 150.0),
        seconds: 1.3,
    },
    DemoSound {
        id: "fail",
        label: "Fail",
        hotkey: "6",
        accent: (0.65, 0.2, 20.0),
        seconds: 2.0,
    },
    DemoSound {
        id: "whoosh",
        label: "Whoosh",
        hotkey: "7",
        accent: (0.7, 0.1, 200.0),
        seconds: 1.2,
    },
    DemoSound {
        id: "drumroll",
        label: "Drum roll",
        hotkey: "8",
        accent: (0.68, 0.16, 60.0),
        seconds: 2.8,
    },
];

/// `useSound(src, { loop })`: a simulated voice of the sound's length.
fn use_sound(
    key: String,
    sound: &DemoSound,
    r#loop: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SimulatedPlayer> {
    let options = UseAudioPlayerOptions {
        r#loop,
        ..UseAudioPlayerOptions::src(sound.id)
    };
    use_simulated_player(key, sound.seconds, options, window, cx)
}

/// `sound.play()`: restarts the voice (`interrupt` defaults to true).
fn play(sound: &Entity<SimulatedPlayer>, window: &mut Window, cx: &mut App) {
    sound.seek(0.0, window, cx);
    sound.play(window, cx);
}

/// `sound.stop()`.
fn stop(sound: &Entity<SimulatedPlayer>, window: &mut Window, cx: &mut App) {
    sound.pause(window, cx);
    sound.seek(0.0, window, cx);
}

/// `sound-pad-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("sound-pad", "default");
    let sound = &DEMO_SOUNDS[0];
    let airhorn = use_sound(format!("{base}/airhorn"), sound, false, window, cx);
    let (playing, progress) = {
        let player = airhorn.read(cx);
        (player.state().playing, player.progress_source())
    };
    let (l, c, h) = sound.accent;
    SoundPad::new(base)
        .accent(oklch(l, c, h))
        .w(px(160.0))
        .hotkey(sound.hotkey)
        .loading(false)
        .on_trigger(move |window, cx| play(&airhorn, window, cx))
        .playing(playing)
        .child(SoundPadIcon::new().icon(IconName::Megaphone))
        .child(SoundPadLabel::new().child(sound.label))
        .child(SoundPadShortcut::new())
        .child(SoundPadProgress::new().source(progress))
        .into_any_element()
}

/// `MODES` of `sound-pad-grid.tsx`.
fn mode_of(id: &str) -> SoundPadMode {
    match id {
        "drumroll" => SoundPadMode::Hold,
        "whoosh" => SoundPadMode::Toggle,
        _ => SoundPadMode::OneShot,
    }
}

/// `sound-pad-grid.tsx`.
fn grid(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("sound-pad", "grid");
    let mut grid = SoundPadGrid::new(base.clone())
        .w_full()
        .max_w(px(512.0))
        .columns(4)
        .hotkeys(true);
    for sound in &DEMO_SOUNDS {
        let mode = mode_of(sound.id);
        let player = use_sound(
            format!("{base}/{}/sound", sound.id),
            sound,
            mode != SoundPadMode::OneShot,
            window,
            cx,
        );
        let (playing, progress) = {
            let p = player.read(cx);
            (p.state().playing, p.progress_source())
        };
        let (l, c, h) = sound.accent;
        let (trigger, stopper) = (player.clone(), player);
        grid = grid.child(
            SoundPad::new(format!("{base}/{}", sound.id))
                .accent(oklch(l, c, h))
                .hotkey(sound.hotkey)
                .loading(false)
                .mode(mode)
                .on_stop(move |window, cx| stop(&stopper, window, cx))
                .on_trigger(move |window, cx| play(&trigger, window, cx))
                .playing(playing)
                .child(SoundPadLabel::new().child(sound.label))
                .child(SoundPadShortcut::new())
                .child(SoundPadProgress::new().source(progress).variant(
                    if mode == SoundPadMode::OneShot {
                        SoundPadProgressVariant::Bar
                    } else {
                        SoundPadProgressVariant::Ring
                    },
                )),
        );
    }
    grid.into_any_element()
}
