//! Playlist player with a waveform seek bar, shuffle, repeat and ducking
//! under the microphone. Port of `components/blocks/music-player/music-player.tsx`.
//!
//! # Bring your own audio
//!
//! The block never opens a device. Playback is an [`AudioPlayerController`]
//! from `components/ui/audio-player`:
//!
//! - Pass one with [`MusicPlayer::player`]. On a track change the block calls
//!   [`MusicPlayer::on_track_change`] first, then `play` when that change
//!   should start on its own (next, previous, a different row, or the track
//!   ending). Load `track.src` in the callback. Repeat-one calls `set_loop`.
//! - With no player, the block keeps a [`SimulatedPlayer`] — the same
//!   clock-driven stand-in as `useAudioPlayer` when nothing can decode — and
//!   loads the current track's `src` and `duration` itself. This is the
//!   gallery default. There is no sample decoder here, so set
//!   [`MusicTrack::duration`] (and [`MusicTrack::peaks`] for the seek bar).
//!
//! Ducking is a downstream gain, separate from the volume slider: React puts
//! a `GainNode` between the media element and `output`. This crate has no
//! audio graph. While [`MusicPlayer::ducking_source`] peaks at or above
//! −35 dB and the switch is on, the gain aims at `db_to_gain(amount)` with a
//! 20 ms attack, holds 200 ms, then releases to unity with a 150 ms time
//! constant (`setTargetAtTime`). [`MusicPlayer::on_duck_gain`] reports that
//! linear gain (1 = unity) for the host to apply. The simulated player has
//! no output, so the gallery only shows the switch and the amount slider.
//!
//! [`MusicPlayer::output`] is React's `output` prop. `Some` means the host
//! routes playback (a mixer bus, or nowhere when the inner value is `None`).
//! The block does not connect it. Passing `output`, or a ducking source, is
//! what the React block calls "routed".
//!
//! ```ignore
//! MusicPlayer::new("playlist")
//!     .tracks(tracks)
//!     .player(player)
//!     .ducking_source(mic_meter)
//!     .on_track_change(|track, _, _| load(&track.src))
//!     .on_duck_gain(|gain, _, _| apply_bus_gain(gain))
//! ```

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ElementId, Entity, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::audio::decibels::db_to_gain;
use crate::audio::frame_source::FrameSource;
use crate::audio::time::{FormatTimeOptions, format_time};
use crate::audio::types::MeterFrame;
use crate::components::{
    AudioPlayer, AudioPlayerArtwork, AudioPlayerController, AudioPlayerControls,
    AudioPlayerDescription, AudioPlayerKeyTarget, AudioPlayerNext, AudioPlayerPlay,
    AudioPlayerPlayState, AudioPlayerPrevious, AudioPlayerStatus, AudioPlayerTime,
    AudioPlayerTimeType, AudioPlayerTitle, AudioPlayerVolume, ParameterSlider,
    ParameterSliderControl, ParameterSliderHeader, ParameterSliderLabel, ParameterSliderValue,
    ShortcutTarget, TrackList, TrackListItem, TrackListItemContent, TrackListItemDescription,
    TrackListItemDuration, TrackListItemIndex, TrackListItemTitle, TrackListVariant,
    UseAudioPlayerOptions, VolumeControlMute, VolumeControlSlider, Waveform, WaveformCanvas,
    WaveformCursor, WaveformHover, use_simulated_player,
};
use crate::motion::Instant;
use crate::theme::ActiveTheme;
use crate::ui::icons::{Icon, IconName};
use crate::ui::styles::{radius_lg, radius_xl};
use crate::ui::{
    Card, CardContent, Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant,
    EmptyTitle, Label, Switch, SwitchSize, Toggle, ToggleSize,
};

/// One entry in a [`MusicPlayer`] playlist (`MusicTrack`).
///
/// ```ignore
/// MusicTrack::new("night-drive", "Night Drive", "night-drive.mp3").duration(19.2)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct MusicTrack {
    pub id: SharedString,
    pub title: SharedString,
    pub artist: Option<SharedString>,
    /// What the player loads. The built-in [`SimulatedPlayer`] treats this as
    /// an opaque source id; it does not read bytes.
    pub src: SharedString,
    /// Cover image URL. Absent shows the music-notes placeholder.
    pub artwork: Option<SharedString>,
    /// Seconds, shown before a file's own duration is known. Required for the
    /// built-in [`SimulatedPlayer`], which cannot decode `src`.
    pub duration: Option<f64>,
    /// Seek-bar peaks, 0..1, from `useWaveformData` (`samples: 400` in the
    /// React block). Absent draws an empty seek bar: this crate does not decode.
    pub peaks: Option<Arc<[f32]>>,
}

impl MusicTrack {
    /// A track with an id, title and source. Artist, art, duration and peaks
    /// are unset.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        src: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artist: None,
            src: src.into(),
            artwork: None,
            duration: None,
            peaks: None,
        }
    }

    pub fn artist(mut self, artist: impl Into<SharedString>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    pub fn artwork(mut self, artwork: impl Into<SharedString>) -> Self {
        self.artwork = Some(artwork.into());
        self
    }

    /// Seconds.
    pub fn duration(mut self, duration: f64) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Waveform peaks, 0..1.
    pub fn peaks(mut self, peaks: impl Into<Arc<[f32]>>) -> Self {
        self.peaks = Some(peaks.into());
        self
    }
}

/// Host-owned output bus (`output`'s `AudioNode`). The block does not connect
/// it; see the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MusicOutput;

/// `Repeat` of the music player: off, the whole playlist, or the current track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Repeat {
    Off,
    #[default]
    All,
    One,
}

/// `nextRepeat`.
fn next_repeat(repeat: Repeat) -> Repeat {
    match repeat {
        Repeat::All => Repeat::One,
        Repeat::Off => Repeat::All,
        Repeat::One => Repeat::Off,
    }
}

const DUCK_THRESHOLD_DB: f32 = -35.0;
const DUCK_ATTACK: f64 = 0.02;
const DUCK_RELEASE: f64 = 0.15;
/// How long a duck holds after the last loud frame before it lets go.
const DUCK_HOLD: f64 = 0.2;

type TrackHandler = Rc<dyn Fn(&MusicTrack, &mut Window, &mut App)>;
type GainHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type PlayerHandle = Rc<dyn AudioPlayerController>;

/// `duckingSource`: omitted, explicit `null`, or a meter.
enum DuckingProp {
    Omitted,
    Null,
    Source(FrameSource<MeterFrame>),
}

/// One scheduled `setTargetAtTime`.
#[derive(Clone, Copy)]
struct DuckEvent {
    start: f64,
    target: f64,
    tau: f64,
}

/// Exponential approach used by Web Audio `setTargetAtTime`. `tau <= 0` holds.
fn approach(from: f64, target: f64, tau: f64, elapsed: f64) -> f64 {
    if elapsed <= 0.0 || tau <= 0.0 {
        return from;
    }
    target + (from - target) * (-elapsed / tau).exp()
}

fn curve_sample(anchor_t: f64, anchor_v: f64, events: &[DuckEvent], t: f64) -> f64 {
    let mut from_t = anchor_t;
    let mut from_v = anchor_v;
    let mut target = anchor_v;
    let mut tau = 0.0;
    for event in events {
        if event.start > t {
            break;
        }
        from_v = approach(from_v, target, tau, event.start - from_t);
        from_t = event.start;
        target = event.target;
        tau = event.tau;
    }
    approach(from_v, target, tau, t - from_t)
}

/// `cancelScheduledValues` plus a new attack and its held release.
fn curve_trigger(
    anchor_t: f64,
    anchor_v: f64,
    events: &[DuckEvent],
    t: f64,
    duck: f64,
) -> (f64, f64, Vec<DuckEvent>) {
    let sampled = curve_sample(anchor_t, anchor_v, events, t);
    (
        t,
        sampled,
        vec![
            DuckEvent {
                start: t,
                target: duck,
                tau: DUCK_ATTACK,
            },
            DuckEvent {
                start: t + DUCK_HOLD,
                target: 1.0,
                tau: DUCK_RELEASE,
            },
        ],
    )
}

/// Downstream duck gain, in seconds since the player was created.
struct DuckEnvelope {
    origin: Instant,
    anchor_t: f64,
    anchor_v: f64,
    events: Vec<DuckEvent>,
}

impl DuckEnvelope {
    fn new(now: Instant) -> Self {
        Self {
            origin: now,
            anchor_t: 0.0,
            anchor_v: 1.0,
            events: Vec::new(),
        }
    }

    fn seconds(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.origin).as_secs_f64()
    }

    fn sample(&self, t: f64) -> f64 {
        curve_sample(self.anchor_t, self.anchor_v, &self.events, t)
    }

    fn trigger(&mut self, t: f64, duck: f64) {
        let (anchor_t, anchor_v, events) =
            curve_trigger(self.anchor_t, self.anchor_v, &self.events, t, duck);
        self.anchor_t = anchor_t;
        self.anchor_v = anchor_v;
        self.events = events;
    }
}

fn loudest_peak_db(frame: &MeterFrame) -> f32 {
    frame
        .channels
        .iter()
        .fold(f32::NEG_INFINITY, |loudest, level| {
            loudest.max(level.peak_db)
        })
}

/// Next playlist index. Shuffle ignores `direction` and jumps to any other
/// row, matching `go` in the React block. `random` is `Math.random()`, in
/// `[0, 1)`.
fn playlist_step(index: usize, len: usize, direction: i32, shuffle: bool, random: f64) -> usize {
    if len == 0 {
        return 0;
    }
    if shuffle && len > 1 {
        let offset = (random.clamp(0.0, 0.999_999_999) * (len - 1) as f64).floor() as usize;
        return (index + 1 + offset) % len;
    }
    (index as i32 + direction).rem_euclid(len as i32) as usize
}

fn next_unit(rng: &mut u32) -> f64 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    f64::from(*rng) / f64::from(u32::MAX)
}

struct MusicState {
    index: usize,
    shuffle: bool,
    repeat: Repeat,
    ducking: bool,
    auto_advance: bool,
    duck_amount_db: f32,
    duck: DuckEnvelope,
    last_gain: f32,
    rng: u32,
    /// External player already advanced for this `Ended` status.
    saw_ended: bool,
    applied_loop: Option<bool>,
}

impl MusicState {
    fn new() -> Self {
        Self {
            index: 0,
            shuffle: false,
            repeat: Repeat::All,
            ducking: true,
            auto_advance: false,
            duck_amount_db: -12.0,
            duck: DuckEnvelope::new(Instant::now()),
            last_gain: 1.0,
            rng: 1,
            saw_ended: false,
            applied_loop: None,
        }
    }
}

fn nest(id: &ElementId, name: &str) -> ElementId {
    ElementId::NamedChild(Arc::new(id.clone()), name.into())
}

fn go(
    state: &Entity<MusicState>,
    tracks: &[MusicTrack],
    direction: i32,
    on_track_change: &Option<TrackHandler>,
    external: Option<&PlayerHandle>,
    window: &mut Window,
    cx: &mut App,
) {
    if tracks.is_empty() {
        return;
    }
    let next = state.update(cx, |s, cx| {
        let unit = next_unit(&mut s.rng);
        let next = playlist_step(s.index, tracks.len(), direction, s.shuffle, unit);
        s.index = next;
        s.auto_advance = true;
        cx.notify();
        next
    });
    if let Some(track) = tracks.get(next)
        && let Some(callback) = on_track_change
    {
        callback(track, window, cx);
    }
    if let Some(player) = external {
        player.play(window, cx);
    }
}

fn apply_style(mut el: impl Styled + IntoElement, style: &StyleRefinement) -> AnyElement {
    el.style().refine(style);
    el.into_any_element()
}

/// A playlist player with a waveform seek bar, shuffle, repeat, volume, a
/// track list and optional ducking under a microphone meter.
///
/// ```ignore
/// MusicPlayer::new("music").tracks(tracks).max_w(px(448.0))
/// ```
#[derive(IntoElement)]
pub struct MusicPlayer {
    id: ElementId,
    tracks: Option<Vec<MusicTrack>>,
    default_tracks: Vec<MusicTrack>,
    player: Option<PlayerHandle>,
    on_track_change: Option<TrackHandler>,
    ducking: DuckingProp,
    output: Option<Option<MusicOutput>>,
    on_duck_gain: Option<GainHandler>,
    style: StyleRefinement,
}

impl MusicPlayer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tracks: None,
            default_tracks: Vec::new(),
            player: None,
            on_track_change: None,
            ducking: DuckingProp::Omitted,
            output: None,
            on_duck_gain: None,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled playlist (`tracks`). Replaces [`MusicPlayer::default_tracks`].
    pub fn tracks(mut self, tracks: Vec<MusicTrack>) -> Self {
        self.tracks = Some(tracks);
        self
    }

    /// Playlist used when [`MusicPlayer::tracks`] is unset. Default empty,
    /// which renders the empty state.
    pub fn default_tracks(mut self, tracks: Vec<MusicTrack>) -> Self {
        self.default_tracks = tracks;
        self
    }

    /// Playback engine. Unset uses a [`SimulatedPlayer`] for the current track.
    pub fn player(mut self, player: impl AudioPlayerController) -> Self {
        self.player = Some(Rc::new(player));
        self
    }

    /// [`MusicPlayer::player`] for a shared controller.
    pub fn player_rc(mut self, player: PlayerHandle) -> Self {
        self.player = Some(player);
        self
    }

    /// Fires when the current track changes, including picking the row that
    /// is already current (before play/pause toggles).
    pub fn on_track_change(
        mut self,
        f: impl Fn(&MusicTrack, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_track_change = Some(Rc::new(f));
        self
    }

    /// Meter that ducks the music, usually the microphone. `None` is React's
    /// `null` (the controls still show). Omit the call to hide them
    /// (`undefined`).
    pub fn ducking_source(mut self, source: impl Into<Option<FrameSource<MeterFrame>>>) -> Self {
        self.ducking = match source.into() {
            Some(source) => DuckingProp::Source(source),
            None => DuckingProp::Null,
        };
        self
    }

    /// React `output`. `Some(MusicOutput)` routes into the host's mixer;
    /// `None` is `output={null}` (connected nowhere). Omit the call for the
    /// speakers. See the module docs.
    pub fn output(mut self, output: Option<MusicOutput>) -> Self {
        self.output = Some(output);
        self
    }

    /// Linear duck gain, 1 at unity. Fires when it moves. The host applies it
    /// on the bus after the player's own volume.
    pub fn on_duck_gain(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_duck_gain = Some(Rc::new(f));
        self
    }
}

impl Styled for MusicPlayer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MusicPlayer {
    #[allow(clippy::too_many_lines)]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let MusicPlayer {
            id,
            tracks,
            default_tracks,
            player: external,
            on_track_change,
            ducking,
            output,
            on_duck_gain,
            style,
        } = self;

        let tracks = tracks.unwrap_or(default_tracks);
        let state = window.use_keyed_state(id.clone(), cx, |_, _| MusicState::new());
        if !tracks.is_empty() {
            let len = tracks.len();
            let overflow = state.read(cx).index >= len;
            if overflow {
                state.update(cx, |s, cx| {
                    s.index = len - 1;
                    cx.notify();
                });
            }
        }

        let (index, repeat, auto_advance, ducking_on, duck_amount) = {
            let s = state.read(cx);
            (
                s.index,
                s.repeat,
                s.auto_advance,
                s.ducking,
                s.duck_amount_db,
            )
        };
        let track = tracks.get(index);

        let show_duck = !matches!(ducking, DuckingProp::Omitted);
        // React: `output !== undefined || duckingSource !== undefined`.
        let routed = output.is_some() || show_duck;
        let ducking_source = match ducking {
            DuckingProp::Source(source) if routed => Some(source),
            _ => None,
        };

        let external_handle = external.clone();
        let player: PlayerHandle = if let Some(player) = external {
            let loop_one = repeat == Repeat::One;
            let needs_loop = state.read(cx).applied_loop != Some(loop_one);
            if needs_loop {
                state.update(cx, |s, _| s.applied_loop = Some(loop_one));
                let player = player.clone();
                window.defer(cx, move |window, cx| player.set_loop(loop_one, window, cx));
            }
            player
        } else {
            let duration = track.and_then(|item| item.duration).unwrap_or(0.0);
            let ended_state = state.clone();
            let ended_tracks = tracks.clone();
            let ended_change = on_track_change.clone();
            let options = UseAudioPlayerOptions {
                src: track.map(|item| item.src.clone()),
                auto_play: auto_advance,
                r#loop: repeat == Repeat::One,
                on_ended: Some(Rc::new(move |window, cx| {
                    let (index, repeat, len) = {
                        let s = ended_state.read(cx);
                        (s.index, s.repeat, ended_tracks.len())
                    };
                    if repeat == Repeat::All || index + 1 < len {
                        go(
                            &ended_state,
                            &ended_tracks,
                            1,
                            &ended_change,
                            None,
                            window,
                            cx,
                        );
                    }
                })),
                ..Default::default()
            };
            Rc::new(use_simulated_player(
                nest(&id, "engine"),
                duration,
                options,
                window,
                cx,
            ))
        };

        let snapshot = player.state(cx);
        if external_handle.is_some() {
            let ended = snapshot.status == AudioPlayerStatus::Ended;
            let saw = state.read(cx).saw_ended;
            if ended && !saw {
                state.update(cx, |s, _| s.saw_ended = true);
                let len = tracks.len();
                if repeat == Repeat::All || index + 1 < len {
                    let state = state.clone();
                    let tracks = tracks.clone();
                    let on_track_change = on_track_change.clone();
                    let player = player.clone();
                    window.defer(cx, move |window, cx| {
                        go(
                            &state,
                            &tracks,
                            1,
                            &on_track_change,
                            Some(&player),
                            window,
                            cx,
                        );
                    });
                }
            } else if !ended && saw {
                state.update(cx, |s, _| s.saw_ended = false);
            }
        }

        if let Some(source) = ducking_source.as_ref() {
            let loud = source
                .latest()
                .as_ref()
                .map(loudest_peak_db)
                .unwrap_or(f32::NEG_INFINITY);
            let now = Instant::now();
            let (report, gain) = state.update(cx, |s, _| {
                let t = s.duck.seconds(now);
                if s.ducking && loud >= DUCK_THRESHOLD_DB {
                    s.duck.trigger(t, f64::from(db_to_gain(s.duck_amount_db)));
                }
                let gain = s.duck.sample(t) as f32;
                let report = (gain - s.last_gain).abs() > 1.0e-3;
                if report {
                    s.last_gain = gain;
                }
                (report, gain)
            });
            if report && let Some(callback) = on_duck_gain.clone() {
                window.defer(cx, move |window, cx| callback(gain, window, cx));
            }
            window.request_animation_frame();
        }

        if tracks.is_empty() {
            return apply_style(
                Empty::new().w_full().child(
                    EmptyHeader::new()
                        .child(
                            EmptyMedia::new()
                                .variant(EmptyMediaVariant::Icon)
                                .icon(IconName::MusicNotes),
                        )
                        .child(EmptyTitle::new("No music"))
                        .child(EmptyDescription::new("Add tracks to start playing.")),
                ),
                &style,
            );
        }

        let theme_border = cx.theme().border;
        let art_bg = cx.theme().muted;
        let art_fg = cx.theme().muted_foreground;
        let art_radius = radius_lg(cx.theme());
        let panel_radius = radius_xl(cx.theme());
        let time_source = player.time(cx);
        let seek_player = player.clone();
        let duration = track
            .and_then(|item| item.duration)
            .filter(|seconds| *seconds > 0.0)
            .unwrap_or(snapshot.duration);

        let mut waveform = Waveform::new(nest(&id, "waveform"))
            .h(px(56.0))
            .current_time(snapshot.current_time)
            .duration(duration)
            .time(time_source)
            .on_seek_committed(move |value, window, cx| seek_player.seek(value, window, cx))
            .child(WaveformCanvas::new())
            .child(WaveformCursor::new())
            .child(WaveformHover::new());
        if let Some(peaks) = track.and_then(|item| item.peaks.clone()) {
            waveform = waveform.peaks(peaks);
        }

        let play_external = external_handle.clone();
        let prev_state = state.clone();
        let prev_tracks = tracks.clone();
        let prev_change = on_track_change.clone();
        let next_state = state.clone();
        let next_tracks = tracks.clone();
        let next_change = on_track_change.clone();

        let title = track.map(|item| item.title.clone());
        let artist = track.and_then(|item| item.artist.clone());
        let artwork = track.and_then(|item| item.artwork.clone());

        let mut description = AudioPlayerDescription::new();
        if let Some(artist) = artist {
            description = description.child(artist);
        }

        let column = div().flex().w_full().flex_col().gap(px(16.0)).child(
            AudioPlayer::new(nest(&id, "player"))
                .player_rc(player.clone())
                .flex_col()
                .items_stretch()
                .gap(px(12.0))
                .on_previous({
                    let play_external = play_external.clone();
                    move |window, cx| {
                        go(
                            &prev_state,
                            &prev_tracks,
                            -1,
                            &prev_change,
                            play_external.as_ref(),
                            window,
                            cx,
                        );
                    }
                })
                .on_next({
                    let play_external = play_external.clone();
                    move |window, cx| {
                        go(
                            &next_state,
                            &next_tracks,
                            1,
                            &next_change,
                            play_external.as_ref(),
                            window,
                            cx,
                        );
                    }
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(artwork_slot(artwork, art_bg, art_fg, art_radius))
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .flex_1()
                                .flex_col()
                                .child(AudioPlayerTitle::new().children(title))
                                .child(description),
                        ),
                )
                .child(AudioPlayerKeyTarget::new(ShortcutTarget::RANGE, waveform))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(AudioPlayerTime::new())
                        .child(AudioPlayerTime::new().r#type(AudioPlayerTimeType::Remaining)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.0))
                        .child(AudioPlayerKeyTarget::new(
                            ShortcutTarget::BUTTON,
                            Toggle::new(nest(&id, "shuffle"))
                                .size(ToggleSize::Sm)
                                .pressed(state.read(cx).shuffle)
                                .icon(IconName::Shuffle)
                                .on_pressed_change({
                                    let state = state.clone();
                                    move |pressed, _, cx| {
                                        state.update(cx, |s, cx| {
                                            s.shuffle = pressed;
                                            cx.notify();
                                        });
                                    }
                                }),
                        ))
                        .child(
                            AudioPlayerControls::new()
                                .child(
                                    AudioPlayerPrevious::new()
                                        .id(nest(&id, "previous"))
                                        .child(Icon::new(IconName::SkipBack)),
                                )
                                .child(AudioPlayerPlay::new().id(nest(&id, "play")).children_with(
                                    |play: AudioPlayerPlayState| {
                                        Icon::new(if play.playing {
                                            IconName::Pause
                                        } else {
                                            IconName::Play
                                        })
                                        .fill()
                                        .into_any_element()
                                    },
                                ))
                                .child(
                                    AudioPlayerNext::new()
                                        .id(nest(&id, "next"))
                                        .child(Icon::new(IconName::SkipForward)),
                                ),
                        )
                        .child(AudioPlayerKeyTarget::new(
                            ShortcutTarget::BUTTON,
                            Toggle::new(nest(&id, "repeat"))
                                .size(ToggleSize::Sm)
                                .pressed(repeat != Repeat::Off)
                                .icon(if repeat == Repeat::One {
                                    IconName::RepeatOnce
                                } else {
                                    IconName::Repeat
                                })
                                .on_pressed_change({
                                    let state = state.clone();
                                    move |_, _, cx| {
                                        state.update(cx, |s, cx| {
                                            s.repeat = next_repeat(s.repeat);
                                            cx.notify();
                                        });
                                    }
                                }),
                        )),
                )
                .child(
                    AudioPlayerVolume::new()
                        .id(nest(&id, "volume"))
                        .child(VolumeControlMute::new().child(Icon::new(if snapshot.muted {
                            IconName::SpeakerX
                        } else {
                            IconName::SpeakerHigh
                        })))
                        .child(VolumeControlSlider::new()),
                ),
        );

        let rows = track_rows(
            &id,
            &tracks,
            index,
            snapshot.playing,
            &state,
            &player,
            &on_track_change,
            external_handle.is_some(),
        );
        let column = column.child(rows).when(show_duck, |column| {
            column.child(duck_panel(
                &id,
                &state,
                ducking_on,
                duck_amount,
                theme_border,
                panel_radius,
            ))
        });

        apply_style(
            Card::new().w_full().child(CardContent::new().child(column)),
            &style,
        )
    }
}

fn artwork_slot(
    artwork: Option<SharedString>,
    bg: impl Into<gpui::Hsla>,
    fg: impl Into<gpui::Hsla>,
    radius: gpui::Pixels,
) -> AnyElement {
    if let Some(src) = artwork {
        AudioPlayerArtwork::new(src).into_any_element()
    } else {
        div()
            .flex()
            .size(px(48.0))
            .items_center()
            .justify_center()
            .rounded(radius)
            .bg(bg.into())
            .text_color(fg.into())
            .child(Icon::new(IconName::MusicNotes).size(px(20.0)))
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn track_rows(
    id: &ElementId,
    tracks: &[MusicTrack],
    index: usize,
    playing: bool,
    state: &Entity<MusicState>,
    player: &PlayerHandle,
    on_track_change: &Option<TrackHandler>,
    external: bool,
) -> AnyElement {
    let mut list = TrackList::new(nest(id, "tracks")).variant(TrackListVariant::Outline);
    for (position, item) in tracks.iter().enumerate() {
        let selected = item.clone();
        let state = state.clone();
        let player = player.clone();
        let on_track_change = on_track_change.clone();
        list = list.child(
            TrackListItem::new(nest(id, &format!("row-{}", item.id)))
                .active(position == index)
                .playing(position == index && playing)
                .on_select(move |window, cx| {
                    let current = state.read(cx).index;
                    if position == current {
                        if let Some(callback) = &on_track_change {
                            callback(&selected, window, cx);
                        }
                        player.toggle(window, cx);
                    } else {
                        state.update(cx, |s, cx| {
                            s.index = position;
                            s.auto_advance = true;
                            cx.notify();
                        });
                        if let Some(callback) = &on_track_change {
                            callback(&selected, window, cx);
                        }
                        if external {
                            player.play(window, cx);
                        }
                    }
                })
                .child(TrackListItemIndex::new().child((position + 1).to_string()))
                .child({
                    let mut content = TrackListItemContent::new()
                        .child(TrackListItemTitle::new().child(item.title.clone()));
                    if let Some(artist) = item.artist.clone() {
                        content = content.child(TrackListItemDescription::new().child(artist));
                    }
                    content
                })
                .when_some(item.duration, |row, seconds| {
                    row.child(
                        TrackListItemDuration::new()
                            .child(format_time(seconds, FormatTimeOptions::default())),
                    )
                }),
        );
    }
    list.into_any_element()
}

fn duck_panel(
    id: &ElementId,
    state: &Entity<MusicState>,
    ducking: bool,
    amount: f32,
    border: gpui::Hsla,
    radius: gpui::Pixels,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .rounded(radius)
        .border_1()
        .border_color(border)
        .p(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(Label::new("Lower music while you talk"))
                .child(
                    Switch::new(nest(id, "duck-switch"))
                        .size(SwitchSize::Sm)
                        .checked(ducking)
                        .on_checked_change({
                            let state = state.clone();
                            move |checked, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.ducking = checked;
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        .child(
            ParameterSlider::new(nest(id, "duck-amount"))
                .disabled(!ducking)
                .max(0.0)
                .min(-30.0)
                .unit("dB")
                .value(amount)
                .on_value_change({
                    let state = state.clone();
                    move |value, _, cx| {
                        state.update(cx, |s, cx| {
                            s.duck_amount_db = value;
                            cx.notify();
                        });
                    }
                })
                .child(
                    ParameterSliderHeader::new()
                        .child(ParameterSliderLabel::new("Amount"))
                        .child(ParameterSliderValue::new()),
                )
                .child(ParameterSliderControl::new()),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::ChannelLevel;

    #[test]
    fn repeat_cycles_all_one_off() {
        assert_eq!(next_repeat(Repeat::All), Repeat::One);
        assert_eq!(next_repeat(Repeat::One), Repeat::Off);
        assert_eq!(next_repeat(Repeat::Off), Repeat::All);
    }

    #[test]
    fn playlist_wraps_and_shuffle_skips_the_current_row() {
        assert_eq!(playlist_step(0, 3, 1, false, 0.0), 1);
        assert_eq!(playlist_step(0, 3, -1, false, 0.0), 2);
        assert_eq!(playlist_step(2, 3, 1, false, 0.0), 0);
        assert_eq!(playlist_step(0, 0, 1, false, 0.0), 0);
        // random 0 → offset 0 → the next row; random near 1 → the other row.
        assert_eq!(playlist_step(0, 3, -1, true, 0.0), 1);
        assert_eq!(playlist_step(0, 3, 1, true, 0.99), 2);
        assert_eq!(playlist_step(1, 3, 1, true, 0.0), 2);
        assert_ne!(playlist_step(1, 3, 1, true, 0.0), 1);
    }

    #[test]
    fn duck_attacks_then_releases_to_unity() {
        let duck = 0.25;
        let (anchor_t, anchor_v, events) = curve_trigger(0.0, 1.0, &[], 0.0, duck);
        assert!((curve_sample(anchor_t, anchor_v, &events, 0.0) - 1.0).abs() < 1e-9);
        let at_attack = curve_sample(anchor_t, anchor_v, &events, 0.1);
        let expect = duck + (1.0 - duck) * (-0.1 / DUCK_ATTACK).exp();
        assert!((at_attack - expect).abs() < 1e-6, "{at_attack} vs {expect}");
        assert!(curve_sample(anchor_t, anchor_v, &events, 1.0) > 0.99);
    }

    #[test]
    fn duck_envelope_holds_the_release_from_the_last_loud_frame() {
        let mut envelope = DuckEnvelope::new(Instant::now());
        envelope.trigger(0.0, 0.25);
        envelope.trigger(0.05, 0.25);
        let held = envelope.sample(0.2);
        assert!(held < 0.5, "{held}");
        assert!(envelope.sample(1.2) > 0.99);
    }

    #[test]
    fn loudest_channel_wins() {
        let frame = MeterFrame::stereo(ChannelLevel::new(-40.0), ChannelLevel::new(-12.0));
        assert_eq!(loudest_peak_db(&frame), -12.0);
        assert!(loudest_peak_db(&MeterFrame::default()).is_infinite());
    }
}
