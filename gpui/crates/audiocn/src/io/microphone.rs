//! Port of `hooks/use-microphone.ts` (with `useAudioAnalyser` built in).
//!
//! [`Microphone::open`] opens an input device on its own thread and returns
//! a [`MicrophoneHandle`] at once (status `Acquiring`, then `Active` or an
//! error status); [`Microphone::start`] waits for the result. A handle has
//! stable [`meter_source`](MicrophoneHandle::meter_source) /
//! [`visual_source`](MicrophoneHandle::visual_source) for meters and
//! visualizers, a gain and mute applied before analysis, and can feed
//! other nodes ([`AudioInput::Microphone`](super::AudioInput)).
//!
//! The browser processing switches (`echo_cancellation`, `noise_suppression`,
//! `auto_gain_control`) are accepted for parity; native capture is always raw,
//! which is what they default to (off).
//!
//! ```ignore
//! let mic = Microphone::start(MicrophoneOptions::default())?;
//! LevelMeter::new("mic").source(mic.meter_source());
//! // or, in a render function:
//! let mic = use_microphone("mic", MicrophoneOptions { enabled: true, ..Default::default() }, window, cx);
//! LevelMeter::new("mic").source(mic.meter.clone())
//! ```

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use cpal::traits::DeviceTrait;
use gpui::{App, ElementId, Entity, Window};
use parking_lot::Mutex;

use super::analyser::{AnalyserTap, AnalyserTapOptions, TapWriter};
use super::context::{
    AtomicF32, RingSource, SampleRing, StreamThread, prefer_f32, stream_config, with_sample_type,
};
use super::devices::{device_id, device_label, find_device};
use super::{AudioIoError, AudioIoErrorKind, Result};
use crate::audio::decibels::{db_to_gain, gain_to_db};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::types::{MeterFrame, VisualFrame};
use crate::components::AudioDeviceKind;

/// `MicrophoneStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MicrophoneStatus {
    #[default]
    Idle,
    /// Opening the device (`"acquiring"`).
    Acquiring,
    Active,
    Denied,
    /// No such device, or it went away.
    Unavailable,
    Error,
}

impl MicrophoneStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Acquiring,
            2 => Self::Active,
            3 => Self::Denied,
            4 => Self::Unavailable,
            5 => Self::Error,
            _ => Self::Idle,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Acquiring => 1,
            Self::Active => 2,
            Self::Denied => 3,
            Self::Unavailable => 4,
            Self::Error => 5,
        }
    }

    /// `statusForError`.
    pub fn for_error(error: &AudioIoError) -> Self {
        match error.kind {
            AudioIoErrorKind::Denied => Self::Denied,
            AudioIoErrorKind::Unavailable => Self::Unavailable,
            _ => Self::Error,
        }
    }
}

/// `UseMicrophoneOptions`, plus the analyser settings for its sources.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MicrophoneOptions {
    /// The device to open ([`AudioDevice::id`](crate::AudioDevice)). `None`
    /// for the system default.
    pub device_id: Option<String>,
    /// [`use_microphone`] only: open as soon as possible. Default false.
    pub enabled: bool,
    /// Accepted for parity; native capture never processes. Default false.
    pub echo_cancellation: bool,
    /// Accepted for parity. Default false.
    pub noise_suppression: bool,
    /// Accepted for parity. Default false.
    pub auto_gain_control: bool,
    /// Requested channel count (used when the device offers it).
    pub channel_count: Option<u16>,
    /// The analyser behind [`MicrophoneHandle::meter_source`] and
    /// [`MicrophoneHandle::visual_source`].
    pub analyser: AnalyserTapOptions,
}

impl MicrophoneOptions {
    /// What reopens the device when it changes (everything but `enabled`).
    fn key(&self) -> MicrophoneOptions {
        MicrophoneOptions {
            enabled: false,
            ..self.clone()
        }
    }
}

struct MicShared {
    status: AtomicU8,
    error: Mutex<Option<AudioIoError>>,
    gain: AtomicF32,
    muted: AtomicBool,
    alive: Arc<AtomicBool>,
    rings: Mutex<Vec<Weak<SampleRing>>>,
    sample_rate: AtomicU32,
    channels: AtomicU32,
    device_id: Mutex<Option<String>>,
    device_label: Mutex<String>,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
}

impl MicShared {
    fn set_status(&self, status: MicrophoneStatus) {
        self.status.store(status.to_u8(), Ordering::Release);
    }

    fn status(&self) -> MicrophoneStatus {
        MicrophoneStatus::from_u8(self.status.load(Ordering::Acquire))
    }
}

struct MicInner {
    shared: Arc<MicShared>,
    thread: Mutex<Option<StreamThread>>,
    tap: Mutex<Option<AnalyserTap>>,
}

impl Drop for MicInner {
    fn drop(&mut self) {
        self.shared.alive.store(false, Ordering::Release);
    }
}

/// An open (or opening) microphone. Clones share it; the device closes on
/// [`stop`](Self::stop) or when the last clone drops.
#[derive(Clone)]
pub struct MicrophoneHandle {
    inner: Arc<MicInner>,
}

impl PartialEq for MicrophoneHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::fmt::Debug for MicrophoneHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrophoneHandle")
            .field("status", &self.status())
            .field("device", &self.device_label())
            .finish()
    }
}

/// Opens microphones (`navigator.mediaDevices.getUserMedia({ audio })`).
pub struct Microphone;

impl Microphone {
    /// Opens a microphone and waits until it is running.
    pub fn start(options: MicrophoneOptions) -> Result<MicrophoneHandle> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = Self::open_inner(
            options,
            create_frame_emitter(),
            create_frame_emitter(),
            Some(sender),
        );
        let _ = receiver.recv();
        match handle.status() {
            MicrophoneStatus::Active => Ok(handle),
            _ => Err(handle.error().unwrap_or_else(|| {
                AudioIoError::new(AudioIoErrorKind::Other, "the microphone did not start")
            })),
        }
    }

    /// Starts opening a microphone and returns at once (status `Acquiring`).
    pub fn open(options: MicrophoneOptions) -> MicrophoneHandle {
        Self::open_with(options, create_frame_emitter(), create_frame_emitter())
    }

    /// [`open`](Self::open), emitting into existing emitters so readers keep
    /// their sources across restarts.
    pub fn open_with(
        options: MicrophoneOptions,
        meter: FrameEmitter<MeterFrame>,
        visual: FrameEmitter<VisualFrame>,
    ) -> MicrophoneHandle {
        Self::open_inner(options, meter, visual, None)
    }

    fn open_inner(
        options: MicrophoneOptions,
        meter: FrameEmitter<MeterFrame>,
        visual: FrameEmitter<VisualFrame>,
        done: Option<std::sync::mpsc::Sender<()>>,
    ) -> MicrophoneHandle {
        let shared = Arc::new(MicShared {
            status: AtomicU8::new(MicrophoneStatus::Acquiring.to_u8()),
            error: Mutex::new(None),
            gain: AtomicF32::new(1.0),
            muted: AtomicBool::new(false),
            alive: Arc::new(AtomicBool::new(true)),
            rings: Mutex::new(Vec::new()),
            sample_rate: AtomicU32::new(0),
            channels: AtomicU32::new(0),
            device_id: Mutex::new(None),
            device_label: Mutex::new(String::new()),
            meter,
            visual,
        });
        let inner = Arc::new(MicInner {
            shared: shared.clone(),
            thread: Mutex::new(None),
            tap: Mutex::new(None),
        });
        let weak = Arc::downgrade(&inner);
        let build_shared = shared.clone();
        let thread = StreamThread::spawn(
            "audiocn-microphone",
            move || build_input_stream(&options, build_shared),
            move |result| {
                match result {
                    Ok(tap) => {
                        if let Some(inner) = weak.upgrade() {
                            *inner.tap.lock() = Some(tap);
                            if inner.shared.status() == MicrophoneStatus::Acquiring {
                                inner.shared.set_status(MicrophoneStatus::Active);
                            }
                        }
                    }
                    Err(error) => {
                        shared.set_status(MicrophoneStatus::for_error(&error));
                        *shared.error.lock() = Some(error);
                    }
                }
                if let Some(done) = done {
                    let _ = done.send(());
                }
            },
        );
        *inner.thread.lock() = Some(thread);
        MicrophoneHandle { inner }
    }
}

fn pick_input_config(
    device: &cpal::Device,
    channel_count: Option<u16>,
) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_input_config()?;
    let default = match device.supported_input_configs() {
        Ok(ranges) => prefer_f32(default, ranges),
        Err(_) => default,
    };
    let Some(wanted) = channel_count.filter(|count| *count != default.channels()) else {
        return Ok(default);
    };
    let rate = default.sample_rate();
    let matching = device.supported_input_configs()?.find(|range| {
        range.channels() == wanted
            && range.sample_format() == default.sample_format()
            && range.min_sample_rate() <= rate
            && rate <= range.max_sample_rate()
    });
    Ok(matching.map_or(default, |range| range.with_sample_rate(rate)))
}

fn build_input_stream(
    options: &MicrophoneOptions,
    shared: Arc<MicShared>,
) -> Result<(cpal::Stream, AnalyserTap)> {
    let device = find_device(AudioDeviceKind::AudioInput, options.device_id.as_deref())?;
    let supported = pick_input_config(&device, options.channel_count)?;
    let config = stream_config(&supported);
    shared
        .sample_rate
        .store(config.sample_rate, Ordering::Release);
    shared
        .channels
        .store(u32::from(config.channels), Ordering::Release);
    *shared.device_id.lock() = device_id(&device);
    *shared.device_label.lock() = device_label(&device);
    let tap = AnalyserTap::with_emitters(
        options.analyser,
        config.sample_rate,
        shared.meter.clone(),
        shared.visual.clone(),
    );
    let writer = tap.writer();
    let stream = with_sample_type!(supported.sample_format(), T => build_input::<T>(
        &device, config, shared, writer
    ))?;
    Ok((stream, tap))
}

fn build_input<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<MicShared>,
    writer: TapWriter,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = usize::from(config.channels.max(1));
    let sample_rate = config.sample_rate;
    let mut scratch: Vec<f32> = Vec::with_capacity(8192);
    let error_shared = shared.clone();
    let stream = device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            let gain = if shared.muted.load(Ordering::Relaxed) {
                0.0
            } else {
                shared.gain.load()
            };
            scratch.clear();
            scratch.extend(
                data.iter()
                    .map(|sample| <f32 as cpal::Sample>::from_sample(*sample) * gain),
            );
            writer.write(&scratch, channels);
            let mut rings = shared.rings.lock();
            rings.retain(|ring| match ring.upgrade() {
                Some(ring) => {
                    ring.set_sample_rate(sample_rate);
                    ring.push_interleaved(&scratch, channels, 1.0);
                    true
                }
                None => false,
            });
        },
        move |error| {
            if matches!(
                error.kind(),
                cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated
            ) {
                // The track ended (unplugged).
                error_shared.set_status(MicrophoneStatus::Unavailable);
                *error_shared.error.lock() = Some(error.into());
            }
        },
        None,
    )?;
    Ok(stream)
}

impl MicrophoneHandle {
    pub fn status(&self) -> MicrophoneStatus {
        self.inner.shared.status()
    }

    pub fn error(&self) -> Option<AudioIoError> {
        self.inner.shared.error.lock().clone()
    }

    /// Meter frames (peak/RMS dBFS per channel), post gain.
    pub fn meter_source(&self) -> FrameSource<MeterFrame> {
        self.inner.shared.meter.source()
    }

    /// Visual frames (bands, history, time domain), post gain.
    pub fn visual_source(&self) -> FrameSource<VisualFrame> {
        self.inner.shared.visual.source()
    }

    /// Closes the device. Meters clear; nodes fed by it go silent.
    pub fn stop(&self) {
        self.inner.thread.lock().take();
        self.inner.tap.lock().take();
        self.inner.shared.alive.store(false, Ordering::Release);
        self.inner.shared.meter.clear();
        self.inner.shared.visual.clear();
        self.inner.shared.set_status(MicrophoneStatus::Idle);
    }

    /// Input gain in dB, applied before analysis and routing.
    pub fn set_gain_db(&self, db: f32) {
        self.inner.shared.gain.store(db_to_gain(db));
    }

    pub fn gain_db(&self) -> f32 {
        gain_to_db(self.inner.shared.gain.load())
    }

    pub fn set_muted(&self, muted: bool) {
        self.inner.shared.muted.store(muted, Ordering::Release);
    }

    pub fn muted(&self) -> bool {
        self.inner.shared.muted.load(Ordering::Acquire)
    }

    /// The device's sample rate (0 until it is open).
    pub fn sample_rate(&self) -> u32 {
        self.inner.shared.sample_rate.load(Ordering::Acquire)
    }

    /// The device's channel count (0 until it is open).
    pub fn channels(&self) -> u16 {
        self.inner.shared.channels.load(Ordering::Acquire) as u16
    }

    /// The opened device's id.
    pub fn device_id(&self) -> Option<String> {
        self.inner.shared.device_id.lock().clone()
    }

    /// The opened device's name (the track label).
    pub fn device_label(&self) -> String {
        self.inner.shared.device_label.lock().clone()
    }

    /// The live signal as a source for the output's clock (a
    /// `MediaStreamAudioSourceNode`); several can exist at once.
    pub fn source(&self) -> RingSource {
        let shared = &self.inner.shared;
        let ring = Arc::new(SampleRing::new(shared.sample_rate.load(Ordering::Acquire)));
        shared.rings.lock().push(Arc::downgrade(&ring));
        let channels = shared.channels.load(Ordering::Acquire).max(1) as usize;
        RingSource::new(ring, channels.min(2), shared.alive.clone())
    }
}

// ---------------------------------------------------------------------------
// use_microphone
// ---------------------------------------------------------------------------

struct UseMicrophoneState {
    manual: Option<bool>,
    previous_enabled: bool,
    key: Option<MicrophoneOptions>,
    handle: Option<MicrophoneHandle>,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
}

/// `UseMicrophoneResult`: the status, stable frame sources and controls.
#[derive(Clone)]
pub struct UseMicrophone {
    pub status: MicrophoneStatus,
    pub error: Option<AudioIoError>,
    /// The microphone's meter frames; stays the same across start/stop.
    pub meter: FrameSource<MeterFrame>,
    pub visual: FrameSource<VisualFrame>,
    /// The open microphone (the hook's `stream`).
    pub handle: Option<MicrophoneHandle>,
    state: Entity<UseMicrophoneState>,
}

impl UseMicrophone {
    /// Opens the microphone (`start()`).
    pub fn start(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.manual = Some(true);
            cx.notify();
        });
    }

    /// Closes it (`stop()`).
    pub fn stop(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.manual = Some(false);
            cx.notify();
        });
    }
}

/// `useMicrophone`: keeps a microphone in keyed state under `id`, open while
/// `options.enabled` (or after [`UseMicrophone::start`]), reopened when the
/// options change. Re-renders while the device is being acquired.
pub fn use_microphone(
    id: impl Into<ElementId>,
    options: MicrophoneOptions,
    window: &mut Window,
    cx: &mut App,
) -> UseMicrophone {
    let state = window.use_keyed_state(id, cx, |_, _| UseMicrophoneState {
        manual: None,
        previous_enabled: options.enabled,
        key: None,
        handle: None,
        meter: create_frame_emitter(),
        visual: create_frame_emitter(),
    });
    let key = options.key();
    let (status, error, handle, meter, visual) = state.update(cx, |state, _| {
        if state.previous_enabled != options.enabled {
            state.previous_enabled = options.enabled;
            state.manual = None;
        }
        let wanted = state.manual.unwrap_or(options.enabled);
        if state.key.as_ref() != Some(&key)
            && let Some(handle) = state.handle.take()
        {
            handle.stop();
        }
        if wanted && state.handle.is_none() {
            state.key = Some(key);
            state.handle = Some(Microphone::open_with(
                options,
                state.meter.clone(),
                state.visual.clone(),
            ));
        } else if !wanted && let Some(handle) = state.handle.take() {
            handle.stop();
        }
        let status = state
            .handle
            .as_ref()
            .map_or(MicrophoneStatus::Idle, MicrophoneHandle::status);
        let error = state.handle.as_ref().and_then(MicrophoneHandle::error);
        (
            status,
            error,
            state.handle.clone(),
            state.meter.source(),
            state.visual.source(),
        )
    });
    if status == MicrophoneStatus::Acquiring {
        window.request_animation_frame();
    }
    UseMicrophone {
        status,
        error,
        meter,
        visual,
        handle,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_and_maps_errors() {
        for status in [
            MicrophoneStatus::Idle,
            MicrophoneStatus::Acquiring,
            MicrophoneStatus::Active,
            MicrophoneStatus::Denied,
            MicrophoneStatus::Unavailable,
            MicrophoneStatus::Error,
        ] {
            assert_eq!(MicrophoneStatus::from_u8(status.to_u8()), status);
        }
        let denied = AudioIoError::new(AudioIoErrorKind::Denied, "no");
        assert_eq!(
            MicrophoneStatus::for_error(&denied),
            MicrophoneStatus::Denied
        );
        let gone = AudioIoError::new(AudioIoErrorKind::Unavailable, "gone");
        assert_eq!(
            MicrophoneStatus::for_error(&gone),
            MicrophoneStatus::Unavailable
        );
        let other = AudioIoError::new(AudioIoErrorKind::Decode, "?");
        assert_eq!(MicrophoneStatus::for_error(&other), MicrophoneStatus::Error);
    }

    #[test]
    fn options_key_ignores_enabled() {
        let on = MicrophoneOptions {
            enabled: true,
            ..Default::default()
        };
        assert_eq!(on.key(), MicrophoneOptions::default().key());
        let other = MicrophoneOptions {
            device_id: Some("x".into()),
            ..Default::default()
        };
        assert_ne!(other.key(), MicrophoneOptions::default().key());
    }
}
