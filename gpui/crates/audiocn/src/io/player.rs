//! Port of `hooks/use-audio-player.ts` with real playback: the native
//! [`AudioPlayerController`] behind [`AudioPlayer`](crate::AudioPlayer).
//!
//! [`AudioPlayerEngine`] is a GPUI entity with the media element's state
//! machine (`idle → loading → ready → playing ⇄ paused → ended`, `error`),
//! the same callbacks and quirks as [`SimulatedPlayer`](crate::SimulatedPlayer),
//! and a voice on the audio thread that decodes nothing at play time: the
//! whole file is decoded into memory on a background thread when `src` is
//! set (so `buffered` jumps to the duration once ready). The voice also
//! feeds an analyser, so a player has
//! [`meter_source`](AudioPlayerEngine::meter_source) /
//! [`visual_source`](AudioPlayerEngine::visual_source) for visualizers
//! (pre-volume, like `useAudioAnalyser(element)`).
//!
//! ```ignore
//! // Every <AudioPlayer src=..> plays for real:
//! audiocn::io::install_audio_player_backend(cx);
//! // Or explicitly:
//! let player = use_audio_player("episode", UseAudioPlayerOptions::src("/audio/episode.mp3"), window, cx);
//! let visual = player.read(cx).visual_source();
//! AudioPlayer::new("player").player(player).child(AudioPlayerPlay::new())
//! ```

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Context, ElementId, Entity, SharedString, Task,
    WeakEntity, Window,
};
use parking_lot::Mutex;

use super::analyser::{AnalyserTap, AnalyserTapOptions, TapWriter};
use super::buffer::AudioBuffer;
use super::context::{AtomicF32, AtomicF64, AudioSource, Destination};
use super::gain::{GainParam, Ramp};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::types::{MeterFrame, VisualFrame};
use crate::components::{
    AudioPlayerController, AudioPlayerState, AudioPlayerStatus, PlayerEvent, UseAudioPlayerOptions,
    set_audio_player_backend,
};
use crate::motion::{Duration, Instant};

/// How often `current_time` updates (`timeupdate`, ~4 Hz).
const TIME_UPDATE_INTERVAL: Duration = Duration::from_millis(250);
/// The ticker's period while playing.
const TICK_INTERVAL: Duration = Duration::from_millis(16);
/// Volume ramp time constant.
const VOLUME_RAMP_SECONDS: f32 = 0.005;

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type ErrorHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;

#[derive(Clone, Default)]
struct Callbacks {
    on_play: Option<Handler>,
    on_pause: Option<Handler>,
    on_ended: Option<Handler>,
    on_error: Option<ErrorHandler>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Applied {
    volume: Option<f32>,
    muted: Option<bool>,
    playback_rate: Option<f32>,
}

// ---------------------------------------------------------------------------
// The audio-thread side
// ---------------------------------------------------------------------------

struct PlayerShared {
    buffer: Mutex<Option<AudioBuffer>>,
    playing: AtomicBool,
    /// Seconds, written by the voice.
    position: AtomicF64,
    /// A seek for the voice to apply, in seconds (NaN: none).
    seek: AtomicF64,
    rate: AtomicF32,
    looping: AtomicBool,
    /// Set by the voice when a non-looping track reaches its end.
    ended: AtomicBool,
    time: FrameEmitter<f64>,
}

struct PlayerVoice {
    shared: Arc<PlayerShared>,
    volume: Ramp,
    tap: TapWriter,
    alive: Arc<AtomicBool>,
    scratch: Vec<f32>,
    /// Playhead in buffer frames.
    position: f64,
}

impl AudioSource for PlayerVoice {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        let shared = &self.shared;
        let buffer = shared.buffer.lock().clone();
        self.scratch.clear();
        self.scratch.resize(out.len(), 0.0);
        if let Some(buffer) = buffer {
            let rate = buffer.sample_rate() as f64;
            let seek = shared.seek.swap(f64::NAN);
            if !seek.is_nan() {
                self.position = seek * rate;
            }
            if shared.playing.load(Ordering::Acquire) {
                let length = buffer.length();
                let step = shared.rate.load().max(0.0) as f64 * rate / sample_rate.max(1) as f64;
                let looping = shared.looping.load(Ordering::Acquire);
                for pair in self.scratch.as_chunks_mut::<2>().0.iter_mut() {
                    if self.position >= length as f64 {
                        if looping && length > 0 {
                            self.position %= length as f64;
                        } else {
                            self.position = length as f64;
                            shared.playing.store(false, Ordering::Release);
                            shared.ended.store(true, Ordering::Release);
                            break;
                        }
                    }
                    let index = self.position as usize;
                    let fraction = (self.position - index as f64) as f32;
                    let next = if index + 1 >= length && looping {
                        0
                    } else {
                        index + 1
                    };
                    let a = buffer.frame(index);
                    let b = buffer.frame(next);
                    pair[0] = a[0] + (b[0] - a[0]) * fraction;
                    pair[1] = a[1] + (b[1] - a[1]) * fraction;
                    self.position += step;
                }
                let seconds = self.position / rate;
                shared.position.store(seconds);
                shared.time.emit(seconds);
            } else {
                shared.position.store(self.position / rate);
            }
        }
        // Paused players feed silence, so visualizers fall like Web Audio's.
        self.tap.write(&self.scratch, 2);
        self.volume.begin(sample_rate);
        for (out, pair) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(self.scratch.as_chunks::<2>().0.iter())
        {
            let gain = self.volume.next();
            out[0] += pair[0] * gain;
            out[1] += pair[1] * gain;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// The engine (GPUI side)
// ---------------------------------------------------------------------------

/// A native audio player: `useAudioPlayer`'s element and state in one
/// entity. `Entity<AudioPlayerEngine>` implements [`AudioPlayerController`].
pub struct AudioPlayerEngine {
    src: Option<SharedString>,
    status: AudioPlayerStatus,
    current_time: f64,
    duration: f64,
    buffered: f64,
    volume: f32,
    muted: bool,
    playback_rate: f32,
    loop_prop: bool,
    loop_override: Option<bool>,
    auto_play: bool,
    pending_play: bool,
    applied: Applied,
    error: Option<SharedString>,
    callbacks: Callbacks,
    shared: Arc<PlayerShared>,
    gain: Arc<GainParam>,
    destination: Destination,
    voice_alive: Option<Arc<AtomicBool>>,
    analyser: AnalyserTapOptions,
    tap: Option<AnalyserTap>,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
    last_time_update: Option<Instant>,
    load_generation: u64,
    load_task: Option<Task<()>>,
    ticker: Option<Task<()>>,
    window: Option<AnyWindowHandle>,
}

impl AudioPlayerEngine {
    /// A player with the initial volume / muted / rate of `options`, nothing
    /// loaded. [`configure`](Self::configure) loads `src`.
    pub fn new(options: &UseAudioPlayerOptions, analyser: AnalyserTapOptions) -> Self {
        let volume = options.volume.clamp(0.0, 1.0);
        let gain = if options.muted { 0.0 } else { volume };
        Self {
            src: None,
            status: AudioPlayerStatus::Idle,
            current_time: 0.0,
            duration: 0.0,
            buffered: 0.0,
            volume,
            muted: options.muted,
            playback_rate: options.playback_rate,
            loop_prop: options.r#loop,
            loop_override: None,
            auto_play: options.auto_play,
            pending_play: false,
            applied: Applied::default(),
            error: None,
            callbacks: Callbacks::default(),
            shared: Arc::new(PlayerShared {
                buffer: Mutex::new(None),
                playing: AtomicBool::new(false),
                position: AtomicF64::new(0.0),
                seek: AtomicF64::new(f64::NAN),
                rate: AtomicF32::new(options.playback_rate),
                looping: AtomicBool::new(options.r#loop),
                ended: AtomicBool::new(false),
                time: create_frame_emitter(),
            }),
            gain: Arc::new(GainParam::new(gain, VOLUME_RAMP_SECONDS)),
            destination: Destination::Speakers,
            voice_alive: None,
            analyser,
            tap: None,
            meter: create_frame_emitter(),
            visual: create_frame_emitter(),
            last_time_update: None,
            load_generation: 0,
            load_task: None,
            ticker: None,
            window: None,
        }
    }

    /// Applies options like the hook's effects: a new `src` loads (then
    /// auto-plays); volume / muted / rate are written only when they changed
    /// since the last call; `loop` is the default of `set_loop`.
    pub fn configure(&mut self, options: &UseAudioPlayerOptions, cx: &mut Context<Self>) {
        self.callbacks = Callbacks {
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
            self.playback_rate = options.playback_rate;
        }
        self.loop_prop = options.r#loop;
        self.auto_play = options.auto_play;
        self.sync_params();
        if options.src != self.src {
            self.load(options.src.clone(), cx);
        }
    }

    fn looping(&self) -> bool {
        self.loop_override.unwrap_or(self.loop_prop)
    }

    fn sync_params(&self) {
        self.gain
            .set_target(if self.muted { 0.0 } else { self.volume });
        self.shared.rate.store(self.playback_rate);
        self.shared.looping.store(self.looping(), Ordering::Release);
    }

    /// Loads a new source (`element.src = ..; element.load()`).
    pub fn load(&mut self, src: Option<SharedString>, cx: &mut Context<Self>) {
        self.src = src.clone();
        self.load_generation += 1;
        self.shared.playing.store(false, Ordering::Release);
        self.shared.ended.store(false, Ordering::Release);
        *self.shared.buffer.lock() = None;
        self.shared.seek.store(0.0);
        self.shared.position.store(0.0);
        self.shared.time.emit(0.0);
        self.current_time = 0.0;
        self.duration = 0.0;
        self.buffered = 0.0;
        self.pending_play = false;
        self.error = None;
        self.load_task = None;
        let Some(src) = src else {
            self.status = AudioPlayerStatus::Idle;
            return;
        };
        self.status = AudioPlayerStatus::Loading;
        let generation = self.load_generation;
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let path = src.to_string();
            let result = cx
                .background_executor()
                .spawn(async move { AudioBuffer::load(&path) })
                .await;
            let events = this
                .update(cx, |engine, cx| {
                    let events = engine.finish_load(generation, result, cx);
                    cx.notify();
                    events
                })
                .unwrap_or_default();
            dispatch_async(&this, events, cx);
        }));
    }

    /// Plays decoded audio (`src` becomes a label, e.g. `"memory"`).
    pub fn load_buffer(
        &mut self,
        label: impl Into<SharedString>,
        buffer: AudioBuffer,
        cx: &mut Context<Self>,
    ) -> Vec<PlayerEvent> {
        self.load(None, cx);
        self.src = Some(label.into());
        let generation = self.load_generation;
        self.finish_load(generation, Ok(buffer), cx)
    }

    fn finish_load(
        &mut self,
        generation: u64,
        result: super::Result<AudioBuffer>,
        cx: &mut Context<Self>,
    ) -> Vec<PlayerEvent> {
        if generation != self.load_generation {
            return Vec::new();
        }
        let connected = self.ensure_voice();
        let buffer = match (result, connected) {
            (Ok(buffer), Ok(())) => buffer,
            (Err(error), _) | (_, Err(error)) => {
                self.status = AudioPlayerStatus::Error;
                self.error = Some(error.message.into());
                return vec![PlayerEvent::Error];
            }
        };
        // loadedmetadata, progress (fully buffered), canplay.
        self.duration = buffer.duration();
        self.buffered = self.duration;
        *self.shared.buffer.lock() = Some(buffer);
        self.status = AudioPlayerStatus::Ready;
        if self.auto_play || self.pending_play {
            self.pending_play = false;
            return self.play_inner(cx);
        }
        Vec::new()
    }

    /// Connects the voice to the destination (once per destination).
    fn ensure_voice(&mut self) -> super::Result<()> {
        if self.voice_alive.is_some() {
            return Ok(());
        }
        let sample_rate = self.destination.sample_rate();
        let tap = AnalyserTap::with_emitters(
            self.analyser,
            sample_rate,
            self.meter.clone(),
            self.visual.clone(),
        );
        let alive = Arc::new(AtomicBool::new(true));
        let voice = PlayerVoice {
            shared: self.shared.clone(),
            volume: Ramp::new(self.gain.clone()),
            tap: tap.writer(),
            alive: alive.clone(),
            scratch: Vec::new(),
            position: 0.0,
        };
        self.shared.seek.store(self.shared.position.load());
        if let Some(output) = self.destination.connect(Box::new(voice))? {
            // `Destination::None` has no meaning for a player: play it nowhere.
            drop(output);
        }
        self.tap = Some(tap);
        self.voice_alive = Some(alive);
        Ok(())
    }

    /// Routes the player somewhere else, e.g. a mixer channel's input bus.
    pub fn set_destination(&mut self, destination: Destination) -> super::Result<()> {
        if destination == self.destination {
            return Ok(());
        }
        self.destination = destination;
        if let Some(alive) = self.voice_alive.take() {
            alive.store(false, Ordering::Release);
            self.tap = None;
            return self.ensure_voice();
        }
        Ok(())
    }

    fn position(&self) -> f64 {
        let seek = self.shared.seek.load();
        if seek.is_nan() {
            self.shared.position.load()
        } else {
            seek
        }
    }

    fn play_inner(&mut self, cx: &mut Context<Self>) -> Vec<PlayerEvent> {
        if self.src.is_none()
            || self.status == AudioPlayerStatus::Error
            || self.shared.playing.load(Ordering::Acquire)
        {
            return Vec::new();
        }
        if self.status == AudioPlayerStatus::Loading {
            self.pending_play = true;
            return Vec::new();
        }
        if self.status == AudioPlayerStatus::Ended
            || (self.duration > 0.0 && self.position() >= self.duration)
        {
            self.shared.seek.store(0.0);
            self.shared.position.store(0.0);
        }
        self.shared.ended.store(false, Ordering::Release);
        self.shared.playing.store(true, Ordering::Release);
        self.status = AudioPlayerStatus::Playing;
        self.current_time = self.position();
        self.last_time_update = Some(Instant::now());
        self.shared.time.emit(self.current_time);
        self.ensure_ticker(cx);
        vec![PlayerEvent::Play]
    }

    fn pause_inner(&mut self) -> Vec<PlayerEvent> {
        self.pending_play = false;
        if !self.shared.playing.swap(false, Ordering::AcqRel) {
            return Vec::new();
        }
        self.current_time = self.position();
        self.shared.time.emit(self.current_time);
        if self.status != AudioPlayerStatus::Ended {
            self.status = AudioPlayerStatus::Paused;
        }
        vec![PlayerEvent::Pause]
    }

    fn seek_inner(&mut self, seconds: f64) {
        let upper = if self.duration > 0.0 {
            self.duration
        } else {
            seconds
        };
        let target = seconds.max(0.0).min(upper).max(0.0);
        self.shared.seek.store(target);
        self.shared.position.store(target);
        self.current_time = target;
        self.shared.time.emit(target);
    }

    /// Advances the GPUI side: `ended`, `timeupdate`. Returns events and
    /// whether the state changed.
    fn tick(&mut self, now: Instant) -> (Vec<PlayerEvent>, bool) {
        if self.shared.ended.swap(false, Ordering::AcqRel) {
            self.current_time = self.duration;
            self.shared.time.emit(self.duration);
            self.status = AudioPlayerStatus::Ended;
            return (vec![PlayerEvent::Pause, PlayerEvent::Ended], true);
        }
        if !self.shared.playing.load(Ordering::Acquire) {
            return (Vec::new(), false);
        }
        let due = self
            .last_time_update
            .is_none_or(|last| now.saturating_duration_since(last) >= TIME_UPDATE_INTERVAL);
        if due {
            self.current_time = self.position();
            self.last_time_update = Some(now);
        }
        (Vec::new(), due)
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() {
            return;
        }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK_INTERVAL).await;
                let Ok((events, running)) = this.update(cx, |engine, cx| {
                    let (events, changed) = engine.tick(Instant::now());
                    if changed {
                        cx.notify();
                    }
                    let running = engine.shared.playing.load(Ordering::Acquire);
                    if !running {
                        engine.ticker = None;
                    }
                    (events, running)
                }) else {
                    return;
                };
                dispatch_async(&this, events, cx);
                if !running {
                    return;
                }
            }
        }));
    }

    /// The state snapshot (`src ? state : idle`).
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

    pub fn src(&self) -> Option<&SharedString> {
        self.src.as_ref()
    }

    /// The playhead, in seconds, updated from the audio thread while playing.
    pub fn time_source(&self) -> FrameSource<f64> {
        self.shared.time.source()
    }

    /// Meter frames of what the player plays (pre-volume).
    pub fn meter_source(&self) -> FrameSource<MeterFrame> {
        self.meter.source()
    }

    /// Visual frames of what the player plays (pre-volume).
    pub fn visual_source(&self) -> FrameSource<VisualFrame> {
        self.visual.source()
    }

    /// The decoded track, once ready.
    pub fn buffer(&self) -> Option<AudioBuffer> {
        self.shared.buffer.lock().clone()
    }
}

impl Drop for AudioPlayerEngine {
    fn drop(&mut self) {
        if let Some(alive) = &self.voice_alive {
            alive.store(false, Ordering::Release);
        }
    }
}

fn dispatch_events(
    engine: &Entity<AudioPlayerEngine>,
    events: &[PlayerEvent],
    window: &mut Window,
    cx: &mut App,
) {
    if events.is_empty() {
        return;
    }
    let (callbacks, error) = {
        let engine = engine.read(cx);
        (engine.callbacks.clone(), engine.error.clone())
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

fn dispatch_async(
    this: &WeakEntity<AudioPlayerEngine>,
    events: Vec<PlayerEvent>,
    cx: &mut AsyncApp,
) {
    if events.is_empty() {
        return;
    }
    let Some(entity) = this.upgrade() else {
        return;
    };
    let Some(window) = entity.read_with(cx, |engine, _| engine.window) else {
        return;
    };
    cx.update_window(window, |_, window, cx| {
        dispatch_events(&entity, &events, window, cx);
    })
    .ok();
}

fn engine_action(
    engine: &Entity<AudioPlayerEngine>,
    window: &mut Window,
    cx: &mut App,
    action: impl FnOnce(&mut AudioPlayerEngine, &mut Context<AudioPlayerEngine>) -> Vec<PlayerEvent>,
) {
    let handle = window.window_handle();
    let events = engine.update(cx, |engine, cx| {
        engine.window = Some(handle);
        let events = action(engine, cx);
        engine.sync_params();
        cx.notify();
        events
    });
    dispatch_events(engine, &events, window, cx);
}

impl AudioPlayerController for Entity<AudioPlayerEngine> {
    fn state(&self, cx: &App) -> AudioPlayerState {
        self.read(cx).state()
    }

    fn time(&self, cx: &App) -> FrameSource<f64> {
        self.read(cx).time_source()
    }

    fn play(&self, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, cx| engine.play_inner(cx));
    }

    fn pause(&self, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| engine.pause_inner());
    }

    fn seek(&self, seconds: f64, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| {
            engine.seek_inner(seconds);
            Vec::new()
        });
    }

    fn set_volume(&self, volume: f32, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| {
            engine.volume = volume.clamp(0.0, 1.0);
            Vec::new()
        });
    }

    fn set_muted(&self, muted: bool, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| {
            engine.muted = muted;
            Vec::new()
        });
    }

    fn set_playback_rate(&self, rate: f32, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| {
            engine.playback_rate = rate;
            Vec::new()
        });
    }

    fn set_loop(&self, r#loop: bool, window: &mut Window, cx: &mut App) {
        engine_action(self, window, cx, |engine, _| {
            engine.loop_override = Some(r#loop);
            Vec::new()
        });
    }
}

/// `useAudioPlayer(options)` with native playback: keeps an
/// [`AudioPlayerEngine`] in keyed state under `id` and applies `options`
/// every render.
pub fn use_audio_player(
    id: impl Into<ElementId>,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Entity<AudioPlayerEngine> {
    use_audio_player_with(id, options, AnalyserTapOptions::default(), window, cx)
}

/// [`use_audio_player`] with analyser settings for the player's sources
/// (read when the player first connects).
pub fn use_audio_player_with(
    id: impl Into<ElementId>,
    options: UseAudioPlayerOptions,
    analyser: AnalyserTapOptions,
    window: &mut Window,
    cx: &mut App,
) -> Entity<AudioPlayerEngine> {
    let engine = window.use_keyed_state(id, cx, |_, _| AudioPlayerEngine::new(&options, analyser));
    let handle = window.window_handle();
    engine.update(cx, |engine, cx| {
        engine.window = Some(handle);
        engine.configure(&options, cx);
    });
    engine
}

fn native_backend(
    id: ElementId,
    options: UseAudioPlayerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Rc<dyn AudioPlayerController> {
    Rc::new(use_audio_player(id, options, window, cx))
}

/// Makes every `AudioPlayer::new(..).src(..)` without `.player(..)` play
/// through [`AudioPlayerEngine`].
pub fn install_audio_player_backend(cx: &mut App) {
    set_audio_player_backend(native_backend, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::analyser::AnalyserTap;
    use crate::io::gain::GainParam;

    fn voice(buffer: AudioBuffer) -> (PlayerVoice, Arc<PlayerShared>, AnalyserTap) {
        let shared = Arc::new(PlayerShared {
            buffer: Mutex::new(Some(buffer)),
            playing: AtomicBool::new(false),
            position: AtomicF64::new(0.0),
            seek: AtomicF64::new(f64::NAN),
            rate: AtomicF32::new(1.0),
            looping: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            time: create_frame_emitter(),
        });
        let tap = AnalyserTap::new(AnalyserTapOptions::default(), 48_000);
        let voice = PlayerVoice {
            shared: shared.clone(),
            volume: Ramp::new(Arc::new(GainParam::new(0.5, 0.0))),
            tap: tap.writer(),
            alive: Arc::new(AtomicBool::new(true)),
            scratch: Vec::new(),
            position: 0.0,
        };
        (voice, shared, tap)
    }

    #[test]
    fn voice_is_silent_until_played_then_ends() {
        let (mut voice, shared, _tap) = voice(AudioBuffer::mono(vec![1.0; 48_000], 48_000));
        let mut out = vec![0.0f32; 2 * 480];
        voice.render(&mut out, 48_000);
        assert!(out.iter().all(|s| *s == 0.0));

        shared.playing.store(true, Ordering::Release);
        voice.render(&mut out, 48_000);
        assert!(
            out.iter().all(|s| (*s - 0.5).abs() < 1e-6),
            "volume applied"
        );
        assert!((shared.position.load() - 0.01).abs() < 1e-9);
        assert_eq!(shared.time.source().latest(), Some(shared.position.load()));

        shared.seek.store(0.995);
        let mut out = vec![0.0f32; 2 * 480];
        voice.render(&mut out, 48_000);
        assert!(shared.ended.load(Ordering::Acquire));
        assert!(!shared.playing.load(Ordering::Acquire));
        assert!((shared.position.load() - 1.0).abs() < 1e-9);
        assert_eq!(out[2 * 239], 0.5);
        assert_eq!(out[2 * 240], 0.0);
    }

    #[test]
    fn voice_loops_and_follows_rate() {
        let (mut voice, shared, _tap) = voice(AudioBuffer::mono(vec![0.25; 4800], 48_000));
        shared.playing.store(true, Ordering::Release);
        shared.looping.store(true, Ordering::Release);
        shared.rate.store(2.0);
        let mut out = vec![0.0f32; 2 * 4800];
        voice.render(&mut out, 48_000);
        assert!(!shared.ended.load(Ordering::Acquire));
        // Two passes of a 0.1 s buffer at 2× in 0.1 s: back at the start.
        assert!(shared.position.load().abs() < 1e-9 || (shared.position.load() - 0.1).abs() < 1e-9);
        assert!(out.iter().all(|s| (*s - 0.125).abs() < 1e-6));
    }
}
