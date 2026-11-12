//! Gallery page for `audio-player`. One variant per docs example:
//! `components/examples/audio-player-demo.tsx`, `components/examples/audio-player-compact.tsx`.
//!
//! The docs examples play synthesised demo tracks (`useDemoTracks`); here the
//! same tracks (titles, artists, durations) play through a `SimulatedPlayer`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::{Icon, IconName};
use audiocn::{
    AudioPlayer, AudioPlayerControls, AudioPlayerDescription, AudioPlayerPlay,
    AudioPlayerPlayState, AudioPlayerRate, AudioPlayerSeek, AudioPlayerSkipBack,
    AudioPlayerSkipForward, AudioPlayerTime, AudioPlayerTimeType, AudioPlayerTitle,
    AudioPlayerVolume, UseAudioPlayerOptions, use_simulated_player,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "audio-player",
    title: "Audio Player",
    description: "A composable audio player with transport, seeking, time, volume, rate, loop and keyboard shortcuts.",
    variants: &["default", "compact"],
    render,
};

/// `DEMO_TRACKS` of `lib/docs/demo-audio.ts`.
pub struct DemoTrack {
    pub id: &'static str,
    pub title: &'static str,
    pub artist: &'static str,
    pub duration: f64,
}

pub const DEMO_TRACKS: [DemoTrack; 3] = [
    DemoTrack {
        id: "night-drive",
        title: "Night Drive",
        artist: "audiocn",
        duration: 19.2,
    },
    DemoTrack {
        id: "low-tide",
        title: "Low Tide",
        artist: "audiocn",
        duration: 24.0,
    },
    DemoTrack {
        id: "arcade",
        title: "Arcade",
        artist: "audiocn",
        duration: 13.7,
    },
];

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "compact" => compact(window, cx),
        _ => return None,
    })
}

/// `({ playing }) => playing ? <PauseIcon weight="fill" /> : <PlayIcon weight="fill" />`.
fn play_icon(state: AudioPlayerPlayState) -> AnyElement {
    Icon::new(if state.playing {
        IconName::Pause
    } else {
        IconName::Play
    })
    .fill()
    .into_any_element()
}

/// `audio-player-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let track = &DEMO_TRACKS[0];
    let player = use_simulated_player(
        format!("{}/player", id("audio-player", "default")),
        track.duration,
        UseAudioPlayerOptions::src(track.id),
        window,
        cx,
    );
    let t = cx.theme().clone();
    AudioPlayer::new(id("audio-player", "default"))
        .player(player)
        .w_full()
        .max_w(px(448.0))
        .flex_col()
        .items_stretch()
        .rounded(t.radius_xl())
        .border_1()
        .border_color(t.border)
        .p(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .size(px(48.0))
                        .items_center()
                        .justify_center()
                        .rounded(t.radius_lg())
                        .bg(t.muted)
                        .text_color(t.muted_foreground)
                        .child(Icon::new(IconName::MusicNotes).size(px(20.0))),
                )
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .flex_col()
                        .child(AudioPlayerTitle::new().child(track.title))
                        .child(AudioPlayerDescription::new().child(track.artist)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(AudioPlayerTime::new())
                .child(AudioPlayerSeek::new())
                .child(AudioPlayerTime::new().r#type(AudioPlayerTimeType::Remaining)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    AudioPlayerControls::new()
                        .child(AudioPlayerSkipBack::new().child(Icon::new(IconName::SkipBack)))
                        .child(AudioPlayerPlay::new().children_with(play_icon))
                        .child(
                            AudioPlayerSkipForward::new().child(Icon::new(IconName::SkipForward)),
                        ),
                )
                .child(AudioPlayerVolume::new()),
        )
        .into_any_element()
}

/// `audio-player-compact.tsx`.
fn compact(window: &mut Window, cx: &mut App) -> AnyElement {
    let track = &DEMO_TRACKS[1];
    let player = use_simulated_player(
        format!("{}/player", id("audio-player", "compact")),
        track.duration,
        UseAudioPlayerOptions::src(track.id),
        window,
        cx,
    );
    let t = cx.theme().clone();
    AudioPlayer::new(id("audio-player", "compact"))
        .player(player)
        .w_full()
        .max_w(px(448.0))
        .rounded_full()
        .border_1()
        .border_color(t.border)
        .py(px(4.0))
        .pr(px(12.0))
        .pl(px(4.0))
        .child(
            AudioPlayerPlay::new()
                .size(px(32.0))
                .children_with(play_icon),
        )
        .child(AudioPlayerSeek::new())
        .child(AudioPlayerTime::new().r#type(AudioPlayerTimeType::Remaining))
        .child(AudioPlayerRate::new())
        .into_any_element()
}
