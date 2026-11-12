//! Playground page for `MusicPlayer`: playlist data sets (3 / 8 / 1 / empty)
//! and per-track fields, controlled and uncontrolled playlists, the built-in
//! vs a host player, the ducking seam (omitted / null / playground meter),
//! `output`, and every callback. Also home of the tracks the mixer page shares.

use std::cell::Cell;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use audiocn::audio::decibels::gain_to_db;
use audiocn::{
    FrameSource, MeterFrame, MusicOutput, MusicPlayer, MusicTrack, UseAudioPlayerOptions,
    use_simulated_player,
};
use gpui::{AnyElement, App, IntoElement, SharedString, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "music-player",
    component: "MusicPlayer",
    source: SourceUse::Parts,
    controls,
    render,
};

/// One track: id, title, artist, length in seconds.
pub(super) type Song = (&'static str, &'static str, &'static str, f64);

/// `DEMO_TRACKS` of `lib/docs/demo-audio.ts`, then five more.
pub(super) const SONGS: [Song; 8] = [
    ("night-drive", "Night Drive", "audiocn", 19.2),
    ("low-tide", "Low Tide", "audiocn", 24.0),
    ("arcade", "Arcade", "audiocn", 13.7),
    ("glass-city", "Glass City", "Neon Arcs", 212.0),
    ("slow-orbit", "Slow Orbit", "Kepler Tapes", 187.5),
    (
        "a-very-long-title",
        "A Very Long Title That Has To Truncate Somewhere In The Row",
        "Someone With An Equally Long Artist Name",
        64.0,
    ),
    ("tin-roof", "Tin Roof", "The Gutters", 143.0),
    ("last-light", "Last Light", "Marlow", 301.2),
];

/// Track data sets, shared by the mixer page. The expression is a placeholder.
pub(super) const TRACK_SETS: &[(&str, &str)] = &[
    ("3 demo tracks", "tracks"),
    ("8 tracks", "tracks"),
    ("1 track", "tracks"),
    ("empty", "Vec::new()"),
];

pub(super) fn songs(label: &str) -> Vec<Song> {
    match label {
        "3 demo tracks" => SONGS[..3].to_vec(),
        "8 tracks" => SONGS.to_vec(),
        "1 track" => SONGS[..1].to_vec(),
        _ => Vec::new(),
    }
}

/// Seek-bar peaks for a track (400, like `useWaveformData({ samples: 400 })`):
/// a deterministic beat-and-swell shape, not a decode.
fn peaks(index: usize) -> Arc<[f32]> {
    static PEAKS: OnceLock<Vec<Arc<[f32]>>> = OnceLock::new();
    PEAKS
        .get_or_init(|| (0..SONGS.len()).map(make_peaks).collect())
        .get(index)
        .cloned()
        .unwrap_or_else(|| make_peaks(index))
}

fn make_peaks(seed: usize) -> Arc<[f32]> {
    const COUNT: usize = 400;
    let mut rng = 0x9e37_79b9_u32.wrapping_mul(seed as u32 + 1);
    let beats = 24.0 + seed as f32 * 6.0;
    (0..COUNT)
        .map(|i| {
            let x = i as f32 / COUNT as f32;
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let jitter = (rng % 1000) as f32 / 1000.0;
            let swell = 0.45 + 0.25 * (x * TAU * (1.5 + seed as f32 * 0.3)).sin();
            let beat = (1.0 - (x * beats).fract()).powi(3) * 0.35;
            let fade = (x * 12.0).min(1.0) * ((1.0 - x) * 8.0).min(1.0);
            ((swell + beat + jitter * 0.15) * fade).clamp(0.0, 1.0)
        })
        .collect()
}

fn controls() -> Vec<Control> {
    vec![
        // Playlist
        Control::bool("controlled", "Controlled", true)
            .group("Playlist")
            .no_code()
            .remount()
            .help("On: .tracks(..); off: .default_tracks(..) (remounts)."),
        Control::choice("track_set", "Tracks", 0, TRACK_SETS)
            .group("Playlist")
            .method("tracks")
            .remount(),
        // Per track
        Control::bool("with_artist", "Artist", true)
            .group("Per track (MusicTrack)")
            .no_code(),
        Control::bool("with_duration", "Duration", true)
            .group("Per track (MusicTrack)")
            .no_code()
            .help("The simulated player needs it; off shows an unknown length."),
        Control::bool("with_peaks", "Peaks", true)
            .group("Per track (MusicTrack)")
            .no_code()
            .help("Off draws an empty seek bar (no decoder here)."),
        Control::bool("with_artwork", "Artwork URL", false)
            .group("Per track (MusicTrack)")
            .no_code()
            .help("A URL that will not load: shows the placeholder fallback."),
        // Audio seam
        Control::choice(
            "player",
            "Player",
            0,
            &[("built-in (unset)", ""), ("host SimulatedPlayer", "player")],
        )
        .group("Audio seam")
        .remount()
        .help("Host: on_track_change loads the track into the page's own player."),
        Control::choice(
            "ducking",
            "Ducking source",
            0,
            &[
                ("omitted (no ducking UI)", ""),
                ("None (UI, no source)", "None"),
                ("playground meter", "Some(signal.meter.clone())"),
            ],
        )
        .group("Audio seam")
        .method("ducking_source")
        .help("Playground meter: the Source panel (pick Microphone (real) in device builds)."),
        Control::choice(
            "output",
            "Output",
            0,
            &[
                ("omitted (speakers)", ""),
                ("Some(MusicOutput)", "Some(MusicOutput)"),
                ("None (nowhere)", "None"),
            ],
        )
        .group("Audio seam"),
        // Layout
        Control::px("max_width", "Max width (px)", 448.0, 280.0, 900.0, 8.0)
            .group("Layout")
            .method("max_w"),
    ]
}

fn tracks(p: &Props) -> Vec<MusicTrack> {
    songs(p.choice("track_set"))
        .into_iter()
        .enumerate()
        .map(|(index, (id, title, artist, seconds))| {
            let mut track = MusicTrack::new(id, title, id);
            if p.bool("with_artist") {
                track = track.artist(artist);
            }
            if p.bool("with_duration") {
                track = track.duration(seconds);
            }
            if p.bool("with_peaks") {
                track = track.peaks(peaks(index));
            }
            if p.bool("with_artwork") {
                track = track.artwork(format!("https://invalid.example/{id}.jpg"));
            }
            track
        })
        .collect()
}

/// The host player's current track.
struct Host {
    src: Option<SharedString>,
    duration: f64,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let tracks = tracks(p);
    let mut player = MusicPlayer::new(ctx.id("music-player"));

    if p.choice("player") == "host SimulatedPlayer" {
        let first = tracks.first().cloned();
        let host = window.use_keyed_state(ctx.id("host"), cx, move |_, _| Host {
            src: first.as_ref().map(|t| t.src.clone()),
            duration: first.and_then(|t| t.duration).unwrap_or(0.0),
        });
        let (src, duration) = {
            let h = host.read(cx);
            (h.src.clone(), h.duration)
        };
        let engine = use_simulated_player(
            ctx.id("host-player"),
            duration,
            UseAudioPlayerOptions {
                src,
                ..Default::default()
            },
            window,
            cx,
        );
        let log = ctx.clone();
        player = player.player(engine).on_track_change(move |track, _, cx| {
            log.log(
                "on_track_change",
                format!("{} (host loads {})", track.title, track.src),
                cx,
            );
            let (src, duration) = (track.src.clone(), track.duration.unwrap_or(0.0));
            host.update(cx, |h, cx| {
                h.src = Some(src);
                h.duration = duration;
                cx.notify();
            });
        });
    } else {
        let log = ctx.clone();
        player = player.on_track_change(move |track, _, cx| {
            log.log(
                "on_track_change",
                format!("{} ({})", track.title, track.id),
                cx,
            );
        });
    }

    player = if p.bool("controlled") {
        player.tracks(tracks)
    } else {
        player.default_tracks(tracks)
    };
    player = match p.choice("ducking") {
        "None (UI, no source)" => player.ducking_source(Option::<FrameSource<MeterFrame>>::None),
        "playground meter" => player.ducking_source(ctx.meter()),
        _ => player,
    };
    player = match p.choice("output") {
        "Some(MusicOutput)" => player.output(Some(MusicOutput)),
        "None (nowhere)" => player.output(None),
        _ => player,
    };

    // The duck gain moves every frame while it ramps: log steps of 0.05 and
    // the return to unity.
    let last = window
        .use_keyed_state(ctx.id("duck-last"), cx, |_, _| Rc::new(Cell::new(1.0f32)))
        .read(cx)
        .clone();
    let log = ctx.clone();
    player
        .on_duck_gain(move |gain, _, cx| {
            let previous = last.get();
            let back_to_unity = gain >= 0.999 && previous < 0.999;
            if (gain - previous).abs() >= 0.05 || back_to_unity {
                last.set(gain);
                log.log(
                    "on_duck_gain",
                    format!("{gain:.3} ({:.1} dB)", gain_to_db(gain)),
                    cx,
                );
            }
        })
        .w_full()
        .max_w(px(p.num("max_width")))
        .into_any_element()
}
