//! The native `AudioContext` (`hooks/use-audio-context.tsx`): one output
//! stream per device, a graph of [`Bus`]es that [`AudioSource`]s render into,
//! and the plumbing every other `io` module shares (stream threads, sample
//! rings between devices, lock-free parameters).
//!
//! The engine renders **stereo, interleaved `f32`** at the output device's
//! sample rate; the output callback maps that onto the device's channels.
//!
//! ```ignore
//! let output = AudioOutput::shared()?;           // getSharedAudioContext()
//! output.destination().add(my_source);           // node.connect(context.destination)
//! ```

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use cpal::traits::{DeviceTrait, StreamTrait};
use parking_lot::Mutex;

use super::microphone::MicrophoneHandle;
use super::{AudioIoError, AudioIoErrorKind, Result};

/// Frames per device callback we ask for (~21 ms at 48 kHz). The server
/// default on PulseAudio is up to two seconds, far too slow for meters.
pub(crate) const STREAM_BUFFER_FRAMES: u32 = 1024;

// ---------------------------------------------------------------------------
// Small lock-free values shared with audio threads
// ---------------------------------------------------------------------------

/// An `f32` readable and writable from any thread.
#[derive(Debug, Default)]
pub(crate) struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub(crate) fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    pub(crate) fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Acquire))
    }

    pub(crate) fn store(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Release);
    }
}

/// An `f64` readable and writable from any thread.
#[derive(Debug, Default)]
pub(crate) struct AtomicF64(AtomicU64);

impl AtomicF64 {
    pub(crate) fn new(value: f64) -> Self {
        Self(AtomicU64::new(value.to_bits()))
    }

    pub(crate) fn load(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Acquire))
    }

    pub(crate) fn store(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::Release);
    }

    pub(crate) fn swap(&self, value: f64) -> f64 {
        f64::from_bits(self.0.swap(value.to_bits(), Ordering::AcqRel))
    }
}

// ---------------------------------------------------------------------------
// Sources and buses
// ---------------------------------------------------------------------------

/// Something that makes sound: an `AudioNode` with no inputs, from the graph's
/// point of view. Runs on the audio thread: never block, never touch GPUI.
pub trait AudioSource: Send + 'static {
    /// Adds `out.len() / 2` stereo frames (interleaved L, R) into `out` at
    /// `sample_rate`. Add, don't overwrite: other sources share `out`.
    /// Return `false` once finished; the bus then drops the source.
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool;

    /// 1 when the signal is mono (both sides equal), so panners use the mono
    /// law like a `StereoPannerNode` fed one channel. Default 2.
    fn channel_count(&self) -> usize {
        2
    }
}

impl AudioSource for Box<dyn AudioSource> {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        (**self).render(out, sample_rate)
    }

    fn channel_count(&self) -> usize {
        (**self).channel_count()
    }
}

/// An [`AudioSource`] from a closure `(out, sample_rate) -> keep_going`.
pub struct FnSource<F>(pub F);

impl<F: FnMut(&mut [f32], u32) -> bool + Send + 'static> AudioSource for FnSource<F> {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        (self.0)(out, sample_rate)
    }
}

/// A summing point: sources added to it play together. The output's
/// [`destination`](AudioOutput::destination) is a bus; so is a
/// [`GainNode`](super::gain::GainNode)'s input and a mixer channel's input.
/// Clones share the same bus. Never add a bus to itself.
#[derive(Clone, Default)]
pub struct Bus {
    sources: Arc<Mutex<Vec<Box<dyn AudioSource>>>>,
}

impl PartialEq for Bus {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.sources, &other.sources)
    }
}

impl std::fmt::Debug for Bus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bus").field("sources", &self.len()).finish()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Connects a source (`source.connect(bus)`).
    pub fn add(&self, source: impl AudioSource) {
        self.sources.lock().push(Box::new(source));
    }

    pub fn add_boxed(&self, source: Box<dyn AudioSource>) {
        self.sources.lock().push(source);
    }

    /// Disconnects every source.
    pub fn clear(&self) {
        self.sources.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.sources.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Renders every source into `out` (adding) and drops finished ones.
    pub fn render(&self, out: &mut [f32], sample_rate: u32) {
        self.sources
            .lock()
            .retain_mut(|source| source.render(out, sample_rate));
    }

    /// This bus as a source, to connect it into another bus. Render it from
    /// one place only: every render advances its sources.
    pub fn source(&self) -> BusSource {
        BusSource { bus: self.clone() }
    }
}

/// A [`Bus`] played as a source (see [`Bus::source`]).
pub struct BusSource {
    bus: Bus,
}

impl AudioSource for BusSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        self.bus.render(out, sample_rate);
        true
    }
}

/// Where a node plays, like the hooks' `destination` option.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Destination {
    /// The shared output (`context.destination`). Default.
    #[default]
    Speakers,
    /// Nowhere: route the node's output yourself (`destination: null`).
    None,
    /// Into a bus, e.g. a mixer channel input or a gain node.
    Bus(Bus),
}

impl Destination {
    /// Connects `source`. Gives it back for [`Destination::None`].
    pub(crate) fn connect(
        &self,
        source: Box<dyn AudioSource>,
    ) -> Result<Option<Box<dyn AudioSource>>> {
        match self {
            Destination::Speakers => {
                AudioOutput::shared()?.destination().add_boxed(source);
                Ok(None)
            }
            Destination::None => Ok(Some(source)),
            Destination::Bus(bus) => {
                bus.add_boxed(source);
                Ok(None)
            }
        }
    }

    /// The sample rate the destination renders at: the shared output's
    /// (opened for `Speakers`; for a bus only when already open, else 48 kHz).
    pub(crate) fn sample_rate(&self) -> u32 {
        let output = match self {
            Destination::Speakers => AudioOutput::shared().ok(),
            Destination::None | Destination::Bus(_) => AudioOutput::shared_if_open(),
        };
        output.map_or(DEFAULT_SAMPLE_RATE, |output| output.sample_rate())
    }
}

/// Used when no output device can be opened.
pub(crate) const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// What feeds a node: the `input` of `useGainNode` / `useWebAudioMixer`.
pub enum AudioInput {
    /// A live microphone (`MediaStream`).
    Microphone(MicrophoneHandle),
    /// Everything connected to a bus (e.g. a player routed with
    /// `Destination::Bus`).
    Bus(Bus),
    /// Any source.
    Source(Box<dyn AudioSource>),
}

impl AudioInput {
    pub fn source(source: impl AudioSource) -> Self {
        Self::Source(Box::new(source))
    }

    pub(crate) fn into_source(self) -> Box<dyn AudioSource> {
        match self {
            AudioInput::Microphone(microphone) => Box::new(microphone.source()),
            AudioInput::Bus(bus) => Box::new(bus.source()),
            AudioInput::Source(source) => source,
        }
    }
}

// ---------------------------------------------------------------------------
// Stream threads
// ---------------------------------------------------------------------------

/// Keeps a `cpal::Stream` (which is not `Send`) alive on its own thread until
/// dropped, so handles that own streams are `Send + Sync`.
pub(crate) struct StreamThread {
    _stop: mpsc::Sender<()>,
}

impl StreamThread {
    /// Builds and starts a stream on a new thread; `on_ready` gets the result.
    pub(crate) fn spawn<R: Send + 'static>(
        name: &str,
        build: impl FnOnce() -> Result<(cpal::Stream, R)> + Send + 'static,
        on_ready: impl FnOnce(Result<R>) + Send + 'static,
    ) -> StreamThread {
        let (stop, stopped) = mpsc::channel::<()>();
        let spawned = thread::Builder::new()
            .name(name.to_string())
            .spawn(move || match build() {
                Ok((stream, value)) => {
                    if let Err(error) = stream.play() {
                        on_ready(Err(AudioIoError::new(
                            AudioIoErrorKind::Other,
                            error.to_string(),
                        )));
                        return;
                    }
                    on_ready(Ok(value));
                    // Returns once the owner drops its sender.
                    let _ = stopped.recv();
                    drop(stream);
                }
                Err(error) => on_ready(Err(error)),
            });
        if let Err(error) = spawned {
            log_error("audiocn: could not spawn an audio thread", &error);
        }
        StreamThread { _stop: stop }
    }

    /// [`spawn`](Self::spawn), waiting for the stream to start.
    pub(crate) fn spawn_blocking<R: Send + 'static>(
        name: &str,
        build: impl FnOnce() -> Result<(cpal::Stream, R)> + Send + 'static,
    ) -> Result<(StreamThread, R)> {
        let (sender, receiver) = mpsc::channel();
        let thread = Self::spawn(name, build, move |result| {
            let _ = sender.send(result);
        });
        let value = receiver.recv().map_err(|_| {
            AudioIoError::new(AudioIoErrorKind::Other, "the audio thread stopped")
        })??;
        Ok((thread, value))
    }
}

fn log_error(context: &str, error: &dyn std::fmt::Display) {
    eprintln!("{context}: {error}");
}

/// `supported` as `f32` when the device offers that at the same rate and
/// channel count (sound servers convert anything), so no conversion runs on
/// our side.
pub(crate) fn prefer_f32(
    supported: cpal::SupportedStreamConfig,
    ranges: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
) -> cpal::SupportedStreamConfig {
    if supported.sample_format() == cpal::SampleFormat::F32 {
        return supported;
    }
    let rate = supported.sample_rate();
    ranges
        .filter(|range| {
            range.sample_format() == cpal::SampleFormat::F32
                && range.channels() == supported.channels()
                && range.min_sample_rate() <= rate
                && rate <= range.max_sample_rate()
        })
        .map(|range| range.with_sample_rate(rate))
        .next()
        .unwrap_or(supported)
}

/// The buffer size to ask a device for: [`STREAM_BUFFER_FRAMES`] within its range.
pub(crate) fn stream_config(supported: &cpal::SupportedStreamConfig) -> cpal::StreamConfig {
    let mut config = supported.config();
    config.buffer_size = match supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            cpal::BufferSize::Fixed(STREAM_BUFFER_FRAMES.clamp(*min, (*max).max(*min)))
        }
        cpal::SupportedBufferSize::Unknown => cpal::BufferSize::Fixed(STREAM_BUFFER_FRAMES),
    };
    config
}

/// Runs `$body` with `$T` bound to the Rust type of a `cpal::SampleFormat`.
macro_rules! with_sample_type {
    ($format:expr, $T:ident => $body:expr) => {
        match $format {
            cpal::SampleFormat::F32 => {
                type $T = f32;
                $body
            }
            cpal::SampleFormat::F64 => {
                type $T = f64;
                $body
            }
            cpal::SampleFormat::I16 => {
                type $T = i16;
                $body
            }
            cpal::SampleFormat::I32 => {
                type $T = i32;
                $body
            }
            cpal::SampleFormat::I24 => {
                type $T = cpal::I24;
                $body
            }
            cpal::SampleFormat::I8 => {
                type $T = i8;
                $body
            }
            cpal::SampleFormat::U8 => {
                type $T = u8;
                $body
            }
            cpal::SampleFormat::U16 => {
                type $T = u16;
                $body
            }
            other => Err($crate::io::AudioIoError::new(
                $crate::io::AudioIoErrorKind::Unsupported,
                format!("unsupported sample format {other}"),
            )),
        }
    };
}
pub(crate) use with_sample_type;

// ---------------------------------------------------------------------------
// The output (AudioContext)
// ---------------------------------------------------------------------------

/// `AudioContextStatus`: `AudioContextState | "unsupported"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioContextStatus {
    #[default]
    Running,
    /// Not used natively (no autoplay policy); kept for parity.
    Suspended,
    /// The device went away. [`AudioOutput::shared`] opens a new one.
    Closed,
    /// No output device.
    Unsupported,
}

struct OutputInner {
    bus: Bus,
    sample_rate: u32,
    channels: u16,
    device_name: String,
    frames: Arc<AtomicU64>,
    failed: Arc<AtomicBool>,
    _thread: StreamThread,
}

/// An open output device: the native `AudioContext`. Clones share it; the
/// stream stops when the last clone drops.
#[derive(Clone)]
pub struct AudioOutput {
    inner: Arc<OutputInner>,
}

static SHARED_OUTPUT: Mutex<Option<AudioOutput>> = Mutex::new(None);

impl AudioOutput {
    /// The app-wide output on the default device, opened on first use
    /// (`getSharedAudioContext`). Reopens after the device went away.
    pub fn shared() -> Result<AudioOutput> {
        let mut shared = SHARED_OUTPUT.lock();
        if let Some(output) = shared.as_ref()
            && output.status() == AudioContextStatus::Running
        {
            return Ok(output.clone());
        }
        let output = AudioOutput::open(None)?;
        *shared = Some(output.clone());
        Ok(output)
    }

    /// The shared output if it is already open (never opens a device).
    pub fn shared_if_open() -> Option<AudioOutput> {
        SHARED_OUTPUT
            .lock()
            .as_ref()
            .filter(|output| output.status() == AudioContextStatus::Running)
            .cloned()
    }

    /// Opens an output device by [`AudioDevice::id`](crate::AudioDevice)
    /// (`None`: the system default).
    pub fn open(device_id: Option<&str>) -> Result<AudioOutput> {
        let device = super::devices::find_device(crate::AudioDeviceKind::AudioOutput, device_id)?;
        let device_name = super::devices::device_label(&device);
        let supported = device.default_output_config()?;
        let supported = match device.supported_output_configs() {
            Ok(ranges) => prefer_f32(supported, ranges),
            Err(_) => supported,
        };
        let config = stream_config(&supported);
        let format = supported.sample_format();
        let bus = Bus::new();
        let frames = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let (thread, ()) = {
            let bus = bus.clone();
            let frames = frames.clone();
            let failed = failed.clone();
            StreamThread::spawn_blocking("audiocn-output", move || {
                let stream = with_sample_type!(format, T => build_output::<T>(
                    &device, config, bus, frames, failed
                ))?;
                Ok((stream, ()))
            })?
        };
        Ok(AudioOutput {
            inner: Arc::new(OutputInner {
                bus,
                sample_rate: config.sample_rate,
                channels: config.channels,
                device_name,
                frames,
                failed,
                _thread: thread,
            }),
        })
    }

    /// Where sources play (`context.destination`).
    pub fn destination(&self) -> Bus {
        self.inner.bus.clone()
    }

    /// Connects a source to the speakers.
    pub fn play(&self, source: impl AudioSource) {
        self.inner.bus.add(source);
    }

    pub fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }

    /// The device's channel count (the engine renders stereo and maps it).
    pub fn channels(&self) -> u16 {
        self.inner.channels
    }

    /// The device's display name.
    pub fn device_name(&self) -> &str {
        &self.inner.device_name
    }

    /// Seconds rendered so far (`context.currentTime`).
    pub fn current_time(&self) -> f64 {
        self.inner.frames.load(Ordering::Relaxed) as f64 / self.inner.sample_rate as f64
    }

    pub fn status(&self) -> AudioContextStatus {
        if self.inner.failed.load(Ordering::Acquire) {
            AudioContextStatus::Closed
        } else {
            AudioContextStatus::Running
        }
    }
}

impl std::fmt::Debug for AudioOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioOutput")
            .field("device", &self.inner.device_name)
            .field("sample_rate", &self.inner.sample_rate)
            .field("channels", &self.inner.channels)
            .finish()
    }
}

/// The status of the shared output without opening it: `useAudioContext().status`.
pub fn shared_output_status() -> AudioContextStatus {
    match SHARED_OUTPUT.lock().as_ref() {
        Some(output) => output.status(),
        None => AudioContextStatus::Suspended,
    }
}

fn build_output<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    bus: Bus,
    frames: Arc<AtomicU64>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels.max(1));
    let sample_rate = config.sample_rate;
    let mut mix = vec![0.0f32; STREAM_BUFFER_FRAMES as usize * 4];
    let stream = device.build_output_stream::<T, _, _>(
        config,
        move |data: &mut [T], _| {
            let count = data.len() / channels;
            if mix.len() < count * 2 {
                mix.resize(count * 2, 0.0);
            }
            let block = &mut mix[..count * 2];
            block.fill(0.0);
            bus.render(block, sample_rate);
            write_device_frames(data, channels, block);
            frames.fetch_add(count as u64, Ordering::Relaxed);
        },
        move |error| {
            if matches!(
                error.kind(),
                cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated
            ) {
                failed.store(true, Ordering::Release);
            }
        },
        None,
    )?;
    Ok(stream)
}

/// Maps stereo frames onto a device buffer with `channels` channels.
fn write_device_frames<T>(data: &mut [T], channels: usize, stereo: &[f32])
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    for (frame, pair) in data
        .chunks_mut(channels)
        .zip(stereo.as_chunks::<2>().0.iter())
    {
        let left = pair[0].clamp(-1.0, 1.0);
        let right = pair[1].clamp(-1.0, 1.0);
        if channels == 1 {
            frame[0] = <T as cpal::Sample>::from_sample((left + right) * 0.5);
            continue;
        }
        frame[0] = <T as cpal::Sample>::from_sample(left);
        frame[1] = <T as cpal::Sample>::from_sample(right);
        for sample in &mut frame[2..] {
            *sample = T::EQUILIBRIUM;
        }
    }
}

// ---------------------------------------------------------------------------
// Sample rings: moving audio between two devices' clocks
// ---------------------------------------------------------------------------

/// Stereo frames from one stream (a microphone) waiting for another (the
/// output). Bounded: the writer drops the oldest frames when it is full.
pub(crate) struct SampleRing {
    frames: Mutex<std::collections::VecDeque<[f32; 2]>>,
    capacity: usize,
    sample_rate: AtomicU32,
}

impl SampleRing {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            frames: Mutex::new(std::collections::VecDeque::with_capacity(8192)),
            capacity: (sample_rate as usize).max(8192),
            sample_rate: AtomicU32::new(sample_rate),
        }
    }

    /// Writes interleaved samples with `channels` channels (mono is
    /// duplicated to both sides, extra channels are dropped).
    pub(crate) fn push_interleaved(&self, samples: &[f32], channels: usize, gain: f32) {
        let channels = channels.max(1);
        let mut frames = self.frames.lock();
        for frame in samples.chunks_exact(channels) {
            let left = frame[0] * gain;
            let right = if channels > 1 { frame[1] * gain } else { left };
            frames.push_back([left, right]);
        }
        let excess = frames.len().saturating_sub(self.capacity);
        frames.drain(..excess);
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    pub(crate) fn set_sample_rate(&self, sample_rate: u32) {
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
    }
}

/// Plays a live input (a microphone) at the output's rate: linear resampling when the
/// rates differ, ~40 ms of latency, silence on underrun.
pub struct RingSource {
    pub(crate) ring: Arc<SampleRing>,
    pub(crate) channel_count: usize,
    pub(crate) alive: Arc<AtomicBool>,
    position: f64,
    previous: [f32; 2],
    primed: bool,
}

impl RingSource {
    pub(crate) fn new(ring: Arc<SampleRing>, channel_count: usize, alive: Arc<AtomicBool>) -> Self {
        Self {
            ring,
            channel_count,
            alive,
            position: 0.0,
            previous: [0.0; 2],
            primed: false,
        }
    }
}

impl AudioSource for RingSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        let input_rate = self.ring.sample_rate().max(1);
        let step = input_rate as f64 / sample_rate.max(1) as f64;
        let target = (input_rate as usize * 40 / 1000).max(256);
        let mut frames = self.ring.frames.lock();
        if !self.primed {
            if frames.len() < target {
                return true;
            }
            self.primed = true;
        }
        // Drift: keep the queue near the target latency.
        if frames.len() > target * 4 {
            let excess = frames.len() - target;
            frames.drain(..excess);
        }
        for pair in out.as_chunks_mut::<2>().0.iter_mut() {
            let Some(next) = frames.front().copied() else {
                self.primed = false;
                break;
            };
            let t = self.position as f32;
            pair[0] += self.previous[0] + (next[0] - self.previous[0]) * t;
            pair[1] += self.previous[1] + (next[1] - self.previous[1]) * t;
            self.position += step;
            while self.position >= 1.0 {
                self.position -= 1.0;
                match frames.pop_front() {
                    Some(frame) => self.previous = frame,
                    None => break,
                }
            }
        }
        true
    }

    fn channel_count(&self) -> usize {
        self.channel_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_sums_sources_and_drops_finished_ones() {
        let bus = Bus::new();
        bus.add(FnSource(|out: &mut [f32], _| {
            out.iter_mut().for_each(|s| *s += 0.25);
            true
        }));
        bus.add(FnSource(move |out: &mut [f32], _| {
            out.iter_mut().for_each(|s| *s += 0.5);
            false
        }));
        let mut out = [0.0f32; 4];
        bus.render(&mut out, 48_000);
        assert_eq!(out, [0.75; 4]);
        assert_eq!(bus.len(), 1);
    }

    #[test]
    fn ring_plays_mono_on_both_sides_after_priming() {
        let ring = Arc::new(SampleRing::new(48_000));
        let alive = Arc::new(AtomicBool::new(true));
        let mut source = RingSource::new(ring.clone(), 1, alive.clone());
        let mut out = vec![0.0f32; 64];
        source.render(&mut out, 48_000);
        assert!(out.iter().all(|s| *s == 0.0), "silent until primed");
        ring.push_interleaved(&vec![0.5; 4096], 1, 1.0);
        source.render(&mut out, 48_000);
        assert!((out[62] - 0.5).abs() < 1e-6 && (out[63] - 0.5).abs() < 1e-6);
        alive.store(false, Ordering::Release);
        assert!(!source.render(&mut out, 48_000));
    }

    #[test]
    fn device_frames_map_stereo_to_mono_and_surround() {
        let mut mono = [0.0f32; 2];
        write_device_frames(&mut mono, 1, &[0.2, 0.4, 2.0, 2.0]);
        assert!((mono[0] - 0.3).abs() < 1e-6 && mono[1] == 1.0);
        let mut quad = [9.0f32; 4];
        write_device_frames(&mut quad, 4, &[0.1, -0.1]);
        assert_eq!(quad, [0.1, -0.1, 0.0, 0.0]);
    }
}
