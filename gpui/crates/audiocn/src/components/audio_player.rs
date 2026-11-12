//! Port of `components/ui/audio-player.tsx` and the player state machine of
//! `hooks/use-audio-player.ts`.
//!
//! # The player seam
//!
//! In React, `<AudioPlayer>` is composable UI over the value returned by
//! `useAudioPlayer` (an `HTMLAudioElement` controller). Here that value is the
//! [`AudioPlayerController`] trait: a snapshot of the playback state
//! ([`AudioPlayerState`]), a smooth playhead ([`FrameSource<f64>`]) and the
//! actions (`play`, `pause`, `seek`, ...). Every part reads the controller of
//! the surrounding [`AudioPlayer`] and never touches audio itself.
//!
//! - [`SimulatedPlayer`] is a pure-Rust, clock-driven fake of the hook: it
//!   "plays" a track of a given duration with the same status transitions,
//!   events and quirks as the media element, without audio output. Use it
//!   through [`use_simulated_player`] (the `useAudioPlayer` of this crate when
//!   there is no audio device), in tests, and in the gallery.
//! - A real implementation (native playback behind the `device` feature)
//!   implements [`AudioPlayerController`] for its own entity and is passed
//!   with [`AudioPlayer::player`]. It can also become the default player of
//!   `<AudioPlayer src=..>` (no `.player`) with [`set_audio_player_backend`].
//!   Without a backend, an `AudioPlayer` that only gets a `src` reports an
//!   error status, like a media element that cannot decode its source.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context, ElementId, Entity, FocusHandle,
    FontWeight, Global, ImageSource, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseMoveEvent, MouseUpEvent, ObjectFit, ParentElement, Pixels, Refineable, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, StyledImage, Task, Window,
    canvas, div, img, prelude::FluentBuilder, px, relative,
};

use crate::audio::frame_source::{FrameEmitter, FrameSource};
use crate::audio::time::{FormatTimeOptions, format_time};
use crate::audio::types::Orientation;
use crate::components::volume_control::{VolumeControl, VolumeCurve};
use crate::config::AudioSize;
use crate::motion::{Duration, Instant};
use crate::theme::ActiveTheme;
use crate::ui::styles::{
    TextSize, provide, radius_lg, ring_overlay, ring_shadow, shadow_sm, use_context,
};

/// `DEFAULT_RATES`: the speeds [`AudioPlayerRate`] cycles through.
pub const DEFAULT_RATES: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
/// Seconds per arrow key (`SEEK_STEP`).
pub const SEEK_STEP: f64 = 5.0;
/// Seconds per Shift+arrow (`SEEK_LARGE_STEP`).
pub const SEEK_LARGE_STEP: f64 = 15.0;
/// Volume change per Up/Down key (`VOLUME_STEP`).
pub const VOLUME_STEP: f32 = 0.05;

/// How often the simulated element reports `timeupdate` (browsers: ~4 Hz).
const TIME_UPDATE_INTERVAL: Duration = Duration::from_millis(250);
/// The simulated element's clock tick (emits the smooth playhead).
const TICK_INTERVAL: Duration = Duration::from_millis(16);

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type ErrorHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;
type TimeHandler = Rc<dyn Fn(f64, &mut Window, &mut App) + 'static>;

/// JS `clamp` from `lib/audio/decibels.ts`: `Math.min(Math.max(v, min), max)`
/// (never panics when `min > max`).
fn clamp_js(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

// ---------------------------------------------------------------------------
// Player state + controller (the `useAudioPlayer` return value)
// ---------------------------------------------------------------------------

/// `AudioPlayerStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioPlayerStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Ended,
    Error,
}

/// A snapshot of a player: the state fields of `AudioPlayerController`.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioPlayerState {
    pub status: AudioPlayerStatus,
    /// `status == Playing`.
    pub playing: bool,
    /// Updates about four times a second. Use [`AudioPlayerController::time`]
    /// for a smooth playhead.
    pub current_time: f64,
    pub duration: f64,
    /// End of the buffered range, in seconds.
    pub buffered: f64,
    /// 0..1.
    pub volume: f32,
    pub muted: bool,
    pub playback_rate: f32,
    pub r#loop: bool,
    /// `MediaError` as a message.
    pub error: Option<SharedString>,
}

impl Default for AudioPlayerState {
    fn default() -> Self {
        Self {
            status: AudioPlayerStatus::Idle,
            playing: false,
            current_time: 0.0,
            duration: 0.0,
            buffered: 0.0,
            volume: 1.0,
            muted: false,
            playback_rate: 1.0,
            r#loop: false,
            error: None,
        }
    }
}

/// The player an [`AudioPlayer`] drives: `AudioPlayerController` of
/// `use-audio-player.ts`. Implement it to put any playback engine behind the
/// player UI (see the module docs).
///
/// ```ignore
/// let player = use_simulated_player("episode", 220.0, UseAudioPlayerOptions::src("episode.mp3"), window, cx);
/// AudioPlayer::new("player").player(player).child(AudioPlayerPlay::new()).child(AudioPlayerSeek::new())
/// ```
pub trait AudioPlayerController: 'static {
    /// The current state. Reading it during render subscribes the view to changes
    /// when the controller is an entity.
    fn state(&self, cx: &App) -> AudioPlayerState;
    /// The current time on every animation frame while playing.
    fn time(&self, cx: &App) -> FrameSource<f64>;
    fn play(&self, window: &mut Window, cx: &mut App);
    fn pause(&self, window: &mut Window, cx: &mut App);
    /// Plays when paused, pauses when playing.
    fn toggle(&self, window: &mut Window, cx: &mut App) {
        if self.state(cx).playing {
            self.pause(window, cx);
        } else {
            self.play(window, cx);
        }
    }
    /// Seconds, clamped to the duration.
    fn seek(&self, seconds: f64, window: &mut Window, cx: &mut App);
    /// 0..1.
    fn set_volume(&self, volume: f32, window: &mut Window, cx: &mut App);
    fn set_muted(&self, muted: bool, window: &mut Window, cx: &mut App);
    fn set_playback_rate(&self, rate: f32, window: &mut Window, cx: &mut App);
    fn set_loop(&self, r#loop: bool, window: &mut Window, cx: &mut App);
}

/// `preload` of [`UseAudioPlayerOptions`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioPreload {
    None,
    #[default]
    Metadata,
    Auto,
}

/// `crossOrigin` of [`UseAudioPlayerOptions`]. Needed to analyse remote audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrossOrigin {
    Anonymous,
    UseCredentials,
}

/// `UseAudioPlayerOptions`: what a player loads and how it starts.
#[derive(Clone)]
pub struct UseAudioPlayerOptions {
    pub src: Option<SharedString>,
    pub auto_play: bool,
    pub r#loop: bool,
    /// 0..1. Default 1.
    pub volume: f32,
    pub muted: bool,
    /// Default 1.
    pub playback_rate: f32,
    /// Default `Metadata`.
    pub preload: AudioPreload,
    pub cross_origin: Option<CrossOrigin>,
    pub on_play: Option<Handler>,
    pub on_pause: Option<Handler>,
    pub on_ended: Option<Handler>,
    pub on_error: Option<ErrorHandler>,
}

impl Default for UseAudioPlayerOptions {
    fn default() -> Self {
        Self {
            src: None,
            auto_play: false,
            r#loop: false,
            volume: 1.0,
            muted: false,
            playback_rate: 1.0,
            preload: AudioPreload::Metadata,
            cross_origin: None,
            on_play: None,
            on_pause: None,
            on_ended: None,
            on_error: None,
        }
    }
}

impl UseAudioPlayerOptions {
    /// Options with only a source.
    pub fn src(src: impl Into<SharedString>) -> Self {
        Self {
            src: Some(src.into()),
            ..Default::default()
        }
    }
}

impl std::fmt::Debug for UseAudioPlayerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UseAudioPlayerOptions")
            .field("src", &self.src)
            .field("auto_play", &self.auto_play)
            .field("loop", &self.r#loop)
            .field("volume", &self.volume)
            .field("muted", &self.muted)
            .field("playback_rate", &self.playback_rate)
            .field("preload", &self.preload)
            .field("cross_origin", &self.cross_origin)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SimulatedPlayer
// ---------------------------------------------------------------------------

/// A media element event the simulated player fires (mapped to the
/// `on_play` / `on_pause` / `on_ended` / `on_error` callbacks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerEvent {
    Play,
    Pause,
    Ended,
    Error,
}

#[derive(Clone, Default)]
struct PlayerCallbacks {
    on_play: Option<Handler>,
    on_pause: Option<Handler>,
    on_ended: Option<Handler>,
    on_error: Option<ErrorHandler>,
}

/// Prop values last written to the element (the hook's `appliedRef`), so
/// re-applying options doesn't undo `set_volume` and friends.
#[derive(Clone, Copy, Debug, Default)]
struct Applied {
    volume: Option<f32>,
    muted: Option<bool>,
    playback_rate: Option<f32>,
}

/// A clock-driven fake of `useAudioPlayer`: plays a track of a known duration
/// without audio output, with the media element's status transitions
/// (`idle → loading → ready → playing ⇄ paused → ended`), `timeupdate` at
/// ~4 Hz, a smooth [`FrameSource`] playhead, looping, rate and volume.
///
/// The state methods take `now` so the machine is pure and testable; the
/// [`AudioPlayerController`] impl on `Entity<SimulatedPlayer>` drives it with
/// the wall clock and a ticker task while playing.
///
/// ```ignore
/// let player = use_simulated_player("night-drive", 19.2, UseAudioPlayerOptions::src("night-drive"), window, cx);
/// AudioPlayer::new("player").player(player.clone())
/// ```
pub struct SimulatedPlayer {
    src: Option<SharedString>,
    /// `status` before the `src ? status : "idle"` gate.
    status: AudioPlayerStatus,
    /// `element.paused`.
    paused: bool,
    /// Playhead at `anchor` (or now, when paused).
    position: f64,
    anchor: Option<Instant>,
    last_time_update: Option<Instant>,
    current_time: f64,
    duration: f64,
    buffered: f64,
    volume: f32,
    muted: bool,
    playback_rate: f32,
    loop_prop: bool,
    loop_override: Option<bool>,
    applied: Applied,
    error: Option<SharedString>,
    time: FrameEmitter<f64>,
    progress: FrameEmitter<f32>,
    callbacks: PlayerCallbacks,
    ticking: bool,
    ticker: Option<Task<()>>,
    window: Option<AnyWindowHandle>,
}

impl SimulatedPlayer {
    /// A player with the initial `volume` / `muted` / `playback_rate` of
    /// `options` and nothing loaded. Load with [`SimulatedPlayer::configure`].
    pub fn new(options: &UseAudioPlayerOptions) -> Self {
        Self {
            src: None,
            status: AudioPlayerStatus::Idle,
            paused: true,
            position: 0.0,
            anchor: None,
            last_time_update: None,
            current_time: 0.0,
            duration: 0.0,
            buffered: 0.0,
            volume: options.volume.clamp(0.0, 1.0),
            muted: options.muted,
            playback_rate: options.playback_rate,
            loop_prop: options.r#loop,
            loop_override: None,
            applied: Applied::default(),
            error: None,
            time: FrameEmitter::new(),
            progress: FrameEmitter::new(),
            callbacks: PlayerCallbacks::default(),
            ticking: false,
            ticker: None,
            window: None,
        }
    }

    /// Applies options like the hook's effects: a new `src` loads (and
    /// auto-plays), `volume` / `muted` / `playback_rate` are written only when
    /// they changed since the last call, `loop` is the default of
    /// [`set_loop`](AudioPlayerController::set_loop). `duration` is the length
    /// of `src`; `None` means it cannot be decoded (status `Error`).
    pub fn configure(
        &mut self,
        options: &UseAudioPlayerOptions,
        duration: Option<f64>,
        now: Instant,
    ) -> Vec<PlayerEvent> {
        self.callbacks = PlayerCallbacks {
            on_play: options.on_play.clone(),
            on_pause: options.on_pause.clone(),
            on_ended: options.on_ended.clone(),
            on_error: options.on_error.clone(),
        };
        if self.applied.volume != Some(options.volume) {
            self.applied.volume = Some(options.volume);
            self.volume = options.volume.clamp(0.0, 1.0);
        }
        if self.applied.muted != Some(options.muted) {
            self.applied.muted = Some(options.muted);
            self.muted = options.muted;
        }
        if self.applied.playback_rate != Some(options.playback_rate) {
            self.applied.playback_rate = Some(options.playback_rate);
            self.set_playback_rate_at(options.playback_rate, now);
        }
        self.loop_prop = options.r#loop;

        if options.src == self.src {
            return Vec::new();
        }
        self.src = options.src.clone();
        // `load()`: playback aborts (no pause event), then `emptied`.
        self.paused = true;
        self.anchor = None;
        self.position = 0.0;
        self.current_time = 0.0;
        self.duration = 0.0;
        self.buffered = 0.0;
        self.last_time_update = None;
        self.time.emit(0.0);
        self.progress.emit(0.0);
        if self.src.is_none() {
            return Vec::new();
        }
        // loadstart
        self.status = AudioPlayerStatus::Loading;
        self.error = None;
        let Some(duration) = duration else {
            self.status = AudioPlayerStatus::Error;
            self.error = Some("MEDIA_ERR_SRC_NOT_SUPPORTED: no audio backend".into());
            return vec![PlayerEvent::Error];
        };
        // loadedmetadata, progress (a simulated source is fully buffered), canplay.
        self.duration = duration.max(0.0);
        self.buffered = self.duration;
        self.status = AudioPlayerStatus::Ready;
        if options.auto_play {
            return self.play_at(now);
        }
        Vec::new()
    }

    fn looping(&self) -> bool {
        self.loop_override.unwrap_or(self.loop_prop)
    }

    /// The playhead at `now`, clamped to the duration.
    pub fn position(&self, now: Instant) -> f64 {
        let raw = self.raw_position(now);
        if self.duration > 0.0 {
            raw.min(self.duration)
        } else {
            raw
        }
    }

    fn raw_position(&self, now: Instant) -> f64 {
        match self.anchor {
            Some(anchor) => {
                let elapsed = now.saturating_duration_since(anchor).as_secs_f64();
                self.position + elapsed * f64::from(self.playback_rate)
            }
            None => self.position,
        }
    }

    fn emit_time(&self, time: f64) {
        self.time.emit(time);
        let progress = if self.duration > 0.0 {
            (time / self.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        self.progress.emit(progress);
    }

    /// `play()`: from the start once ended. Nothing without a source.
    pub fn play_at(&mut self, now: Instant) -> Vec<PlayerEvent> {
        if self.src.is_none() || self.status == AudioPlayerStatus::Error || !self.paused {
            return Vec::new();
        }
        if self.status == AudioPlayerStatus::Ended
            || (self.duration > 0.0 && self.position >= self.duration)
        {
            self.position = 0.0;
        }
        self.paused = false;
        self.anchor = Some(now);
        self.status = AudioPlayerStatus::Playing;
        self.current_time = self.position;
        self.last_time_update = Some(now);
        self.emit_time(self.position);
        vec![PlayerEvent::Play]
    }

    /// `pause()`. An ended track stays `Ended`.
    pub fn pause_at(&mut self, now: Instant) -> Vec<PlayerEvent> {
        if self.paused {
            return Vec::new();
        }
        self.position = self.position(now);
        self.anchor = None;
        self.paused = true;
        self.current_time = self.position;
        self.emit_time(self.position);
        if self.status != AudioPlayerStatus::Ended {
            self.status = AudioPlayerStatus::Paused;
        }
        vec![PlayerEvent::Pause]
    }

    /// `seek(seconds)`: clamped to `0..duration` (or unclamped above while
    /// the duration is unknown).
    pub fn seek_at(&mut self, seconds: f64, now: Instant) {
        let upper = if self.duration > 0.0 {
            self.duration
        } else {
            seconds
        };
        let target = clamp_js(seconds, 0.0, upper).max(0.0);
        self.position = target;
        if !self.paused {
            self.anchor = Some(now);
        }
        self.current_time = target;
        self.emit_time(target);
    }

    fn set_playback_rate_at(&mut self, rate: f32, now: Instant) {
        if !self.paused {
            self.position = self.position(now);
            self.anchor = Some(now);
        }
        self.playback_rate = rate;
    }

    /// Advances the clock: emits the playhead, reports `timeupdate` every
    /// 250 ms, loops or ends at the duration. Returns the events and whether
    /// the reported state changed.
    pub fn tick(&mut self, now: Instant) -> (Vec<PlayerEvent>, bool) {
        if self.paused {
            return (Vec::new(), false);
        }
        let raw = self.raw_position(now);
        if self.duration > 0.0 && raw >= self.duration {
            if self.looping() {
                self.position = raw % self.duration;
                self.anchor = Some(now);
                self.current_time = self.position;
                self.last_time_update = Some(now);
                self.emit_time(self.position);
                return (Vec::new(), true);
            }
            // Ended playback: `paused` becomes true, then `pause`, then `ended`.
            self.position = self.duration;
            self.anchor = None;
            self.paused = true;
            self.current_time = self.duration;
            self.emit_time(self.duration);
            self.status = AudioPlayerStatus::Ended;
            return (vec![PlayerEvent::Pause, PlayerEvent::Ended], true);
        }
        self.emit_time(raw);
        let due = self
            .last_time_update
            .is_none_or(|last| now.saturating_duration_since(last) >= TIME_UPDATE_INTERVAL);
        if due {
            self.current_time = raw;
            self.last_time_update = Some(now);
        }
        (Vec::new(), due)
    }

    /// Simulates a media error (decode or network failure).
    pub fn fail(&mut self, message: impl Into<SharedString>, now: Instant) -> Vec<PlayerEvent> {
        self.position = self.position(now);
        self.anchor = None;
        self.paused = true;
        self.status = AudioPlayerStatus::Error;
        self.error = Some(message.into());
        vec![PlayerEvent::Error]
    }

    /// The state snapshot (`src ? state : idle`, like the hook).
    pub fn state(&self) -> AudioPlayerState {
        let status = if self.src.is_some() {
            self.status
        } else {
            AudioPlayerStatus::Idle
        };
        AudioPlayerState {
            status,
            playing: status == AudioPlayerStatus::Playing,
            current_time: if self.src.is_some() {
                self.current_time
            } else {
                0.0
            },
            duration: self.duration,
            buffered: self.buffered,
            volume: self.volume,
            muted: self.muted,
            playback_rate: self.playback_rate,
            r#loop: self.looping(),
            error: self.error.clone(),
        }
    }

    /// The loaded source.
    pub fn src(&self) -> Option<&SharedString> {
        self.src.as_ref()
    }

    /// The smooth playhead, in seconds.
    pub fn time_source(&self) -> FrameSource<f64> {
        self.time.source()
    }

    /// Playback progress, 0..1, on every tick while playing (`useSound().progress`).
    pub fn progress_source(&self) -> FrameSource<f32> {
        self.progress.source()
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.paused || self.ticking {
            return;
        }
        self.ticking = true;
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK_INTERVAL).await;
                let Some(entity) = this.upgrade() else {
                    return;
                };
                let Ok((events, running, window)) = this.update(cx, |player, cx| {
                    let (events, changed) = player.tick(Instant::now());
                    if changed {
                        cx.notify();
                    }
                    if player.paused {
                        player.ticking = false;
                    }
                    (events, !player.paused, player.window)
                }) else {
                    return;
                };
                if !events.is_empty()
                    && let Some(window) = window
                {
                    cx.update_window(window, |_, window, cx| {
                        dispatch_events(&entity, &events, window, cx);
                    })
                    .ok();
                }
                if !running {
                    return;
                }
            }
        }));
    }
}

fn dispatch_events(
    player: &Entity<SimulatedPlayer>,
    events: &[PlayerEvent],
    window: &mut Window,
    cx: &mut App,
) {
    if events.is_empty() {
        return;
    }
    let (callbacks, error) = {
        let p = player.read(cx);
        (p.callbacks.clone(), p.error.clone())
    };
    for event in events {
        match event {
            PlayerEvent::Play => {
                if let Some(f) = &callbacks.on_play {
                    f(window, cx);
                }
            }
            PlayerEvent::Pause => {
                if let Some(f) = &callbacks.on_pause {
                    f(window, cx);
                }
            }
            PlayerEvent::Ended => {
                if let Some(f) = &callbacks.on_ended {
                    f(window, cx);
                }
            }
            PlayerEvent::Error => {
                if let Some(f) = &callbacks.on_error {
                    f(error.clone(), window, cx);
                }
            }
        }
    }
}

fn simulated_action(
    player: &Entity<SimulatedPlayer>,
    window: &mut Window,
    cx: &mut App,
    action: impl FnOnce(&mut SimulatedPlayer, Instant) -> Vec<PlayerEvent>,
) {
    let handle = window.window_handle();
    let events = player.update(cx, |p, cx| {
        p.window = Some(handle);
        let events = action(p, Instant::now());
        p.ensure_ticker(cx);
        cx.notify();
        events
    });
    dispatch_events(player, &events, window, cx);
}

impl AudioPlayerController for Entity<SimulatedPlayer> {
    fn state(&self, cx: &App) -> AudioPlayerState {
        self.read(cx).state()
    }

    fn time(&self, cx: &App) -> FrameSource<f64> {
        let player = self.read(cx);
        if !player.paused {
            // Sample the clock exactly at read time for a frame-accurate playhead.
            player.time.emit(player.position(Instant::now()));
        }
        player.time.source()
    }

    fn play(&self, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, now| p.play_at(now));
    }

    fn pause(&self, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, now| p.pause_at(now));
    }

    fn seek(&self, seconds: f64, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, now| {
            p.seek_at(seconds, now);
            Vec::new()
        });
    }

    fn set_volume(&self, volume: f32, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, _| {
            p.volume = volume.clamp(0.0, 1.0);
            Vec::new()
        });
    }

    fn set_muted(&self, muted: bool, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, _| {
            p.muted = muted;
            Vec::new()
        });
    }

    fn set_playback_rate(&self, rate: f32, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, now| {
            p.set_playback_rate_at(rate, now);
            Vec::new()
        });
    }

    fn set_loop(&self, r#loop: bool, window: &mut Window, cx: &mut App) {
        simulated_action(self, window, cx, |p, _| {
            p.loop_override = Some(r#loop);
            Vec::new()
        });
    }
}

/// `useAudioPlayer` with a [`SimulatedPlayer`]: keeps one in keyed element
/// state, applies `options` every render (like the hook's effects) and
/// returns the entity, which implements [`AudioPlayerController`].
/// `duration` is the length of `options.src` in seconds.
///
/// ```ignore
/// let player = use_simulated_player("low-tide", 24.0, UseAudioPlayerOptions::src("low-tide"), window, cx);
/// ```
pub fn use_simulated_player(
    id: impl Into<ElementId>,
    duration: f64,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SimulatedPlayer> {
    configure_simulated(id.into(), Some(duration), options, window, cx)
}

fn configure_simulated(
    id: ElementId,
    duration: Option<f64>,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SimulatedPlayer> {
    let player = window.use_keyed_state(id, cx, |_, _| SimulatedPlayer::new(&options));
    let handle = window.window_handle();
    let events = player.update(cx, |p, cx| {
        p.window = Some(handle);
        let events = p.configure(&options, duration, Instant::now());
        p.ensure_ticker(cx);
        if !events.is_empty() {
            cx.notify();
        }
        events
    });
    if !events.is_empty() {
        let player = player.clone();
        window.defer(cx, move |window, cx| {
            dispatch_events(&player, &events, window, cx)
        });
    }
    player
}

/// Creates the player of an [`AudioPlayer`] that gets no
/// [`AudioPlayer::player`] (`<AudioPlayer src=..>`): called every render
/// with the player's options; keep the player in keyed state under `id`.
pub type AudioPlayerBackend = fn(
    id: ElementId,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Rc<dyn AudioPlayerController>;

struct AudioPlayerBackendGlobal(AudioPlayerBackend);
impl Global for AudioPlayerBackendGlobal {}

/// Sets the engine behind `AudioPlayer::new(..).src(..)` (e.g. native playback
/// from the `device` feature).
pub fn set_audio_player_backend(backend: AudioPlayerBackend, cx: &mut App) {
    cx.set_global(AudioPlayerBackendGlobal(backend));
}

/// The default backend: a [`SimulatedPlayer`] that cannot decode anything, so
/// a source reports `Error` (no audio output is available).
fn no_audio_backend(
    id: ElementId,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Rc<dyn AudioPlayerController> {
    Rc::new(configure_simulated(id, None, options, window, cx))
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

/// What kind of element had focus when a key reached the [`AudioPlayer`]
/// (the DOM `event.target` checks of `shortcutFor`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShortcutTarget {
    /// A button: Space and K don't toggle playback (the button handles them).
    pub button: bool,
    /// An input, select, textarea or contenteditable: Space and K don't toggle.
    pub editable: bool,
    /// A slider (`type="range"` or `role="slider"`): arrows belong to it.
    pub slider: bool,
}

impl ShortcutTarget {
    /// Any other element.
    pub const OTHER: Self = Self {
        button: false,
        editable: false,
        slider: false,
    };
    pub const BUTTON: Self = Self {
        button: true,
        editable: false,
        slider: false,
    };
    /// A range input (Base UI slider thumb): editable and a slider.
    pub const RANGE: Self = Self {
        button: false,
        editable: true,
        slider: true,
    };
}

/// A player action from a shortcut key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShortcutAction {
    Toggle,
    Seek(f64),
    SetVolume(f32),
    SetMuted(bool),
}

/// `shortcutFor`: the player action for a key (GPUI key name), or `None` to
/// let the key through.
pub fn shortcut_for(
    key: &str,
    shift: bool,
    target: ShortcutTarget,
    state: &AudioPlayerState,
) -> Option<ShortcutAction> {
    let seek_amount = if shift { SEEK_LARGE_STEP } else { SEEK_STEP };
    let toggles_playback = !(target.button || target.editable);
    let slider_keys = !target.slider;
    match key {
        "space" if toggles_playback => Some(ShortcutAction::Toggle),
        "down" if slider_keys => Some(ShortcutAction::SetVolume(
            (state.volume - VOLUME_STEP).clamp(0.0, 1.0),
        )),
        "up" if slider_keys => Some(ShortcutAction::SetVolume(
            (state.volume + VOLUME_STEP).clamp(0.0, 1.0),
        )),
        "left" if slider_keys => Some(ShortcutAction::Seek(state.current_time - seek_amount)),
        "right" if slider_keys => Some(ShortcutAction::Seek(state.current_time + seek_amount)),
        "end" => Some(ShortcutAction::Seek(state.duration)),
        "home" => Some(ShortcutAction::Seek(0.0)),
        // `event.key` is "K" / "M" with Shift, which the React map doesn't match.
        "k" if !shift && toggles_playback => Some(ShortcutAction::Toggle),
        "m" if !shift => Some(ShortcutAction::SetMuted(!state.muted)),
        _ => None,
    }
}

fn apply_shortcut(
    action: ShortcutAction,
    player: &dyn AudioPlayerController,
    window: &mut Window,
    cx: &mut App,
) {
    match action {
        ShortcutAction::Toggle => player.toggle(window, cx),
        ShortcutAction::Seek(seconds) => player.seek(seconds, window, cx),
        ShortcutAction::SetVolume(volume) => player.set_volume(volume, window, cx),
        ShortcutAction::SetMuted(muted) => player.set_muted(muted, window, cx),
    }
}

/// Set by a part's key listener (bubble phase, before the root's), read by
/// the root: the GPUI stand-in for `event.target`.
type TargetMarker = Rc<Cell<ShortcutTarget>>;

// ---------------------------------------------------------------------------
// AudioPlayer
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AudioPlayerContext {
    player: Rc<dyn AudioPlayerController>,
    state: AudioPlayerState,
    on_previous: Option<Handler>,
    on_next: Option<Handler>,
    marker: TargetMarker,
}

fn use_part(part: &str) -> AudioPlayerContext {
    use_context::<AudioPlayerContext>()
        .unwrap_or_else(|| panic!("{part} must be used inside AudioPlayer."))
}

/// `useAudioPlayerContext`: the player controller of the surrounding
/// [`AudioPlayer`], while a part renders. `None` outside an `AudioPlayer`.
pub fn use_audio_player_context() -> Option<Rc<dyn AudioPlayerController>> {
    use_context::<AudioPlayerContext>().map(|context| context.player)
}

struct AudioPlayerRootState {
    marker: TargetMarker,
    reported_time: Option<f64>,
}

/// A composable audio player with transport, seeking, time, volume, rate, loop
/// and keyboard shortcuts. Parts read the player of the nearest `AudioPlayer`.
///
/// Keyboard, while focus is inside: Space or K play/pause, Left/Right seek 5 s
/// (Shift: 15 s), Up/Down volume, M mute, Home/End start or end.
///
/// ```ignore
/// AudioPlayer::new("player")
///     .player(player)
///     .child(AudioPlayerPlay::new())
///     .child(AudioPlayerSeek::new())
/// ```
#[derive(IntoElement)]
pub struct AudioPlayer {
    id: ElementId,
    player: Option<Rc<dyn AudioPlayerController>>,
    options: UseAudioPlayerOptions,
    on_time_update: Option<TimeHandler>,
    on_previous: Option<Handler>,
    on_next: Option<Handler>,
    shortcuts: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioPlayer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            player: None,
            options: UseAudioPlayerOptions::default(),
            on_time_update: None,
            on_previous: None,
            on_next: None,
            shortcuts: true,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Use an existing player (e.g. from [`use_simulated_player`]) instead of
    /// an internal one.
    pub fn player(mut self, player: impl AudioPlayerController) -> Self {
        self.player = Some(Rc::new(player));
        self
    }

    /// [`AudioPlayer::player`] for a shared controller.
    pub fn player_rc(mut self, player: Rc<dyn AudioPlayerController>) -> Self {
        self.player = Some(player);
        self
    }

    /// The internal player's source (ignored with [`AudioPlayer::player`]).
    pub fn src(mut self, src: impl Into<SharedString>) -> Self {
        self.options.src = Some(src.into());
        self
    }

    pub fn auto_play(mut self, auto_play: bool) -> Self {
        self.options.auto_play = auto_play;
        self
    }

    pub fn r#loop(mut self, r#loop: bool) -> Self {
        self.options.r#loop = r#loop;
        self
    }

    /// 0..1. Default 1.
    pub fn volume(mut self, volume: f32) -> Self {
        self.options.volume = volume;
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.options.muted = muted;
        self
    }

    /// Default 1.
    pub fn playback_rate(mut self, rate: f32) -> Self {
        self.options.playback_rate = rate;
        self
    }

    /// Default `Metadata`.
    pub fn preload(mut self, preload: AudioPreload) -> Self {
        self.options.preload = preload;
        self
    }

    /// Needed to analyse remote audio.
    pub fn cross_origin(mut self, cross_origin: CrossOrigin) -> Self {
        self.options.cross_origin = Some(cross_origin);
        self
    }

    pub fn on_play(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.options.on_play = Some(Rc::new(f));
        self
    }

    pub fn on_pause(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.options.on_pause = Some(Rc::new(f));
        self
    }

    pub fn on_ended(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.options.on_ended = Some(Rc::new(f));
        self
    }

    pub fn on_error(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.options.on_error = Some(Rc::new(f));
        self
    }

    /// Called when the current time changes.
    pub fn on_time_update(mut self, f: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_time_update = Some(Rc::new(f));
        self
    }

    /// Enables the previous button.
    pub fn on_previous(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_previous = Some(Rc::new(f));
        self
    }

    /// Enables the next button.
    pub fn on_next(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next = Some(Rc::new(f));
        self
    }

    /// Keyboard shortcuts while focus is inside. Default true.
    pub fn shortcuts(mut self, shortcuts: bool) -> Self {
        self.shortcuts = shortcuts;
        self
    }
}

impl ParentElement for AudioPlayer {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for AudioPlayer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioPlayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let player = match self.player {
            Some(player) => player,
            None => {
                let backend = cx
                    .try_global::<AudioPlayerBackendGlobal>()
                    .map_or(no_audio_backend as AudioPlayerBackend, |b| b.0);
                let id =
                    ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "player".into());
                backend(id, self.options.clone(), window, cx)
            }
        };
        let state = player.state(cx);
        let root = window.use_keyed_state(self.id.clone(), cx, |_, _| AudioPlayerRootState {
            marker: Rc::new(Cell::new(ShortcutTarget::OTHER)),
            reported_time: None,
        });

        // Reports time changes, not every render.
        let time = state.current_time;
        let changed = root.update(cx, |s, _| {
            let changed = s.reported_time != Some(time);
            s.reported_time = Some(time);
            changed
        });
        if changed && let Some(on_time_update) = self.on_time_update.clone() {
            window.defer(cx, move |window, cx| on_time_update(time, window, cx));
        }

        let marker = root.read(cx).marker.clone();
        let context = AudioPlayerContext {
            player: player.clone(),
            state,
            on_previous: self.on_previous,
            on_next: self.on_next,
            marker: marker.clone(),
        };

        let reset = marker.clone();
        let shortcuts = self.shortcuts;
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(12.0))
            .gap_y(px(8.0))
            .children(self.children)
            .capture_key_down(move |_, _, _| reset.set(ShortcutTarget::OTHER))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let target = marker.replace(ShortcutTarget::OTHER);
                let modifiers = event.keystroke.modifiers;
                if !shortcuts
                    || window.default_prevented()
                    || modifiers.platform
                    || modifiers.control
                {
                    return;
                }
                let state = player.state(cx);
                if let Some(action) =
                    shortcut_for(&event.keystroke.key, modifiers.shift, target, &state)
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    apply_shortcut(action, player.as_ref(), window, cx);
                }
            });
        el.style().refine(&self.style);
        provide(context, el)
    }
}

/// Marks the elements inside it for the [`AudioPlayer`]'s shortcuts, e.g. a
/// shadcn `Button` or `Toggle` placed in a player (a DOM `<button>` target):
/// Space then activates the control instead of toggling playback.
///
/// ```ignore
/// AudioPlayerKeyTarget::new(ShortcutTarget::BUTTON, Toggle::new("shuffle").icon(IconName::Shuffle))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerKeyTarget {
    target: ShortcutTarget,
    child: AnyElement,
}

impl AudioPlayerKeyTarget {
    pub fn new(target: ShortcutTarget, child: impl IntoElement) -> Self {
        Self {
            target,
            child: child.into_any_element(),
        }
    }
}

impl RenderOnce for AudioPlayerKeyTarget {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let marker = use_context::<AudioPlayerContext>().map(|c| c.marker);
        let target = self.target;
        div()
            .flex()
            .when_some(marker, |el, marker| {
                el.on_key_down(move |_, _, _| marker.set(target))
            })
            .child(self.child)
    }
}

// ---------------------------------------------------------------------------
// Artwork, title, description, controls
// ---------------------------------------------------------------------------

/// The cover image (`size-12 rounded-lg object-cover`). Decorative unless
/// [`alt`](AudioPlayerArtwork::alt) is set.
///
/// ```ignore
/// AudioPlayerArtwork::new("https://example.com/cover.png").alt("Night Drive album cover")
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerArtwork {
    src: ImageSource,
    alt: SharedString,
    style: StyleRefinement,
}

impl AudioPlayerArtwork {
    pub fn new(src: impl Into<ImageSource>) -> Self {
        Self {
            src: src.into(),
            alt: SharedString::default(),
            style: StyleRefinement::default(),
        }
    }

    /// Alternative text. Default "" (decorative).
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = alt.into();
        self
    }
}

impl Styled for AudioPlayerArtwork {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioPlayerArtwork {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = img(self.src)
            .size(px(48.0))
            .flex_shrink_0()
            .rounded(radius_lg(t))
            .bg(t.muted)
            .object_fit(ObjectFit::Cover);
        if !self.alt.is_empty() {
            el = el.aria_label(self.alt);
        }
        el.style().refine(&self.style);
        el
    }
}

macro_rules! text_part {
    ($(#[$doc:meta])* $name:ident, |$t:ident, $el:ident| $body:expr) => {
        $(#[$doc])*
        #[derive(IntoElement, Default)]
        pub struct $name {
            children: Vec<AnyElement>,
            style: StyleRefinement,
        }

        impl $name {
            pub fn new() -> Self {
                Self::default()
            }
        }

        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }

        impl RenderOnce for $name {
            fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
                let $t = cx.theme();
                let $el = div();
                let mut el = $body.children(self.children);
                el.style().refine(&self.style);
                el
            }
        }
    };
}

text_part!(
    /// The track title (`truncate text-sm font-medium`).
    ///
    /// ```ignore
    /// AudioPlayerTitle::new().child("Night Drive")
    /// ```
    AudioPlayerTitle,
    |t, el| {
        let _ = t;
        el.min_w_0()
            .truncate()
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
    }
);

text_part!(
    /// The artist or show (`text-muted-foreground truncate text-xs`).
    ///
    /// ```ignore
    /// AudioPlayerDescription::new().child("audiocn")
    /// ```
    AudioPlayerDescription,
    |t, el| el.min_w_0().truncate().text_xs_().text_color(t.muted_foreground)
);

text_part!(
    /// A row of transport buttons (`flex items-center gap-1`).
    ///
    /// ```ignore
    /// AudioPlayerControls::new().child(AudioPlayerSkipBack::new()).child(AudioPlayerPlay::new())
    /// ```
    AudioPlayerControls,
    |t, el| {
        let _ = t;
        el.flex().items_center().gap(px(4.0))
    }
);

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonKind {
    Plain,
    Play,
    Rate,
    Loop,
}

struct PlayerButtonState {
    focus: FocusHandle,
}

struct PlayerButtonSpec {
    id: ElementId,
    label: SharedString,
    kind: ButtonKind,
    disabled: bool,
    pressed: Option<bool>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_click: Handler,
}

/// `usePlayerButton`: a `size-8 rounded-full` button; a text label when it
/// has no children.
fn player_button(
    spec: PlayerButtonSpec,
    marker: TargetMarker,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let focus = window
        .use_keyed_state(
            spec.id.clone(),
            cx,
            |_, cx: &mut Context<PlayerButtonState>| PlayerButtonState {
                focus: cx.focus_handle().tab_stop(true),
            },
        )
        .read(cx)
        .focus
        .clone();
    let t = cx.theme();
    let focus_visible =
        !spec.disabled && focus.is_focused(window) && window.last_input_was_keyboard();
    let has_children = !spec.children.is_empty();
    let pressed = spec.pressed.unwrap_or(false);
    let (bg, fg, hover_bg) = match spec.kind {
        ButtonKind::Play => (
            Some(t.primary),
            t.primary_foreground,
            t.primary.opacity(0.85),
        ),
        ButtonKind::Loop if pressed => (Some(t.muted), t.primary, t.muted),
        _ => (None, t.foreground, t.muted),
    };
    let size = if spec.kind == ButtonKind::Play {
        px(40.0)
    } else {
        px(32.0)
    };
    let ring = t.ring.opacity(0.3);
    let label = spec.label.clone();
    let disabled = spec.disabled;

    let mut el = div()
        .id(spec.id)
        .relative()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(size)
        .rounded_full()
        .text_color(fg)
        .when_some(bg, |el, bg| el.bg(bg))
        .when(!disabled, |el| el.hover(move |s| s.bg(hover_bg)))
        .when(disabled, |el| el.opacity(0.4))
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .when_some(spec.pressed, |el, pressed| {
            el.aria_toggled(if pressed {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
        })
        .when(!has_children, |el| {
            el.w_auto()
                .px(px(10.0))
                .child(div().text_xs_().whitespace_nowrap().child(label))
        })
        .when(spec.kind == ButtonKind::Rate, |el| el.w_auto().px(px(8.0)))
        .when(focus_visible, |el| {
            el.child(ring_overlay(px(9999.0), 3.0, ring))
        })
        .children(spec.children);
    el.style().refine(&spec.style);
    if disabled {
        return el.into_any_element();
    }
    let on_click = spec.on_click;
    el.track_focus(&focus)
        .on_key_down(move |_, _, _| marker.set(ShortcutTarget::BUTTON))
        .on_click(move |_, window, cx| on_click(window, cx))
        .into_any_element()
}

macro_rules! button_part {
    ($name:ident, $slot:literal) => {
        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }

        impl $name {
            /// Element id for the button's focus state. Default
            #[doc = concat!("`\"", $slot, "\"`")]
            /// (unique per player).
            pub fn id(mut self, id: impl Into<ElementId>) -> Self {
                self.id = id.into();
                self
            }
        }
    };
}

/// State passed to [`AudioPlayerPlay::children_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioPlayerPlayState {
    pub playing: bool,
    pub loading: bool,
}

/// The play / pause button (`size-10 bg-primary`). Disabled while idle or
/// after an error.
///
/// ```ignore
/// AudioPlayerPlay::new().children_with(|s| {
///     Icon::new(if s.playing { IconName::Pause } else { IconName::Play }).fill().into_any_element()
/// })
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerPlay {
    id: ElementId,
    children: Vec<AnyElement>,
    children_with: Option<Rc<dyn Fn(AudioPlayerPlayState) -> AnyElement>>,
    style: StyleRefinement,
}

impl AudioPlayerPlay {
    pub fn new() -> Self {
        Self {
            id: "audio-player-play".into(),
            children: Vec::new(),
            children_with: None,
            style: StyleRefinement::default(),
        }
    }

    /// Children as a function of the state (`children={({ playing }) => ..}`).
    pub fn children_with(
        mut self,
        f: impl Fn(AudioPlayerPlayState) -> AnyElement + 'static,
    ) -> Self {
        self.children_with = Some(Rc::new(f));
        self
    }
}

impl Default for AudioPlayerPlay {
    fn default() -> Self {
        Self::new()
    }
}

button_part!(AudioPlayerPlay, "audio-player-play");

impl RenderOnce for AudioPlayerPlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerPlay");
        let state = &context.state;
        let loading = state.status == AudioPlayerStatus::Loading;
        let mut children = self.children;
        if let Some(f) = &self.children_with {
            children.push(f(AudioPlayerPlayState {
                playing: state.playing,
                loading,
            }));
        }
        let player = context.player.clone();
        player_button(
            PlayerButtonSpec {
                id: self.id,
                label: if state.playing { "Pause" } else { "Play" }.into(),
                kind: ButtonKind::Play,
                disabled: matches!(
                    state.status,
                    AudioPlayerStatus::Idle | AudioPlayerStatus::Error
                ),
                pressed: None,
                children,
                style: self.style,
                on_click: Rc::new(move |window, cx| player.toggle(window, cx)),
            },
            context.marker,
            window,
            cx,
        )
    }
}

/// The previous-track button. Enabled by [`AudioPlayer::on_previous`].
///
/// ```ignore
/// AudioPlayerPrevious::new().child(Icon::new(IconName::SkipBack))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerPrevious {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

/// The next-track button. Enabled by [`AudioPlayer::on_next`].
///
/// ```ignore
/// AudioPlayerNext::new().child(Icon::new(IconName::SkipForward))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerNext {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioPlayerPrevious {
    pub fn new() -> Self {
        Self {
            id: "audio-player-previous".into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Default for AudioPlayerPrevious {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayerNext {
    pub fn new() -> Self {
        Self {
            id: "audio-player-next".into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Default for AudioPlayerNext {
    fn default() -> Self {
        Self::new()
    }
}

button_part!(AudioPlayerPrevious, "audio-player-previous");
button_part!(AudioPlayerNext, "audio-player-next");

fn track_button(
    id: ElementId,
    label: &'static str,
    handler: Option<Handler>,
    (children, style): (Vec<AnyElement>, StyleRefinement),
    marker: TargetMarker,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let disabled = handler.is_none();
    player_button(
        PlayerButtonSpec {
            id,
            label: label.into(),
            kind: ButtonKind::Plain,
            disabled,
            pressed: None,
            children,
            style,
            on_click: Rc::new(move |window, cx| {
                if let Some(handler) = &handler {
                    handler(window, cx);
                }
            }),
        },
        marker,
        window,
        cx,
    )
}

impl RenderOnce for AudioPlayerPrevious {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerPrevious");
        track_button(
            self.id,
            "Previous",
            context.on_previous,
            (self.children, self.style),
            context.marker,
            window,
            cx,
        )
    }
}

impl RenderOnce for AudioPlayerNext {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerNext");
        track_button(
            self.id,
            "Next",
            context.on_next,
            (self.children, self.style),
            context.marker,
            window,
            cx,
        )
    }
}

/// Seeks back `seconds` (default 10). Label "Back 10 seconds".
///
/// ```ignore
/// AudioPlayerSkipBack::new().child(Icon::new(IconName::SkipBack))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerSkipBack {
    id: ElementId,
    seconds: f64,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

/// Seeks forward `seconds` (default 10). Label "Forward 10 seconds".
///
/// ```ignore
/// AudioPlayerSkipForward::new().seconds(30.0).child(Icon::new(IconName::SkipForward))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerSkipForward {
    id: ElementId,
    seconds: f64,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

macro_rules! skip_part {
    ($name:ident, $slot:literal) => {
        impl $name {
            pub fn new() -> Self {
                Self {
                    id: $slot.into(),
                    seconds: 10.0,
                    children: Vec::new(),
                    style: StyleRefinement::default(),
                }
            }

            /// Default 10.
            pub fn seconds(mut self, seconds: f64) -> Self {
                self.seconds = seconds;
                self
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        button_part!($name, $slot);
    };
}

skip_part!(AudioPlayerSkipBack, "audio-player-skip-back");
skip_part!(AudioPlayerSkipForward, "audio-player-skip-forward");

/// JS number formatting for labels (`10`, `2.5`).
fn js_number(value: f64) -> String {
    format!("{value}")
}

fn skip_button(
    part: &str,
    id: ElementId,
    label: String,
    delta: f64,
    (children, style): (Vec<AnyElement>, StyleRefinement),
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let context = use_part(part);
    let player = context.player.clone();
    player_button(
        PlayerButtonSpec {
            id,
            label: label.into(),
            kind: ButtonKind::Plain,
            disabled: false,
            pressed: None,
            children,
            style,
            on_click: Rc::new(move |window, cx| {
                let current = player.state(cx).current_time;
                player.seek(current + delta, window, cx);
            }),
        },
        context.marker,
        window,
        cx,
    )
}

impl RenderOnce for AudioPlayerSkipBack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        skip_button(
            "AudioPlayerSkipBack",
            self.id,
            format!("Back {} seconds", js_number(self.seconds)),
            -self.seconds,
            (self.children, self.style),
            window,
            cx,
        )
    }
}

impl RenderOnce for AudioPlayerSkipForward {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        skip_button(
            "AudioPlayerSkipForward",
            self.id,
            format!("Forward {} seconds", js_number(self.seconds)),
            self.seconds,
            (self.children, self.style),
            window,
            cx,
        )
    }
}

/// Cycles the playback speed through `rates` (default 0.5×–2×) on click.
///
/// ```ignore
/// AudioPlayerRate::new().rates(vec![1.0, 1.5, 2.0])
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerRate {
    id: ElementId,
    rates: Vec<f32>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioPlayerRate {
    pub fn new() -> Self {
        Self {
            id: "audio-player-rate".into(),
            rates: DEFAULT_RATES.to_vec(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Default `[0.5, 0.75, 1, 1.25, 1.5, 2]`.
    pub fn rates(mut self, rates: Vec<f32>) -> Self {
        self.rates = rates;
        self
    }
}

impl Default for AudioPlayerRate {
    fn default() -> Self {
        Self::new()
    }
}

button_part!(AudioPlayerRate, "audio-player-rate");

/// The rate after `current` in `rates` (the first one when `current` is not
/// listed, 1 when `rates` is empty).
pub fn next_rate(rates: &[f32], current: f32) -> f32 {
    let index = rates.iter().position(|&rate| rate == current);
    let next = index.map_or(0, |i| (i + 1) % rates.len().max(1));
    rates.get(next).copied().unwrap_or(1.0)
}

/// The width of `0` in `family` at `size` (CSS `ch`).
pub(crate) fn ch_width(family: SharedString, size: Pixels, window: &Window) -> Pixels {
    let text = window.text_system();
    let font_id = text.resolve_font(&gpui::font(family));
    text.ch_width(font_id, size).unwrap_or(size * 0.6)
}

impl RenderOnce for AudioPlayerRate {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerRate");
        let rate = context.state.playback_rate;
        let next = next_rate(&self.rates, rate);
        let mut children = self.children;
        if children.is_empty() {
            let t = cx.theme();
            let longest = self
                .rates
                .iter()
                .map(|rate| format!("{rate}×").chars().count())
                .max()
                .unwrap_or(0);
            let ch = ch_width(t.font_mono.clone(), px(12.0), window);
            children.push(
                div()
                    .min_w(ch * longest as f32)
                    .text_center()
                    .font_family(t.font_mono.clone())
                    .text_xs_()
                    .child(format!("{rate}×"))
                    .into_any_element(),
            );
        }
        let player = context.player.clone();
        player_button(
            PlayerButtonSpec {
                id: self.id,
                label: format!("Playback speed {rate}×").into(),
                kind: ButtonKind::Rate,
                disabled: false,
                pressed: None,
                children,
                style: self.style,
                on_click: Rc::new(move |window, cx| player.set_playback_rate(next, window, cx)),
            },
            context.marker,
            window,
            cx,
        )
    }
}

/// Toggles looping (`aria-pressed`: `bg-muted text-primary`).
///
/// ```ignore
/// AudioPlayerLoop::new().child(Icon::new(IconName::Repeat))
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerLoop {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioPlayerLoop {
    pub fn new() -> Self {
        Self {
            id: "audio-player-loop".into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Default for AudioPlayerLoop {
    fn default() -> Self {
        Self::new()
    }
}

button_part!(AudioPlayerLoop, "audio-player-loop");

impl RenderOnce for AudioPlayerLoop {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerLoop");
        let looping = context.state.r#loop;
        let player = context.player.clone();
        player_button(
            PlayerButtonSpec {
                id: self.id,
                label: if looping { "Loop on" } else { "Loop off" }.into(),
                kind: ButtonKind::Loop,
                disabled: false,
                pressed: Some(looping),
                children: self.children,
                style: self.style,
                on_click: Rc::new(move |window, cx| {
                    let looping = player.state(cx).r#loop;
                    player.set_loop(!looping, window, cx);
                }),
            },
            context.marker,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------
// Seek
// ---------------------------------------------------------------------------

struct SeekState {
    focus: FocusHandle,
    drag: Option<f64>,
    track: Rc<Cell<Option<Bounds<Pixels>>>>,
}

/// The seek bar: a slider over the track with the buffered range, the
/// played range and a thumb. Arrows seek `step` seconds (Shift:
/// `large_step`), Home/End jump; dragging seeks on release.
///
/// ```ignore
/// AudioPlayerSeek::new().step(10.0)
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerSeek {
    id: ElementId,
    step: f64,
    large_step: f64,
    style: StyleRefinement,
}

impl AudioPlayerSeek {
    pub fn new() -> Self {
        Self {
            id: "audio-player-seek".into(),
            step: SEEK_STEP,
            large_step: SEEK_LARGE_STEP,
            style: StyleRefinement::default(),
        }
    }

    /// Element id (unique per player). Default `"audio-player-seek"`.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Seconds per arrow key. Default 5.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Seconds per Shift+arrow. Default 15.
    pub fn large_step(mut self, large_step: f64) -> Self {
        self.large_step = large_step;
        self
    }
}

impl Default for AudioPlayerSeek {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for AudioPlayerSeek {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The value the seek thumb moves to for a key, or `None` (Base UI slider
/// keys plus the `onKeyDown` override for Left/Right).
pub fn seek_key_value(
    key: &str,
    shift: bool,
    value: f64,
    max: f64,
    step: f64,
    large_step: f64,
) -> Option<f64> {
    let amount = if shift { large_step } else { step };
    let fine = if shift { large_step } else { 0.01 };
    match key {
        "right" => Some(value + amount),
        "left" => Some(value - amount),
        "up" => Some(clamp_js(value + fine, 0.0, max)),
        "down" => Some(clamp_js(value - fine, 0.0, max)),
        "pageup" => Some(clamp_js(value + large_step, 0.0, max)),
        "pagedown" => Some(clamp_js(value - large_step, 0.0, max)),
        "home" => Some(0.0),
        "end" => Some(max),
        _ => None,
    }
}

fn value_at(x: Pixels, track: Bounds<Pixels>, max: f64) -> f64 {
    let width = f32::from(track.size.width).max(1.0);
    let fraction = (f32::from(x - track.origin.x) / width).clamp(0.0, 1.0);
    // step 0.01
    (f64::from(fraction) * max * 100.0).round() / 100.0
}

impl RenderOnce for AudioPlayerSeek {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerSeek");
        let seek = window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<SeekState>| {
            SeekState {
                focus: cx.focus_handle().tab_stop(true),
                drag: None,
                track: Rc::new(Cell::new(None)),
            }
        });
        let state = &context.state;
        let duration = if state.duration.is_finite() {
            state.duration.max(0.0)
        } else {
            0.0
        };
        let disabled = duration == 0.0;
        let max = duration.max(0.001);
        let (focus, drag, track_cell) = {
            let s = seek.read(cx);
            (s.focus.clone(), s.drag, s.track.clone())
        };
        let time = context
            .player
            .time(cx)
            .latest()
            .unwrap_or(state.current_time);
        let value = clamp_js(drag.unwrap_or(time), 0.0, max);
        let fraction = (value / max) as f32;
        let buffered = if duration > 0.0 {
            (state.buffered / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        if state.playing && drag.is_none() {
            window.request_animation_frame();
        }

        let t = cx.theme();
        let focus_visible =
            !disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let thumb_ring = if focus_visible {
            Some(ring_shadow(4.0, t.ring.opacity(0.4)))
        } else {
            None
        };
        let mut thumb_shadow = shadow_sm();
        thumb_shadow.push(ring_shadow(1.0, t.foreground.opacity(0.15)));
        if let Some(ring) = thumb_ring {
            thumb_shadow.push(ring);
        }
        let mut hover_shadow = shadow_sm();
        hover_shadow.push(ring_shadow(4.0, t.ring.opacity(0.3)));

        let thumb = div()
            .id("thumb")
            .absolute()
            .top(px(-4.0))
            .left(relative(fraction))
            .ml(px(-6.0))
            .size(px(12.0))
            .flex_shrink_0()
            .rounded_full()
            .bg(t.background)
            .shadow(thumb_shadow)
            .when(!disabled && !focus_visible, |el| {
                el.hover(move |s| s.shadow(hover_shadow.clone()))
            })
            .role(gpui::Role::Slider)
            .aria_label("Seek");

        let track = div()
            .relative()
            .h(px(4.0))
            .w_full()
            .rounded_full()
            .bg(t.input.opacity(0.9))
            .child(
                canvas(
                    move |bounds, _, _| track_cell.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(relative(buffered))
                    .rounded_full()
                    .bg(t.muted_foreground.opacity(0.25)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(relative(fraction))
                    .rounded_full()
                    .bg(t.primary),
            )
            .child(thumb);

        // While dragging, follow the pointer anywhere in the window.
        let drag_listener = drag.is_some().then(|| {
            let seek = seek.clone();
            let player = context.player.clone();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let move_seek = seek.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if !phase.bubble() {
                            return;
                        }
                        move_seek.update(cx, |s, cx| {
                            if let (Some(_), Some(track)) = (s.drag, s.track.get()) {
                                s.drag = Some(value_at(event.position.x, track, max));
                                cx.notify();
                            }
                        });
                    });
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                        if !phase.bubble() {
                            return;
                        }
                        let committed = seek.update(cx, |s, cx| {
                            cx.notify();
                            s.drag.take()
                        });
                        if let Some(value) = committed {
                            player.seek(value, window, cx);
                        }
                    });
                },
            )
            .absolute()
            .size_0()
        });

        let control = div()
            .relative()
            .flex()
            .h(px(16.0))
            .w_full()
            .items_center()
            .px(px(6.0))
            .child(track)
            .children(drag_listener);

        let mut el = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_1()
            .min_w(px(96.0))
            .items_center()
            .child(control);
        el.style().refine(&self.style);
        if disabled {
            return el;
        }

        // `before:-inset-y-1.5`: a taller hit area around the 16px control.
        let hit_seek = seek.clone();
        let hit_focus = focus.clone();
        let hit = div()
            .id("hit")
            .absolute()
            .left_0()
            .right_0()
            .top(px(-6.0))
            .bottom(px(-6.0))
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                window.focus(&hit_focus, cx);
                hit_seek.update(cx, |s, cx| {
                    if let Some(track) = s.track.get() {
                        s.drag = Some(value_at(event.position.x, track, max));
                        cx.notify();
                    }
                });
            });

        let player = context.player.clone();
        let marker = context.marker.clone();
        let (step, large_step) = (self.step, self.large_step);
        el.child(hit)
            .track_focus(&focus)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                // `type="range"`: editable and a slider for the root's shortcuts.
                marker.set(ShortcutTarget::RANGE);
                let key = event.keystroke.key.as_str();
                if key == "space" {
                    cx.stop_propagation();
                    return;
                }
                let current = player
                    .time(cx)
                    .latest()
                    .unwrap_or_else(|| player.state(cx).current_time);
                let value = clamp_js(current, 0.0, max);
                if let Some(next) = seek_key_value(
                    key,
                    event.keystroke.modifiers.shift,
                    value,
                    max,
                    step,
                    large_step,
                ) {
                    cx.stop_propagation();
                    window.prevent_default();
                    player.seek(next, window, cx);
                }
            })
    }
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// `type` of [`AudioPlayerTime`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioPlayerTimeType {
    #[default]
    Current,
    Remaining,
    Duration,
}

type TimeFormat = Rc<dyn Fn(f64, AudioPlayerTimeType) -> String>;

/// `defaultTimeFormat`: `formatTime`, with a minus sign for `Remaining`.
pub fn default_time_format(seconds: f64, kind: AudioPlayerTimeType) -> String {
    format_time(
        seconds,
        FormatTimeOptions {
            remaining: kind == AudioPlayerTimeType::Remaining,
            ..Default::default()
        },
    )
}

/// The seconds an [`AudioPlayerTime`] of `kind` shows for `state`.
pub fn time_seconds(kind: AudioPlayerTimeType, state: &AudioPlayerState) -> f64 {
    match kind {
        AudioPlayerTimeType::Current => state.current_time,
        AudioPlayerTimeType::Duration => state.duration,
        AudioPlayerTimeType::Remaining => (state.duration - state.current_time).max(0.0),
    }
}

/// The current, remaining or total time (`font-mono text-xs`), as wide as the
/// longest time the track can show so it never shifts.
///
/// ```ignore
/// AudioPlayerTime::new().r#type(AudioPlayerTimeType::Remaining)
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerTime {
    kind: AudioPlayerTimeType,
    format: TimeFormat,
    style: StyleRefinement,
}

impl AudioPlayerTime {
    pub fn new() -> Self {
        Self {
            kind: AudioPlayerTimeType::Current,
            format: Rc::new(default_time_format),
            style: StyleRefinement::default(),
        }
    }

    /// Default `Current`.
    pub fn r#type(mut self, kind: AudioPlayerTimeType) -> Self {
        self.kind = kind;
        self
    }

    /// Default [`default_time_format`].
    pub fn format(mut self, f: impl Fn(f64, AudioPlayerTimeType) -> String + 'static) -> Self {
        self.format = Rc::new(f);
        self
    }
}

impl Default for AudioPlayerTime {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for AudioPlayerTime {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioPlayerTime {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerTime");
        let state = &context.state;
        let format = &self.format;
        let width = format(0.0, self.kind)
            .chars()
            .count()
            .max(format(state.duration, self.kind).chars().count());
        let text = format(time_seconds(self.kind, state), self.kind);
        let t = cx.theme();
        let ch = ch_width(t.font_mono.clone(), px(12.0), window);
        let mut el = div()
            .min_w(ch * width as f32)
            .text_right()
            .font_family(t.font_mono.clone())
            .text_xs_()
            .whitespace_nowrap()
            .text_color(t.muted_foreground)
            .child(text);
        el.style().refine(&self.style);
        el
    }
}

// ---------------------------------------------------------------------------
// Volume
// ---------------------------------------------------------------------------

/// The volume control bound to the player (`w-32`): a
/// [`VolumeControl`](crate::VolumeControl) with a mute button and slider by
/// default, or your own `VolumeControlMute` / `VolumeControlSlider` children.
///
/// ```ignore
/// AudioPlayerVolume::new()
/// ```
#[derive(IntoElement)]
pub struct AudioPlayerVolume {
    id: ElementId,
    step: Option<f32>,
    curve: Option<VolumeCurve>,
    orientation: Option<Orientation>,
    size: Option<AudioSize>,
    disabled: Option<bool>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioPlayerVolume {
    pub fn new() -> Self {
        Self {
            id: "audio-player-volume".into(),
            step: None,
            curve: None,
            orientation: None,
            size: None,
            disabled: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// See [`VolumeControl::step`](crate::VolumeControl::step).
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// See [`VolumeControl::curve`](crate::VolumeControl::curve).
    pub fn curve(mut self, curve: VolumeCurve) -> Self {
        self.curve = Some(curve);
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }
}

impl Default for AudioPlayerVolume {
    fn default() -> Self {
        Self::new()
    }
}

button_part!(AudioPlayerVolume, "audio-player-volume");

impl RenderOnce for AudioPlayerVolume {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let context = use_part("AudioPlayerVolume");
        let (on_volume, on_muted) = (context.player.clone(), context.player.clone());
        let mut control = VolumeControl::new(self.id)
            .w(px(128.0))
            .value(context.state.volume)
            .muted(context.state.muted)
            .on_value_change(move |volume, window, cx| on_volume.set_volume(volume, window, cx))
            .on_muted_change(move |muted, window, cx| on_muted.set_muted(muted, window, cx))
            .children(self.children);
        if let Some(step) = self.step {
            control = control.step(step);
        }
        if let Some(curve) = self.curve {
            control = control.curve(curve);
        }
        if let Some(orientation) = self.orientation {
            control = control.orientation(orientation);
        }
        if let Some(size) = self.size {
            control = control.size(size);
        }
        if let Some(disabled) = self.disabled {
            control = control.disabled(disabled);
        }
        control.style().refine(&self.style);
        // The mute button is a `<button>` and the slider thumb an
        // `<input type="range">`: Space and K never toggle playback there.
        // The slider stops the arrow keys itself; on the mute button they
        // still change the volume, like the DOM version.
        AudioPlayerKeyTarget::new(ShortcutTarget::BUTTON, control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller_state() -> AudioPlayerState {
        AudioPlayerState {
            status: AudioPlayerStatus::Paused,
            current_time: 84.0,
            duration: 220.0,
            ..Default::default()
        }
    }

    #[test]
    fn renders_parts_bound_to_an_external_player() {
        let state = controller_state();
        let current = AudioPlayerTimeType::Current;
        let remaining = AudioPlayerTimeType::Remaining;
        assert_eq!(
            default_time_format(time_seconds(current, &state), current),
            "1:24"
        );
        assert_eq!(
            default_time_format(time_seconds(remaining, &state), remaining),
            "\u{2212}2:16"
        );
    }

    #[test]
    fn handles_keyboard_shortcuts() {
        let state = controller_state();
        let other = ShortcutTarget::OTHER;
        assert_eq!(
            shortcut_for("right", false, other, &state),
            Some(ShortcutAction::Seek(89.0))
        );
        assert_eq!(
            shortcut_for("m", false, other, &state),
            Some(ShortcutAction::SetMuted(true))
        );
        assert_eq!(
            shortcut_for("k", false, other, &state),
            Some(ShortcutAction::Toggle)
        );
        assert_eq!(
            shortcut_for("left", true, other, &state),
            Some(ShortcutAction::Seek(69.0))
        );
        assert_eq!(
            shortcut_for("home", false, other, &state),
            Some(ShortcutAction::Seek(0.0))
        );
        assert_eq!(
            shortcut_for("end", false, other, &state),
            Some(ShortcutAction::Seek(220.0))
        );
        assert_eq!(
            shortcut_for("down", false, other, &state),
            Some(ShortcutAction::SetVolume(0.95))
        );
        assert_eq!(
            shortcut_for("up", false, other, &state),
            Some(ShortcutAction::SetVolume(1.0))
        );
        assert_eq!(shortcut_for("x", false, other, &state), None);
    }

    #[test]
    fn shortcuts_leave_buttons_and_sliders_alone() {
        let state = controller_state();
        assert_eq!(
            shortcut_for("space", false, ShortcutTarget::BUTTON, &state),
            None
        );
        assert_eq!(
            shortcut_for("k", false, ShortcutTarget::BUTTON, &state),
            None
        );
        assert_eq!(
            shortcut_for("right", false, ShortcutTarget::BUTTON, &state),
            Some(ShortcutAction::Seek(89.0))
        );
        assert_eq!(
            shortcut_for("right", false, ShortcutTarget::RANGE, &state),
            None
        );
        assert_eq!(
            shortcut_for("space", false, ShortcutTarget::RANGE, &state),
            None
        );
        assert_eq!(
            shortcut_for("m", false, ShortcutTarget::RANGE, &state),
            Some(ShortcutAction::SetMuted(true))
        );
        assert_eq!(
            shortcut_for("space", false, ShortcutTarget::OTHER, &state),
            Some(ShortcutAction::Toggle)
        );
    }

    #[test]
    fn seek_keys_follow_the_slider() {
        assert_eq!(
            seek_key_value("right", false, 10.0, 60.0, 5.0, 15.0),
            Some(15.0)
        );
        assert_eq!(
            seek_key_value("left", true, 20.0, 60.0, 5.0, 15.0),
            Some(5.0)
        );
        assert_eq!(
            seek_key_value("home", false, 20.0, 60.0, 5.0, 15.0),
            Some(0.0)
        );
        assert_eq!(
            seek_key_value("end", false, 20.0, 60.0, 5.0, 15.0),
            Some(60.0)
        );
        assert_eq!(
            seek_key_value("pageup", false, 50.0, 60.0, 5.0, 15.0),
            Some(60.0)
        );
        assert_eq!(seek_key_value("a", false, 50.0, 60.0, 5.0, 15.0), None);
    }

    #[test]
    fn cycles_rates() {
        assert_eq!(next_rate(&DEFAULT_RATES, 1.0), 1.25);
        assert_eq!(next_rate(&DEFAULT_RATES, 2.0), 0.5);
        assert_eq!(next_rate(&DEFAULT_RATES, 3.0), 0.5);
        assert_eq!(next_rate(&[], 1.5), 1.0);
    }

    fn loaded(duration: f64, options: UseAudioPlayerOptions) -> (SimulatedPlayer, Instant) {
        let now = Instant::now();
        let mut player = SimulatedPlayer::new(&options);
        player.configure(&options, Some(duration), now);
        (player, now)
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn simulated_player_loads_plays_and_pauses() {
        let idle = SimulatedPlayer::new(&UseAudioPlayerOptions::default());
        assert_eq!(idle.state().status, AudioPlayerStatus::Idle);

        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        let state = player.state();
        assert_eq!(state.status, AudioPlayerStatus::Ready);
        assert_eq!(state.duration, 10.0);
        assert_eq!(state.buffered, 10.0);

        assert_eq!(player.play_at(t0), vec![PlayerEvent::Play]);
        assert!(player.state().playing);
        assert!(player.play_at(t0).is_empty());

        let (events, changed) = player.tick(t0 + ms(100));
        assert!(events.is_empty());
        assert!(!changed, "timeupdate is ~4 Hz");
        assert!((player.time_source().latest().unwrap() - 0.1).abs() < 1e-6);
        let (_, changed) = player.tick(t0 + ms(300));
        assert!(changed);
        assert!((player.state().current_time - 0.3).abs() < 1e-6);

        assert_eq!(player.pause_at(t0 + ms(500)), vec![PlayerEvent::Pause]);
        let state = player.state();
        assert_eq!(state.status, AudioPlayerStatus::Paused);
        assert!((state.current_time - 0.5).abs() < 1e-6);
        assert!(player.pause_at(t0 + ms(600)).is_empty());
    }

    #[test]
    fn simulated_player_ends_and_restarts() {
        let (mut player, t0) = loaded(1.0, UseAudioPlayerOptions::src("a.mp3"));
        player.play_at(t0);
        let (events, _) = player.tick(t0 + ms(1200));
        assert_eq!(events, vec![PlayerEvent::Pause, PlayerEvent::Ended]);
        let state = player.state();
        assert_eq!(state.status, AudioPlayerStatus::Ended);
        assert_eq!(state.current_time, 1.0);
        // A pause after the end keeps `ended`.
        assert!(player.pause_at(t0 + ms(1300)).is_empty());
        assert_eq!(player.state().status, AudioPlayerStatus::Ended);
        // Play from the start once ended.
        player.play_at(t0 + ms(2000));
        assert_eq!(player.state().current_time, 0.0);
    }

    #[test]
    fn simulated_player_loops_and_follows_the_rate() {
        let options = UseAudioPlayerOptions {
            r#loop: true,
            ..UseAudioPlayerOptions::src("a.mp3")
        };
        let (mut player, t0) = loaded(1.0, options);
        player.play_at(t0);
        let (events, _) = player.tick(t0 + ms(1250));
        assert!(events.is_empty());
        assert!(player.state().playing);
        assert!((player.position(t0 + ms(1250)) - 0.25).abs() < 1e-6);

        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        player.play_at(t0);
        player.set_playback_rate_at(2.0, t0 + ms(1000));
        assert!((player.position(t0 + ms(2000)) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn simulated_player_seeks_within_the_track() {
        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        player.seek_at(-3.0, t0);
        assert_eq!(player.state().current_time, 0.0);
        player.seek_at(25.0, t0);
        assert_eq!(player.state().current_time, 10.0);
        player.seek_at(4.0, t0);
        assert_eq!(player.time_source().latest(), Some(4.0));
    }

    #[test]
    fn keeps_a_volume_set_imperatively_when_another_prop_changes() {
        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        player.volume = 0.3;
        let options = UseAudioPlayerOptions {
            muted: true,
            ..UseAudioPlayerOptions::src("a.mp3")
        };
        player.configure(&options, Some(10.0), t0);
        let state = player.state();
        assert!((state.volume - 0.3).abs() < 1e-6);
        assert!(state.muted);
    }

    #[test]
    fn doesnt_reload_when_auto_play_or_preload_changes() {
        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        player.seek_at(4.0, t0);
        let options = UseAudioPlayerOptions {
            auto_play: true,
            preload: AudioPreload::Auto,
            ..UseAudioPlayerOptions::src("a.mp3")
        };
        assert!(player.configure(&options, Some(10.0), t0).is_empty());
        let state = player.state();
        assert_eq!(state.current_time, 4.0);
        assert_eq!(state.status, AudioPlayerStatus::Ready);
    }

    #[test]
    fn stops_the_old_track_when_the_source_is_cleared() {
        let (mut player, t0) = loaded(10.0, UseAudioPlayerOptions::src("a.mp3"));
        player.play_at(t0);
        player.configure(&UseAudioPlayerOptions::default(), None, t0 + ms(500));
        let state = player.state();
        assert_eq!(state.status, AudioPlayerStatus::Idle);
        assert!(!state.playing);
        assert_eq!(state.duration, 0.0);
        assert!(player.src().is_none());
    }

    #[test]
    fn auto_plays_a_new_source_and_reports_errors() {
        let options = UseAudioPlayerOptions {
            auto_play: true,
            ..UseAudioPlayerOptions::src("a.mp3")
        };
        let mut player = SimulatedPlayer::new(&options);
        let events = player.configure(&options, Some(5.0), Instant::now());
        assert_eq!(events, vec![PlayerEvent::Play]);
        assert!(player.state().playing);

        let mut broken = SimulatedPlayer::new(&UseAudioPlayerOptions::default());
        let events = broken.configure(&UseAudioPlayerOptions::src("b.mp3"), None, Instant::now());
        assert_eq!(events, vec![PlayerEvent::Error]);
        assert_eq!(broken.state().status, AudioPlayerStatus::Error);
        assert!(broken.state().error.is_some());
        assert!(broken.play_at(Instant::now()).is_empty());
    }
}
