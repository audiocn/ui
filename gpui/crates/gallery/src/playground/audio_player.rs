//! Playground page for `AudioPlayer`: simulated tracks, every part, every callback.

use std::cell::Cell;
use std::rc::Rc;

use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::{Icon, IconName};
use audiocn::{
    AudioPlayer, AudioPlayerArtwork, AudioPlayerControls, AudioPlayerDescription, AudioPlayerLoop,
    AudioPlayerNext, AudioPlayerPlay, AudioPlayerPlayState, AudioPlayerPrevious, AudioPlayerRate,
    AudioPlayerSeek, AudioPlayerSkipBack, AudioPlayerSkipForward, AudioPlayerTime,
    AudioPlayerTimeType, AudioPlayerTitle, AudioPlayerVolume, AudioPreload, AudioSize, CrossOrigin,
    Orientation, UseAudioPlayerOptions, VolumeCurve, use_simulated_player,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec, Value};

pub const SPEC: Spec = Spec {
    slug: "audio-player",
    component: "AudioPlayer",
    source: SourceUse::None,
    controls,
    render,
};

struct Track {
    label: &'static str,
    src: &'static str,
    title: &'static str,
    artist: &'static str,
    duration: f64,
}

const TRACKS: [Track; 5] = [
    Track {
        label: "Night Drive (19.2s)",
        src: "night-drive",
        title: "Night Drive",
        artist: "audiocn",
        duration: 19.2,
    },
    Track {
        label: "Low Tide (24.0s)",
        src: "low-tide",
        title: "Low Tide",
        artist: "audiocn",
        duration: 24.0,
    },
    Track {
        label: "Arcade (13.7s)",
        src: "arcade",
        title: "Arcade",
        artist: "audiocn",
        duration: 13.7,
    },
    Track {
        label: "Short (3.0s)",
        src: "short",
        title: "Short",
        artist: "audiocn",
        duration: 3.0,
    },
    Track {
        label: "Long (3:00)",
        src: "long",
        title: "Long",
        artist: "audiocn",
        duration: 180.0,
    },
];

const PRELOADS: &[(&str, &str)] = &[
    ("metadata", "AudioPreload::Metadata"),
    ("none", "AudioPreload::None"),
    ("auto", "AudioPreload::Auto"),
];

const ORIGINS: &[(&str, &str)] = &[
    ("unset", ""),
    ("anonymous", "CrossOrigin::Anonymous"),
    ("use-credentials", "CrossOrigin::UseCredentials"),
];

const TIME_FORMATS: &[(&str, &str)] = &[("default", ""), ("seconds", ""), ("tenths", "")];

const RATES: &[(&str, &str)] = &[
    ("default", ""),
    ("integers", "vec![0.5, 1.0, 1.5, 2.0]"),
    ("slow", "vec![0.25, 0.5, 0.75, 1.0]"),
];

const CURVES: &[(&str, &str)] = &[
    ("perceptual", "VolumeCurve::Perceptual"),
    ("linear", "VolumeCurve::Linear"),
];

const ORIENTATIONS: &[(&str, &str)] = &[
    ("unset", ""),
    ("horizontal", "Orientation::Horizontal"),
    ("vertical", "Orientation::Vertical"),
];

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

const SRC: &[(&str, &str)] = &[
    ("Night Drive (19.2s)", "\"night-drive\""),
    ("Low Tide (24.0s)", "\"low-tide\""),
    ("Arcade (13.7s)", "\"arcade\""),
    ("Short (3.0s)", "\"short\""),
    ("Long (3:00)", "\"long\""),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("src", "Track", 0, SRC)
            .group("Player")
            .help("Duration preset. Previous and next cycle it."),
        Control::bool("auto_play", "Auto play", false)
            .group("Player")
            .remount()
            .help("Applies when the track loads."),
        Control::bool("loop", "Loop", false)
            .group("Player")
            .method("r#loop"),
        Control::num("volume", "Volume", 1.0, 0.0, 1.0, 0.01).group("Player"),
        Control::bool("muted", "Muted", false).group("Player"),
        Control::num("playback_rate", "Playback rate", 1.0, 0.25, 2.0, 0.25).group("Player"),
        Control::choice("preload", "Preload", 0, PRELOADS).group("Player"),
        Control::choice("cross_origin", "Cross origin", 0, ORIGINS)
            .group("Player")
            .method("cross_origin"),
        Control::bool("shortcuts", "Shortcuts", true)
            .group("Player")
            .help("Space, arrows and M while focus is inside."),
        Control::bool("compact", "Compact", false)
            .group("Layout")
            .no_code()
            .help("One row: the parts you leave on."),
        Control::bool("show_artwork", "Artwork", true)
            .group("Parts")
            .no_code(),
        Control::text("artwork_src", "Artwork src", "")
            .group("Parts")
            .no_code()
            .help("Empty uses the demo icon. A URL uses AudioPlayerArtwork."),
        Control::bool("show_title", "Title", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_description", "Description", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_controls", "Controls", true)
            .group("Transport")
            .no_code(),
        Control::bool("show_previous", "Previous", false)
            .group("Transport")
            .no_code(),
        Control::bool("show_skip_back", "Skip back", true)
            .group("Transport")
            .no_code(),
        Control::bool("show_play", "Play", true)
            .group("Transport")
            .no_code(),
        Control::bool("show_skip_forward", "Skip forward", true)
            .group("Transport")
            .no_code(),
        Control::bool("show_next", "Next", false)
            .group("Transport")
            .no_code(),
        Control::num("skip_seconds", "Skip seconds", 10.0, 1.0, 60.0, 1.0)
            .group("Transport")
            .no_code(),
        Control::bool("show_seek", "Seek", true)
            .group("Seek")
            .no_code(),
        Control::num("seek_step", "Seek step", 5.0, 1.0, 30.0, 1.0)
            .group("Seek")
            .no_code(),
        Control::num("seek_large_step", "Seek large step", 15.0, 1.0, 60.0, 1.0)
            .group("Seek")
            .no_code(),
        Control::bool("show_time_current", "Time current", true)
            .group("Time")
            .no_code(),
        Control::bool("show_time_remaining", "Time remaining", true)
            .group("Time")
            .no_code(),
        Control::bool("show_time_duration", "Time duration", false)
            .group("Time")
            .no_code(),
        Control::choice("time_format", "Time format", 0, TIME_FORMATS)
            .group("Time")
            .no_code(),
        Control::bool("show_volume", "Volume", true)
            .group("Volume")
            .no_code(),
        Control::num("volume_step", "Volume step", 0.05, 0.01, 0.25, 0.01)
            .group("Volume")
            .no_code(),
        Control::choice("volume_curve", "Volume curve", 0, CURVES)
            .group("Volume")
            .no_code(),
        Control::choice("volume_orientation", "Volume orientation", 0, ORIENTATIONS)
            .group("Volume")
            .no_code(),
        Control::choice("volume_size", "Volume size", 0, SIZES)
            .group("Volume")
            .no_code(),
        Control::bool("volume_disabled", "Volume disabled", false)
            .group("Volume")
            .no_code(),
        Control::bool("show_rate", "Rate", false)
            .group("Rate")
            .no_code(),
        Control::choice("rates", "Rates", 0, RATES)
            .group("Rate")
            .no_code(),
        Control::bool("show_loop", "Loop button", false)
            .group("Loop")
            .no_code(),
    ]
}

fn track_of(label: &str) -> &'static Track {
    TRACKS
        .iter()
        .find(|track| track.label == label)
        .unwrap_or(&TRACKS[0])
}

fn preload_of(label: &str) -> AudioPreload {
    match label {
        "none" => AudioPreload::None,
        "auto" => AudioPreload::Auto,
        _ => AudioPreload::Metadata,
    }
}

fn origin_of(label: &str) -> Option<CrossOrigin> {
    match label {
        "anonymous" => Some(CrossOrigin::Anonymous),
        "use-credentials" => Some(CrossOrigin::UseCredentials),
        _ => None,
    }
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let track = track_of(p.choice("src"));
    let index = TRACKS
        .iter()
        .position(|item| item.src == track.src)
        .unwrap_or(0);
    let play = ctx.clone();
    let pause = ctx.clone();
    let ended = ctx.clone();
    let error = ctx.clone();
    let options = UseAudioPlayerOptions {
        src: Some(track.src.into()),
        auto_play: p.bool("auto_play"),
        r#loop: p.bool("loop"),
        volume: p.num("volume"),
        muted: p.bool("muted"),
        playback_rate: p.num("playback_rate"),
        preload: preload_of(p.choice("preload")),
        cross_origin: origin_of(p.choice("cross_origin")),
        on_play: Some(Rc::new(move |_, cx| play.log("on_play", "", cx))),
        on_pause: Some(Rc::new(move |_, cx| pause.log("on_pause", "", cx))),
        on_ended: Some(Rc::new(move |_, cx| ended.log("on_ended", "", cx))),
        on_error: Some(Rc::new(move |message, _, cx| {
            error.log(
                "on_error",
                message.unwrap_or_else(|| SharedString::from("none")),
                cx,
            );
        })),
    };
    let player = use_simulated_player(ctx.id("player"), track.duration, options, window, cx);

    let clock = window.use_keyed_state(ctx.id("time-log"), cx, |_, _| Rc::new(Cell::new(-1i64)));
    let clock = clock.read(cx).clone();
    let time_log = ctx.clone();
    let previous = step_track(ctx, index, -1, "on_previous");
    let next = step_track(ctx, index, 1, "on_next");

    let mut root = AudioPlayer::new(ctx.id("root"))
        .player(player)
        .src(track.src)
        .auto_play(p.bool("auto_play"))
        .r#loop(p.bool("loop"))
        .volume(p.num("volume"))
        .muted(p.bool("muted"))
        .playback_rate(p.num("playback_rate"))
        .preload(preload_of(p.choice("preload")))
        .shortcuts(p.bool("shortcuts"))
        .on_time_update(move |time, _, cx| {
            let second = time.floor() as i64;
            if clock.get() != second {
                clock.set(second);
                time_log.log("on_time_update", format!("{second}s"), cx);
            }
        })
        .on_previous(previous)
        .on_next(next)
        .w_full();
    if let Some(origin) = origin_of(p.choice("cross_origin")) {
        root = root.cross_origin(origin);
    }

    let theme = cx.theme().clone();
    let compact = p.bool("compact");
    root = if compact {
        root.rounded_full()
            .border_1()
            .border_color(theme.border)
            .py(px(4.0))
            .pr(px(12.0))
            .pl(px(4.0))
    } else {
        root.flex_col()
            .items_stretch()
            .rounded(theme.radius_xl())
            .border_1()
            .border_color(theme.border)
            .p(px(16.0))
    };

    let parts = parts(p, track, &theme);
    if compact {
        root.children(parts.into_iter().map(|part| part.element))
            .into_any_element()
    } else {
        root.children(rows(parts)).into_any_element()
    }
}

fn step_track(
    ctx: &Ctx,
    index: usize,
    delta: isize,
    event: &'static str,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let ctx = ctx.clone();
    move |_, cx| {
        let count = TRACKS.len() as isize;
        let next = (index as isize + delta).rem_euclid(count) as usize;
        ctx.set("src", Value::Choice(next), cx);
        ctx.log(event, TRACKS[next].title, cx);
    }
}

enum Slot {
    Meta,
    Timeline,
    Transport,
}

struct Part {
    slot: Slot,
    element: AnyElement,
}

fn parts(p: &Props, track: &Track, theme: &audiocn::Theme) -> Vec<Part> {
    let mut parts = Vec::new();
    if p.bool("show_artwork") || p.bool("show_title") || p.bool("show_description") {
        parts.push(Part {
            slot: Slot::Meta,
            element: meta(p, track, theme),
        });
    }
    if p.bool("show_time_current") {
        parts.push(Part {
            slot: Slot::Timeline,
            element: time_view(AudioPlayerTimeType::Current, p.choice("time_format"))
                .into_any_element(),
        });
    }
    if p.bool("show_seek") {
        parts.push(Part {
            slot: Slot::Timeline,
            element: AudioPlayerSeek::new()
                .id("pg-seek")
                .step(p.num("seek_step") as f64)
                .large_step(p.num("seek_large_step") as f64)
                .flex_1()
                .min_w(px(48.0))
                .into_any_element(),
        });
    }
    if p.bool("show_time_remaining") {
        parts.push(Part {
            slot: Slot::Timeline,
            element: time_view(AudioPlayerTimeType::Remaining, p.choice("time_format"))
                .into_any_element(),
        });
    }
    if p.bool("show_time_duration") {
        parts.push(Part {
            slot: Slot::Timeline,
            element: time_view(AudioPlayerTimeType::Duration, p.choice("time_format"))
                .into_any_element(),
        });
    }
    if let Some(transport) = transport(p) {
        parts.push(Part {
            slot: Slot::Transport,
            element: transport,
        });
    }
    if p.bool("show_volume") {
        parts.push(Part {
            slot: Slot::Transport,
            element: volume(p).into_any_element(),
        });
    }
    if p.bool("show_rate") {
        parts.push(Part {
            slot: Slot::Transport,
            element: rate(p).into_any_element(),
        });
    }
    if p.bool("show_loop") {
        parts.push(Part {
            slot: Slot::Transport,
            element: AudioPlayerLoop::new().into_any_element(),
        });
    }
    parts
}

fn meta(p: &Props, track: &Track, theme: &audiocn::Theme) -> AnyElement {
    let mut row = div().flex().min_w_0().flex_1().items_center().gap(px(12.0));
    if p.bool("show_artwork") {
        let src = p.text("artwork_src");
        row = if src.is_empty() {
            row.child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .size(px(48.0))
                    .items_center()
                    .justify_center()
                    .rounded(theme.radius_lg())
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(Icon::new(IconName::MusicNotes).size(px(20.0))),
            )
        } else {
            row.child(AudioPlayerArtwork::new(src).alt(track.title))
        };
    }
    if p.bool("show_title") || p.bool("show_description") {
        let mut text = div().flex().min_w_0().flex_col();
        if p.bool("show_title") {
            text = text.child(AudioPlayerTitle::new().child(track.title));
        }
        if p.bool("show_description") {
            text = text.child(AudioPlayerDescription::new().child(track.artist));
        }
        row = row.child(text);
    }
    row.into_any_element()
}

fn time_view(kind: AudioPlayerTimeType, format: &str) -> AudioPlayerTime {
    let time = AudioPlayerTime::new().r#type(kind);
    match format {
        "seconds" => time.format(|seconds, kind| {
            let sign = if kind == AudioPlayerTimeType::Remaining {
                "−"
            } else {
                ""
            };
            format!("{sign}{seconds:.0}s")
        }),
        "tenths" => time.format(|seconds, kind| {
            let sign = if kind == AudioPlayerTimeType::Remaining {
                "−"
            } else {
                ""
            };
            format!("{sign}{seconds:.1}")
        }),
        _ => time,
    }
}

fn transport(p: &Props) -> Option<AnyElement> {
    let any = p.bool("show_controls")
        && (p.bool("show_previous")
            || p.bool("show_skip_back")
            || p.bool("show_play")
            || p.bool("show_skip_forward")
            || p.bool("show_next"));
    if !any {
        return None;
    }
    let seconds = p.num("skip_seconds") as f64;
    let mut controls = AudioPlayerControls::new();
    if p.bool("show_previous") {
        controls = controls.child(AudioPlayerPrevious::new().child(Icon::new(IconName::CaretLeft)));
    }
    if p.bool("show_skip_back") {
        controls = controls.child(
            AudioPlayerSkipBack::new()
                .seconds(seconds)
                .child(Icon::new(IconName::SkipBack)),
        );
    }
    if p.bool("show_play") {
        controls = controls.child(AudioPlayerPlay::new().children_with(play_icon));
    }
    if p.bool("show_skip_forward") {
        controls = controls.child(
            AudioPlayerSkipForward::new()
                .seconds(seconds)
                .child(Icon::new(IconName::SkipForward)),
        );
    }
    if p.bool("show_next") {
        controls = controls.child(AudioPlayerNext::new().child(Icon::new(IconName::CaretRight)));
    }
    Some(controls.into_any_element())
}

fn play_icon(state: AudioPlayerPlayState) -> AnyElement {
    Icon::new(if state.playing {
        IconName::Pause
    } else {
        IconName::Play
    })
    .fill()
    .into_any_element()
}

fn volume(p: &Props) -> AudioPlayerVolume {
    let mut volume = AudioPlayerVolume::new().step(p.num("volume_step"));
    volume = match p.choice("volume_curve") {
        "linear" => volume.curve(VolumeCurve::Linear),
        _ => volume.curve(VolumeCurve::Perceptual),
    };
    volume = match p.choice("volume_orientation") {
        "horizontal" => volume.orientation(Orientation::Horizontal),
        "vertical" => volume.orientation(Orientation::Vertical),
        _ => volume,
    };
    volume = match p.choice("volume_size") {
        "sm" => volume.size(AudioSize::Sm),
        "default" => volume.size(AudioSize::Default),
        "lg" => volume.size(AudioSize::Lg),
        _ => volume,
    };
    if p.bool("volume_disabled") {
        volume = volume.disabled(true);
    }
    volume
}

fn rate(p: &Props) -> AudioPlayerRate {
    let rate = AudioPlayerRate::new();
    match p.choice("rates") {
        "integers" => rate.rates(vec![0.5, 1.0, 1.5, 2.0]),
        "slow" => rate.rates(vec![0.25, 0.5, 0.75, 1.0]),
        _ => rate,
    }
}

/// Full layout stacks meta, the timeline and the transport. Each group is one row.
fn rows(parts: Vec<Part>) -> Vec<AnyElement> {
    let mut meta = Vec::new();
    let mut timeline = Vec::new();
    let mut transport = Vec::new();
    for part in parts {
        match part.slot {
            Slot::Meta => meta.push(part.element),
            Slot::Timeline => timeline.push(part.element),
            Slot::Transport => transport.push(part.element),
        }
    }
    let mut rows = Vec::new();
    if !meta.is_empty() {
        rows.push(row(meta));
    }
    if !timeline.is_empty() {
        rows.push(row(timeline));
    }
    if !transport.is_empty() {
        rows.push(
            div()
                .flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .children(transport)
                .into_any_element(),
        );
    }
    rows
}

fn row(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .w_full()
        .items_center()
        .gap(px(8.0))
        .children(children)
        .into_any_element()
}
