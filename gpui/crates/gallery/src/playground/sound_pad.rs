//! Playground page for `SoundPad` and `SoundPadGrid`: modes, progress, hotkeys.

use audiocn::theme::oklch;
use audiocn::ui::icons::IconName;
use audiocn::{
    AudioPlayerController, HotkeyScope, SoundPad, SoundPadGrid, SoundPadIcon, SoundPadLabel,
    SoundPadMode, SoundPadProgress, SoundPadProgressVariant, SoundPadShortcut, SoundPadSize,
    SoundPadVariant, UseAudioPlayerOptions, use_simulated_player,
};
use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "sound-pad",
    component: "SoundPad",
    source: SourceUse::None,
    controls,
    render,
};

struct Sound {
    id: &'static str,
    label: &'static str,
    hotkey: &'static str,
    accent: (f32, f32, f32),
    seconds: f64,
    icon: IconName,
    mode: SoundPadMode,
}

const SOUNDS: [Sound; 8] = [
    Sound {
        id: "airhorn",
        label: "Airhorn",
        hotkey: "1",
        accent: (0.7, 0.2, 30.0),
        seconds: 1.4,
        icon: IconName::Megaphone,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "applause",
        label: "Applause",
        hotkey: "2",
        accent: (0.75, 0.15, 85.0),
        seconds: 2.4,
        icon: IconName::MusicNotes,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "ding",
        label: "Ding",
        hotkey: "3",
        accent: (0.7, 0.15, 250.0),
        seconds: 1.7,
        icon: IconName::Bell,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "laser",
        label: "Laser",
        hotkey: "4",
        accent: (0.7, 0.2, 320.0),
        seconds: 0.5,
        icon: IconName::Waveform,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "success",
        label: "Success",
        hotkey: "5",
        accent: (0.72, 0.17, 150.0),
        seconds: 1.3,
        icon: IconName::Check,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "fail",
        label: "Fail",
        hotkey: "6",
        accent: (0.65, 0.2, 20.0),
        seconds: 2.0,
        icon: IconName::X,
        mode: SoundPadMode::OneShot,
    },
    Sound {
        id: "whoosh",
        label: "Whoosh",
        hotkey: "7",
        accent: (0.7, 0.1, 200.0),
        seconds: 1.2,
        icon: IconName::SpeakerHigh,
        mode: SoundPadMode::Toggle,
    },
    Sound {
        id: "drumroll",
        label: "Drum roll",
        hotkey: "8",
        accent: (0.68, 0.16, 60.0),
        seconds: 2.8,
        icon: IconName::Headphones,
        mode: SoundPadMode::Hold,
    },
];

const LAYOUTS: &[(&str, &str)] = &[("pad", ""), ("grid", "")];

const MODES: &[(&str, &str)] = &[
    ("one-shot", "SoundPadMode::OneShot"),
    ("toggle", "SoundPadMode::Toggle"),
    ("hold", "SoundPadMode::Hold"),
    ("loop", "SoundPadMode::Loop"),
];

const VARIANTS: &[(&str, &str)] = &[
    ("default", "SoundPadVariant::Default"),
    ("outline", "SoundPadVariant::Outline"),
    ("ghost", "SoundPadVariant::Ghost"),
];

const SIZES: &[(&str, &str)] = &[
    ("default", "SoundPadSize::Default"),
    ("sm", "SoundPadSize::Sm"),
    ("lg", "SoundPadSize::Lg"),
];

const PROGRESS: &[(&str, &str)] = &[
    ("bar", "SoundPadProgressVariant::Bar"),
    ("fill", "SoundPadProgressVariant::Fill"),
    ("ring", "SoundPadProgressVariant::Ring"),
];

const PROGRESS_FROM: &[(&str, &str)] = &[("player", ""), ("value", ""), ("hidden", "")];

const SCOPES: &[(&str, &str)] = &[
    ("focus", "HotkeyScope::Focus"),
    ("global", "HotkeyScope::Global"),
];

const ICONS: &[(&str, &str)] = &[
    ("megaphone", ""),
    ("music", ""),
    ("bell", ""),
    ("waveform", ""),
    ("check", ""),
    ("speaker", ""),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("layout", "Layout", 0, LAYOUTS)
            .group("Layout")
            .no_code()
            .help("Pad is one sound. Grid is eight, inside SoundPadGrid."),
        Control::choice("mode", "Mode", 0, MODES).group("Pad"),
        Control::bool("per_sound_modes", "Per-sound modes", false)
            .group("Pad")
            .no_code()
            .help("Grid only: whoosh toggles, drum roll holds, the rest are one-shot."),
        Control::choice("variant", "Variant", 0, VARIANTS).group("Pad"),
        Control::choice("size", "Size", 0, SIZES).group("Pad"),
        Control::color("accent", "Accent")
            .group("Pad")
            .help("Default keeps each sound's own colour."),
        Control::bool("loading", "Loading", false).group("Pad"),
        Control::bool("disabled", "Disabled", false).group("Pad"),
        Control::bool("playing", "Playing", false)
            .group("Pad")
            .help("Forces the playing style. Triggering a pad sets it too."),
        Control::text("hotkey", "Hotkey", "1")
            .group("Pad")
            .help("Single pad. The grid uses keys 1–8."),
        Control::text("label", "Label", "Airhorn")
            .group("Parts")
            .no_code()
            .help("Single pad. The grid uses each sound's name."),
        Control::bool("show_label", "Label", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_icon", "Icon", true)
            .group("Parts")
            .no_code(),
        Control::choice("icon", "Icon", 0, ICONS)
            .group("Parts")
            .no_code()
            .help("Single pad. The grid uses an icon per sound."),
        Control::bool("show_shortcut", "Shortcut", true)
            .group("Parts")
            .no_code(),
        Control::choice("progress_from", "Progress", 0, PROGRESS_FROM)
            .group("Progress")
            .no_code()
            .help("Player follows the simulated voice. Value is a fixed 0..1."),
        Control::choice("progress_variant", "Progress style", 0, PROGRESS)
            .group("Progress")
            .no_code(),
        Control::num("progress", "Progress value", 0.4, 0.0, 1.0, 0.05)
            .group("Progress")
            .no_code(),
        Control::num("columns", "Columns", 4.0, 1.0, 8.0, 1.0)
            .group("Grid")
            .no_code()
            .help("SoundPadGrid::columns. The grid drops columns to keep a minimum pad width."),
        Control::bool("hotkeys", "Hotkeys", false)
            .group("Grid")
            .no_code()
            .help("SoundPadGrid::hotkeys. Default off."),
        Control::choice("hotkey_scope", "Hotkey scope", 0, SCOPES)
            .group("Grid")
            .no_code(),
        Control::px("pad_min_width", "Pad min width", 88.0, 48.0, 240.0, 4.0)
            .group("Grid")
            .no_code(),
        Control::px("pad_gap", "Pad gap", 8.0, 0.0, 32.0, 1.0)
            .group("Grid")
            .no_code(),
    ]
}

fn mode_of(label: &str) -> SoundPadMode {
    match label {
        "toggle" => SoundPadMode::Toggle,
        "hold" => SoundPadMode::Hold,
        "loop" => SoundPadMode::Loop,
        _ => SoundPadMode::OneShot,
    }
}

fn variant_of(label: &str) -> SoundPadVariant {
    match label {
        "outline" => SoundPadVariant::Outline,
        "ghost" => SoundPadVariant::Ghost,
        _ => SoundPadVariant::Default,
    }
}

fn size_of(label: &str) -> SoundPadSize {
    match label {
        "sm" => SoundPadSize::Sm,
        "lg" => SoundPadSize::Lg,
        _ => SoundPadSize::Default,
    }
}

fn progress_variant(label: &str) -> SoundPadProgressVariant {
    match label {
        "fill" => SoundPadProgressVariant::Fill,
        "ring" => SoundPadProgressVariant::Ring,
        _ => SoundPadProgressVariant::Bar,
    }
}

fn scope_of(label: &str) -> HotkeyScope {
    match label {
        "global" => HotkeyScope::Global,
        _ => HotkeyScope::Focus,
    }
}

fn icon_of(label: &str) -> IconName {
    match label {
        "music" => IconName::MusicNotes,
        "bell" => IconName::Bell,
        "waveform" => IconName::Waveform,
        "check" => IconName::Check,
        "speaker" => IconName::SpeakerHigh,
        _ => IconName::Megaphone,
    }
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let sounds: Vec<&Sound> = if p.choice("layout") == "grid" {
        SOUNDS.iter().collect()
    } else {
        vec![&SOUNDS[0]]
    };
    let mut grid = SoundPadGrid::new(ctx.id("grid"))
        .columns(p.num("columns") as usize)
        .hotkeys(p.bool("hotkeys"))
        .hotkey_scope(scope_of(p.choice("hotkey_scope")))
        .pad_gap(px(p.num("pad_gap")))
        .pad_min_width(px(p.num("pad_min_width")))
        .w_full();
    for sound in sounds {
        grid = grid.child(pad(p, ctx, sound, window, cx));
    }
    grid.into_any_element()
}

fn pad(p: &Props, ctx: &Ctx, sound: &Sound, window: &mut Window, cx: &mut App) -> SoundPad {
    let single = p.choice("layout") != "grid";
    let mode = if !single && p.bool("per_sound_modes") {
        sound.mode
    } else {
        mode_of(p.choice("mode"))
    };
    let player = use_simulated_player(
        ctx.id(&format!("player-{}", sound.id)),
        sound.seconds,
        UseAudioPlayerOptions {
            r#loop: mode != SoundPadMode::OneShot,
            ..UseAudioPlayerOptions::src(sound.id)
        },
        window,
        cx,
    );
    let (simulated, progress) = {
        let state = player.read(cx);
        (state.state().playing, state.progress_source())
    };
    let trigger = player.clone();
    let stop_player = player;
    let trigger_log = ctx.clone();
    let stop_log = ctx.clone();
    let id = sound.id;
    let accent = p
        .color("accent", cx)
        .unwrap_or_else(|| oklch(sound.accent.0, sound.accent.1, sound.accent.2));
    let hotkey = if single {
        p.text("hotkey")
    } else {
        sound.hotkey.into()
    };
    let label = if single {
        p.text("label")
    } else {
        sound.label.into()
    };
    let icon = if single {
        icon_of(p.choice("icon"))
    } else {
        sound.icon
    };

    let mut pad = SoundPad::new(ctx.id(sound.id))
        .accent(accent)
        .disabled(p.bool("disabled"))
        .hotkey(hotkey)
        .loading(p.bool("loading"))
        .mode(mode)
        .on_stop(move |window, cx| {
            stop(&stop_player, window, cx);
            stop_log.log("on_stop", id, cx);
        })
        .on_trigger(move |window, cx| {
            play(&trigger, window, cx);
            trigger_log.log("on_trigger", id, cx);
        })
        .playing(p.bool("playing") || simulated)
        .size(size_of(p.choice("size")))
        .variant(variant_of(p.choice("variant")));
    if single {
        pad = pad.w(px(160.0));
    }
    if p.bool("show_icon") {
        pad = pad.child(SoundPadIcon::new().icon(icon));
    }
    if p.bool("show_label") && !label.is_empty() {
        pad = pad.child(SoundPadLabel::new().child(label));
    }
    if p.bool("show_shortcut") {
        pad = pad.child(SoundPadShortcut::new());
    }
    if let Some(progress) = progress_part(p, progress) {
        pad = pad.child(progress);
    }
    pad
}

fn progress_part(p: &Props, source: audiocn::FrameSource<f32>) -> Option<SoundPadProgress> {
    let variant = progress_variant(p.choice("progress_variant"));
    match p.choice("progress_from") {
        "hidden" => None,
        "value" => Some(
            SoundPadProgress::new()
                .value(p.num("progress"))
                .variant(variant),
        ),
        _ => Some(SoundPadProgress::new().source(source).variant(variant)),
    }
}

fn play(player: &Entity<audiocn::SimulatedPlayer>, window: &mut Window, cx: &mut App) {
    player.seek(0.0, window, cx);
    player.play(window, cx);
}

fn stop(player: &Entity<audiocn::SimulatedPlayer>, window: &mut Window, cx: &mut App) {
    player.pause(window, cx);
    player.seek(0.0, window, cx);
}
