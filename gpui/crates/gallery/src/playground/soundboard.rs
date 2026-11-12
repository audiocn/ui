//! Playground page for `Soundboard`: pad data sets (8 demo / 3 custom / 12 /
//! empty) and per-pad fields, controlled and uncontrolled pad lists, host
//! voices, columns, and every callback. Also home of the sound clips the
//! mixer page shares.

use audiocn::motion::Instant;
use audiocn::theme::oklch;
use audiocn::{FrameEmitter, SoundPadMode, Soundboard, SoundboardSound, SoundboardVoice};
use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "soundboard",
    component: "Soundboard",
    source: SourceUse::None,
    controls,
    render,
};

/// One clip: id, label, hotkey, accent (oklch l, c, h), length in seconds.
pub(super) type Clip = (
    &'static str,
    &'static str,
    &'static str,
    (f32, f32, f32),
    f64,
);

/// `DEMO_SOUNDS` of `lib/docs/demo-audio.ts`, with each clip's length.
pub(super) const DEMO_CLIPS: [Clip; 8] = [
    ("airhorn", "Airhorn", "1", (0.7, 0.2, 30.0), 1.4),
    ("applause", "Applause", "2", (0.75, 0.15, 85.0), 2.4),
    ("ding", "Ding", "3", (0.7, 0.15, 250.0), 1.7),
    ("laser", "Laser", "4", (0.7, 0.2, 320.0), 0.5),
    ("success", "Success", "5", (0.72, 0.17, 150.0), 1.3),
    ("fail", "Fail", "6", (0.65, 0.2, 20.0), 2.0),
    ("whoosh", "Whoosh", "7", (0.7, 0.1, 200.0), 1.2),
    ("drumroll", "Drum roll", "8", (0.68, 0.16, 60.0), 2.8),
];

/// Clips that are not docs-demo ids (so a missing duration leaves them loading).
pub(super) const CUSTOM_CLIPS: [Clip; 4] = [
    ("intro-sting", "Intro sting", "q", (0.7, 0.14, 280.0), 3.2),
    ("crowd-laugh", "Crowd laugh", "w", (0.74, 0.12, 110.0), 4.5),
    ("rimshot", "Rimshot", "e", (0.68, 0.18, 0.0), 0.8),
    (
        "long-ambience",
        "Long ambience bed",
        "r",
        (0.66, 0.08, 180.0),
        12.0,
    ),
];

/// Sound data sets, shared by the mixer page. The expression is a placeholder.
pub(super) const SOUND_SETS: &[(&str, &str)] = &[
    ("8 demo sounds", "sounds"),
    ("3 custom sounds", "sounds"),
    ("12 sounds", "sounds"),
    ("empty", "Vec::new()"),
];

pub(super) fn clips(label: &str) -> Vec<Clip> {
    match label {
        "8 demo sounds" => DEMO_CLIPS.to_vec(),
        "3 custom sounds" => CUSTOM_CLIPS[..3].to_vec(),
        "12 sounds" => DEMO_CLIPS.iter().chain(&CUSTOM_CLIPS).copied().collect(),
        _ => Vec::new(),
    }
}

const MODES: &[(&str, &str)] = &[
    ("unset (one shot)", ""),
    ("one shot", "SoundPadMode::OneShot"),
    ("toggle", "SoundPadMode::Toggle"),
    ("hold", "SoundPadMode::Hold"),
    ("loop", "SoundPadMode::Loop"),
];

const VOICES: &[(&str, &str)] = &[
    ("simulated (no voice)", ""),
    ("host: idle", ""),
    ("host: loading", ""),
    ("host: playing", ""),
];

fn controls() -> Vec<Control> {
    vec![
        // Sounds
        Control::bool("controlled", "Controlled", false)
            .group("Sounds")
            .no_code()
            .remount()
            .help("On: .sounds(..) kept by the page from on_sounds_change; off: default_sounds."),
        Control::choice("sound_set", "Sounds", 0, SOUND_SETS)
            .group("Sounds")
            .method("default_sounds")
            .remount(),
        Control::num("columns", "Columns", 4.0, 1.0, 8.0, 1.0).group("Sounds"),
        // Per pad
        Control::bool("with_hotkeys", "Hotkeys", true)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .remount(),
        Control::bool("with_accents", "Accents", true)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .remount(),
        Control::bool("with_durations", "Durations", true)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .remount()
            .help("Off: demo ids use their known length; other pads stay loading."),
        Control::choice("mode", "Mode (all pads)", 0, MODES)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .remount(),
        Control::num("volume", "Volume (all pads)", 1.0, 0.0, 1.0, 0.05)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .remount(),
        Control::choice("voice", "First pad voice", 0, VOICES)
            .group("Per pad (SoundboardSound)")
            .no_code()
            .help("Host-owned playback (SoundboardVoice) on the first pad."),
        // Layout
        Control::px("max_width", "Max width (px)", 672.0, 280.0, 1100.0, 8.0)
            .group("Layout")
            .method("max_w"),
    ]
}

fn sound_of(clip: Clip, p: &Props) -> SoundboardSound {
    let (id, label, hotkey, (l, c, h), seconds) = clip;
    let mut sound = SoundboardSound::new(id, label, id);
    if p.bool("with_hotkeys") {
        sound = sound.hotkey(hotkey);
    }
    if p.bool("with_accents") {
        sound = sound.accent(oklch(l, c, h));
    }
    if p.bool("with_durations") {
        sound = sound.duration(seconds);
    }
    sound.mode = match p.choice("mode") {
        "one shot" => Some(SoundPadMode::OneShot),
        "toggle" => Some(SoundPadMode::Toggle),
        "hold" => Some(SoundPadMode::Hold),
        "loop" => Some(SoundPadMode::Loop),
        _ => None,
    };
    if p.num("volume") < 1.0 {
        sound = sound.volume(p.num("volume"));
    }
    sound
}

/// The page's copy of the pads (controlled mode) and the host voice clock.
struct Host {
    sounds: Vec<SoundboardSound>,
    progress: FrameEmitter<f32>,
    started: Instant,
}

fn sounds_detail(sounds: &[SoundboardSound]) -> String {
    let ids: Vec<&str> = sounds.iter().map(|s| s.id.as_ref()).collect();
    format!("{} pads: {}", sounds.len(), ids.join(", "))
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let seed: Vec<SoundboardSound> = clips(p.choice("sound_set"))
        .into_iter()
        .map(|clip| sound_of(clip, p))
        .collect();
    let host = {
        let seed = seed.clone();
        window.use_keyed_state(ctx.id("host"), cx, move |_, _| Host {
            sounds: seed,
            progress: FrameEmitter::new(),
            started: Instant::now(),
        })
    };

    let mut sounds = if controlled {
        host.read(cx).sounds.clone()
    } else {
        seed
    };
    // Host voice on the first pad.
    let voice = match p.choice("voice") {
        "host: idle" => Some((false, false)),
        "host: loading" => Some((false, true)),
        "host: playing" => Some((true, false)),
        _ => None,
    };
    if let Some((playing, loading)) = voice {
        let (progress, started) = {
            let h = host.read(cx);
            (h.progress.clone(), h.started)
        };
        if playing {
            let t = started.elapsed().as_secs_f32();
            progress.emit((t / 3.0).fract());
            window.request_animation_frame();
        } else {
            progress.emit(0.0);
        }
        if let Some(first) = sounds.first_mut() {
            first.voice = Some(SoundboardVoice {
                playing,
                loading,
                progress: Some(progress.source()),
            });
        }
    }

    let mut board = Soundboard::new(ctx.id("soundboard")).columns(p.num("columns") as usize);
    board = if controlled {
        board.sounds(sounds)
    } else {
        board.default_sounds(sounds)
    };
    let (c_change, c_play, c_stop, c_gain) = (ctx.clone(), ctx.clone(), ctx.clone(), ctx.clone());
    board
        .on_sounds_change(move |sounds, _, cx| {
            c_change.log("on_sounds_change", sounds_detail(&sounds), cx);
            if controlled {
                let mut sounds = sounds;
                // The voice is re-applied from the panel on every render.
                for sound in &mut sounds {
                    sound.voice = None;
                }
                host.update(cx, |h, cx| {
                    h.sounds = sounds;
                    cx.notify();
                });
            }
        })
        .on_play(move |sound, _, cx| {
            c_play.log(
                "on_play",
                format!(
                    "{} (src {}, {:?})",
                    sound.label,
                    sound.src,
                    sound.mode.unwrap_or_default()
                ),
                cx,
            );
        })
        .on_stop(move |id, _, cx| c_stop.log("on_stop", id.to_string(), cx))
        .on_output_gain(move |gain, _, cx| {
            c_gain.log("on_output_gain", format!("{gain:.3}"), cx);
        })
        .w_full()
        .max_w(px(p.num("max_width")))
        .into_any_element()
}
