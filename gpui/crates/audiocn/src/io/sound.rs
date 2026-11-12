//! Port of `hooks/use-sound.ts`: low-latency playback of a short sound,
//! decoded into memory, for sound pads (one-shot, loop, hold) and UI clicks.
//!
//! A [`Sound`] loads its `src` on a background thread, plays voices through
//! its own gain node (`volume`, ramped over 5 ms) to its destination, and
//! reports the latest voice's progress (0..1) on a [`FrameSource`] from the
//! audio thread. `interrupt` restarts instead of layering; otherwise up to
//! `max_voices` play at once and the oldest is cut.
//!
//! ```ignore
//! let sound = Sound::new("/sounds/peon-work-work.wav", SoundOptions::default());
//! SoundPad::new("peon").playing(sound.is_playing()).on_trigger(move |_, _| sound.play())
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use gpui::{App, ElementId, Window};
use parking_lot::Mutex;

use super::buffer::AudioBuffer;
use super::context::{AtomicF64, AudioOutput, AudioSource, Bus, Destination};
use super::gain::{GainNode, GainNodeOptions};
use super::{AudioIoError, AudioIoErrorKind, Result};
use crate::audio::click::{ClickPlayOptions, ClickSound, ClickVoice};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};

/// `RAMP_SECONDS`: the volume ramp's time constant.
const RAMP_SECONDS: f32 = 0.005;

/// `UseSoundOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundOptions {
    /// 0..1 gain. Default 1.
    pub volume: f32,
    /// Default 1.
    pub playback_rate: f32,
    pub r#loop: bool,
    /// Restart instead of layering when played again. Default true.
    pub interrupt: bool,
    /// Most simultaneous voices when not interrupting. Default 4.
    pub max_voices: usize,
    /// Where the sound goes. Default the speakers.
    pub destination: Destination,
}

impl Default for SoundOptions {
    fn default() -> Self {
        Self {
            volume: 1.0,
            playback_rate: 1.0,
            r#loop: false,
            interrupt: true,
            max_voices: 4,
            destination: Destination::Speakers,
        }
    }
}

// ---------------------------------------------------------------------------
// Buffer voices
// ---------------------------------------------------------------------------

/// What the owner of a voice can see and control.
#[derive(Default)]
pub(crate) struct VoiceState {
    stopped: AtomicBool,
    ended: AtomicBool,
    /// Report progress (only the newest voice does).
    report: AtomicBool,
    /// Playhead in buffer frames.
    position: AtomicF64,
}

/// An `AudioBufferSourceNode`: plays a buffer once or looped, resampled to
/// the output rate (linear interpolation) at `playback_rate`.
pub(crate) struct BufferVoice {
    buffer: AudioBuffer,
    playback_rate: f32,
    looping: bool,
    gain: f32,
    position: f64,
    delay_frames: u64,
    state: Arc<VoiceState>,
    progress: Option<FrameEmitter<f32>>,
    on_end: Option<Box<dyn FnOnce() + Send>>,
}

impl BufferVoice {
    pub(crate) fn new(buffer: AudioBuffer, playback_rate: f32, looping: bool) -> Self {
        Self {
            buffer,
            playback_rate,
            looping,
            gain: 1.0,
            position: 0.0,
            delay_frames: 0,
            state: Arc::new(VoiceState::default()),
            progress: None,
            on_end: None,
        }
    }

    fn finish(&mut self) -> bool {
        self.state.ended.store(true, Ordering::Release);
        if let Some(on_end) = self.on_end.take() {
            on_end();
        }
        false
    }

    /// Progress of a voice at `position` frames (`useSound`'s formula).
    fn progress_at(position: f64, length: usize, looping: bool) -> f32 {
        if length == 0 {
            return 0.0;
        }
        let length = length as f64;
        if looping {
            ((position % length) / length) as f32
        } else {
            (position / length).min(1.0) as f32
        }
    }
}

impl AudioSource for BufferVoice {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        if self.state.stopped.load(Ordering::Acquire) {
            return self.finish();
        }
        let length = self.buffer.length();
        if length == 0 {
            return self.finish();
        }
        let step = self.playback_rate.max(0.0) as f64 * self.buffer.sample_rate() as f64
            / sample_rate.max(1) as f64;
        let mut ended = false;
        for pair in out.as_chunks_mut::<2>().0.iter_mut() {
            if self.delay_frames > 0 {
                self.delay_frames -= 1;
                continue;
            }
            if self.position >= length as f64 {
                if self.looping {
                    self.position %= length as f64;
                } else {
                    ended = true;
                    break;
                }
            }
            let index = self.position as usize;
            let fraction = (self.position - index as f64) as f32;
            let next_index = if index + 1 >= length && self.looping {
                0
            } else {
                index + 1
            };
            let a = self.buffer.frame(index);
            let b = self.buffer.frame(next_index);
            pair[0] += (a[0] + (b[0] - a[0]) * fraction) * self.gain;
            pair[1] += (a[1] + (b[1] - a[1]) * fraction) * self.gain;
            self.position += step;
        }
        self.state.position.store(self.position);
        if let Some(progress) = &self.progress
            && self.state.report.load(Ordering::Acquire)
        {
            progress.emit(Self::progress_at(self.position, length, self.looping));
        }
        if ended { self.finish() } else { true }
    }

    fn channel_count(&self) -> usize {
        self.buffer.number_of_channels().clamp(1, 2)
    }
}

// ---------------------------------------------------------------------------
// Sound
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum LoadState {
    Empty,
    Loading,
    Loaded(AudioBuffer),
    Failed(AudioIoError),
}

struct SoundShared {
    progress: FrameEmitter<f32>,
    voices: Mutex<Vec<Arc<VoiceState>>>,
}

impl SoundShared {
    /// A voice ended: forget it; the last one resets progress (`ended`).
    fn voice_ended(&self) {
        let mut voices = self.voices.lock();
        voices.retain(|voice| !voice.ended.load(Ordering::Acquire));
        if voices.is_empty() {
            self.progress.emit(0.0);
        }
    }
}

struct SoundInner {
    src: Option<String>,
    load: Mutex<LoadState>,
    options: Mutex<SoundOptions>,
    output: Mutex<Option<GainNode>>,
    output_error: Mutex<Option<AudioIoError>>,
    shared: Arc<SoundShared>,
}

/// A sound (`useSound`'s controller). Clones share it; voices stop when the
/// last clone drops.
#[derive(Clone)]
pub struct Sound {
    inner: Arc<SoundInner>,
}

impl PartialEq for Sound {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Sound {
    /// Loads `src` (path or site path, see [`AudioBuffer::load`]) in the
    /// background; decoded buffers are cached per `src`.
    pub fn new(src: impl Into<String>, options: SoundOptions) -> Sound {
        let src = src.into();
        let sound = Self::create(Some(src.clone()), LoadState::Loading, options);
        let weak = Arc::downgrade(&sound.inner);
        let spawned = thread::Builder::new()
            .name("audiocn-sound-load".into())
            .spawn(move || {
                let result = AudioBuffer::load_cached(&src);
                if let Some(inner) = weak.upgrade() {
                    *inner.load.lock() = match result {
                        Ok(buffer) => LoadState::Loaded(buffer),
                        Err(error) => LoadState::Failed(error),
                    };
                }
            });
        if let Err(error) = spawned {
            *sound.inner.load.lock() = LoadState::Failed(AudioIoError::new(
                AudioIoErrorKind::Other,
                error.to_string(),
            ));
        }
        sound
    }

    /// A sound from a decoded buffer (`useSound(audioBuffer)`).
    pub fn from_buffer(buffer: AudioBuffer, options: SoundOptions) -> Sound {
        Self::create(None, LoadState::Loaded(buffer), options)
    }

    /// No sound (`useSound(null)`): never loads, never plays.
    pub fn empty() -> Sound {
        Self::create(None, LoadState::Empty, SoundOptions::default())
    }

    fn create(src: Option<String>, load: LoadState, options: SoundOptions) -> Sound {
        let connect = !matches!(load, LoadState::Empty);
        let sound = Sound {
            inner: Arc::new(SoundInner {
                src,
                load: Mutex::new(load),
                options: Mutex::new(options.clone()),
                output: Mutex::new(None),
                output_error: Mutex::new(None),
                shared: Arc::new(SoundShared {
                    progress: create_frame_emitter(),
                    voices: Mutex::new(Vec::new()),
                }),
            }),
        };
        if connect {
            sound.connect(&options);
        }
        sound
    }

    fn connect(&self, options: &SoundOptions) {
        let node = GainNode::new(GainNodeOptions {
            input: None,
            gain: options.volume,
            destination: options.destination.clone(),
            time_constant: RAMP_SECONDS,
        });
        match node {
            Ok(node) => {
                *self.inner.output.lock() = Some(node);
                *self.inner.output_error.lock() = None;
            }
            Err(error) => *self.inner.output_error.lock() = Some(error),
        }
    }

    /// The `src` this sound loads.
    pub fn src(&self) -> Option<&str> {
        self.inner.src.as_deref()
    }

    /// Applies new options: volume ramps; rate and loop apply to the next
    /// voice (a voice keeps the loop and rate it started with); a new
    /// destination reconnects.
    pub fn set_options(&self, options: SoundOptions) {
        let previous = std::mem::replace(&mut *self.inner.options.lock(), options.clone());
        if previous.destination != options.destination {
            self.stop();
            self.connect(&options);
        } else if let Some(node) = self.inner.output.lock().as_ref() {
            node.set_gain(options.volume);
        }
    }

    pub fn options(&self) -> SoundOptions {
        self.inner.options.lock().clone()
    }

    pub fn set_volume(&self, volume: f32) {
        let mut options = self.options();
        options.volume = volume;
        self.set_options(options);
    }

    /// Plays from the start (no-op until loaded).
    pub fn play(&self) {
        let LoadState::Loaded(buffer) = self.inner.load.lock().clone() else {
            return;
        };
        let options = self.options();
        let output = self.inner.output.lock();
        let Some(output) = output.as_ref() else {
            return;
        };
        if options.interrupt {
            self.stop_voices();
        } else {
            let mut voices = self.inner.shared.voices.lock();
            voices.retain(|voice| !voice.ended.load(Ordering::Acquire));
            while voices.len() >= options.max_voices.max(1) {
                let oldest = voices.remove(0);
                oldest.stopped.store(true, Ordering::Release);
            }
        }
        let mut voice = BufferVoice::new(buffer, options.playback_rate, options.r#loop);
        voice.progress = Some(self.inner.shared.progress.clone());
        let shared = Arc::downgrade(&self.inner.shared);
        voice.on_end = Some(Box::new(move || {
            if let Some(shared) = shared.upgrade() {
                shared.voice_ended();
            }
        }));
        let state = voice.state.clone();
        state.report.store(true, Ordering::Release);
        {
            let mut voices = self.inner.shared.voices.lock();
            for voice in voices.iter() {
                voice.report.store(false, Ordering::Release);
            }
            voices.push(state);
        }
        output.input().add(voice);
    }

    fn stop_voices(&self) {
        let mut voices = self.inner.shared.voices.lock();
        for voice in voices.drain(..) {
            voice.stopped.store(true, Ordering::Release);
        }
    }

    /// Stops every voice.
    pub fn stop(&self) {
        self.stop_voices();
        self.inner.shared.progress.emit(0.0);
    }

    pub fn is_playing(&self) -> bool {
        self.inner
            .shared
            .voices
            .lock()
            .iter()
            .any(|voice| !voice.ended.load(Ordering::Acquire))
    }

    pub fn is_loaded(&self) -> bool {
        matches!(*self.inner.load.lock(), LoadState::Loaded(_))
    }

    /// Still decoding.
    pub fn is_loading(&self) -> bool {
        matches!(*self.inner.load.lock(), LoadState::Loading)
    }

    /// A load or output error.
    pub fn error(&self) -> Option<AudioIoError> {
        match &*self.inner.load.lock() {
            LoadState::Failed(error) => Some(error.clone()),
            _ => self.inner.output_error.lock().clone(),
        }
    }

    /// Seconds (0 until loaded).
    pub fn duration(&self) -> f64 {
        match &*self.inner.load.lock() {
            LoadState::Loaded(buffer) => buffer.duration(),
            _ => 0.0,
        }
    }

    /// The decoded buffer, once loaded.
    pub fn buffer(&self) -> Option<AudioBuffer> {
        match &*self.inner.load.lock() {
            LoadState::Loaded(buffer) => Some(buffer.clone()),
            _ => None,
        }
    }

    /// Playback progress, 0..1, while playing.
    pub fn progress(&self) -> FrameSource<f32> {
        self.inner.shared.progress.source()
    }

    /// The sound's output node input (`output`), e.g. to add more sources.
    pub fn output(&self) -> Option<Bus> {
        self.inner.output.lock().as_ref().map(GainNode::input)
    }

    /// With `Destination::None`: the sound's output, once, to route yourself.
    pub fn take_output(&self) -> Option<Box<dyn AudioSource>> {
        self.inner.output.lock().as_ref()?.take_output()
    }
}

impl Drop for SoundInner {
    fn drop(&mut self) {
        for voice in self.shared.voices.lock().drain(..) {
            voice.stopped.store(true, Ordering::Release);
        }
    }
}

struct UseSoundState {
    sound: Sound,
}

/// `useSound(src, options)`: keeps a [`Sound`] in keyed state under `id`
/// (reloaded when `src` changes, options applied every render) and
/// re-renders while it loads or plays so `is_playing` stays current.
pub fn use_sound(
    id: impl Into<ElementId>,
    src: Option<&str>,
    options: SoundOptions,
    window: &mut Window,
    cx: &mut App,
) -> Sound {
    let make = |src: Option<&str>, options: SoundOptions| match src {
        Some(src) => Sound::new(src, options),
        None => Sound::empty(),
    };
    let state = window.use_keyed_state(id, cx, |_, _| UseSoundState {
        sound: make(src, options.clone()),
    });
    let sound = state.read(cx).sound.clone();
    let sound = if sound.src() != src {
        sound.stop();
        let sound = make(src, options);
        state.update(cx, |state, _| state.sound = sound.clone());
        sound
    } else {
        if sound.options() != options {
            sound.set_options(options);
        }
        sound
    };
    if sound.is_loading() || sound.is_playing() {
        window.request_animation_frame();
    }
    sound
}

// ---------------------------------------------------------------------------
// Clicks
// ---------------------------------------------------------------------------

/// Plays a [`ClickVoice`] on the shared output. `when` is on
/// [`AudioOutput::current_time`]'s clock.
pub fn play_click_voice(voice: ClickVoice) -> Result<()> {
    let output = AudioOutput::shared()?;
    let mut source = BufferVoice::new(
        AudioBuffer::mono(voice.samples.to_vec(), voice.sample_rate),
        voice.playback_rate,
        false,
    );
    source.gain = voice.gain;
    if let Some(when) = voice.when {
        let delay = (when - output.current_time()) * output.sample_rate() as f64;
        source.delay_frames = delay.max(0.0) as u64;
    }
    output.play(source);
    Ok(())
}

/// Plays a synthesised click (the knob's tick) on the shared output.
/// Returns false when `min_interval_ms` held it back.
pub fn play_click(sound: &ClickSound, options: ClickPlayOptions) -> Result<bool> {
    let output = AudioOutput::shared()?;
    match sound.play(options, output.sample_rate()) {
        Some(voice) => play_click_voice(voice).map(|()| true),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(voice: &mut BufferVoice, frames: usize, rate: u32) -> (Vec<f32>, bool) {
        let mut out = vec![0.0f32; frames * 2];
        let alive = voice.render(&mut out, rate);
        (out, alive)
    }

    #[test]
    fn plays_a_buffer_once_and_ends() {
        let buffer = AudioBuffer::mono(vec![0.5; 100], 48_000);
        let mut voice = BufferVoice::new(buffer, 1.0, false);
        let (out, alive) = render(&mut voice, 64, 48_000);
        assert!(alive);
        assert!(out.iter().all(|s| *s == 0.5));
        let (out, alive) = render(&mut voice, 64, 48_000);
        assert!(!alive);
        assert_eq!(out[2 * 35], 0.5);
        assert_eq!(out[2 * 36], 0.0);
        assert!(voice.state.ended.load(Ordering::Acquire));
    }

    #[test]
    fn loops_and_resamples() {
        let buffer = AudioBuffer::mono(vec![1.0; 10], 24_000);
        let mut voice = BufferVoice::new(buffer, 1.0, true);
        // 24 kHz at 48 kHz: half a frame per output frame.
        let (_, alive) = render(&mut voice, 30, 48_000);
        assert!(alive);
        // 15 frames into a 10-frame loop.
        assert!((voice.state.position.load() - 5.0).abs() < 1e-9);
        assert!((BufferVoice::progress_at(15.0, 10, true) - 0.5).abs() < 1e-6);
        assert_eq!(BufferVoice::progress_at(15.0, 10, false), 1.0);
    }

    #[test]
    fn delays_scheduled_voices() {
        let buffer = AudioBuffer::mono(vec![1.0; 10], 48_000);
        let mut voice = BufferVoice::new(buffer, 1.0, false);
        voice.delay_frames = 4;
        let (out, _) = render(&mut voice, 8, 48_000);
        assert_eq!(&out[..8], &[0.0; 8]);
        assert_eq!(out[8], 1.0);
    }

    #[test]
    fn interrupt_and_max_voices() {
        let bus = Bus::new();
        let buffer = AudioBuffer::mono(vec![0.25; 48_000], 48_000);
        let sound = Sound::from_buffer(
            buffer.clone(),
            SoundOptions {
                destination: Destination::Bus(bus.clone()),
                ..Default::default()
            },
        );
        sound.play();
        sound.play();
        let mut out = vec![0.0f32; 64];
        bus.render(&mut out, 48_000);
        // Interrupting: only one voice is heard.
        assert!((out[63] - 0.25).abs() < 1e-6, "{}", out[63]);
        assert!(sound.is_playing());
        sound.stop();
        bus.render(&mut out, 48_000);
        assert!(!sound.is_playing());
        assert_eq!(sound.progress().latest(), Some(0.0));

        let layered = Sound::from_buffer(
            buffer,
            SoundOptions {
                interrupt: false,
                max_voices: 2,
                destination: Destination::Bus(bus.clone()),
                ..Default::default()
            },
        );
        for _ in 0..3 {
            layered.play();
        }
        let mut out = vec![0.0f32; 64];
        bus.render(&mut out, 48_000);
        assert!((out[63] - 0.5).abs() < 1e-5, "{}", out[63]);
    }

    #[test]
    fn empty_sound_never_plays() {
        let sound = Sound::empty();
        sound.play();
        assert!(!sound.is_playing() && !sound.is_loaded() && sound.duration() == 0.0);
    }
}
