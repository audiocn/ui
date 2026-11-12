//! Port of `hooks/use-system-audio.ts`: capture what the computer is playing.
//!
//! Browsers capture system audio through the screen-share picker. Natively
//! there is no picker: on Linux, PulseAudio and PipeWire (through
//! `pipewire-pulse`) expose a **monitor source** per output ("Monitor of
//! Built-in Audio"), which cpal's PulseAudio host lists as an input device.
//! [`SystemAudio::start`] opens the monitor of the default output (or the
//! first monitor found) like a microphone. Plain ALSA has no monitors (unless
//! a loopback device is configured), so the status is then `Unsupported`.
//! On macOS/Windows the same applies to any loopback input that shows up
//! with "monitor" in its name; WASAPI loopback is not wired up.
//!
//! ```ignore
//! let system = SystemAudio::new(SystemAudioOptions::default());
//! system.start();
//! Spectrum::new("system").source(system.visual_source());
//! ```

use std::sync::Arc;

use gpui::{App, ElementId, Window};
use parking_lot::Mutex;

use super::analyser::AnalyserTapOptions;
use super::devices::{AudioDevices, is_monitor, with_host};
use super::microphone::{Microphone, MicrophoneHandle, MicrophoneOptions, MicrophoneStatus};
use super::{AudioIoError, AudioIoErrorKind};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::types::{MeterFrame, VisualFrame};
use crate::components::{AudioDevice, AudioDeviceKind};

/// `SystemAudioStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SystemAudioStatus {
    #[default]
    Idle,
    /// Opening the monitor source (the browser's picker).
    Prompting,
    Active,
    /// Not reachable natively (kept for parity).
    NoAudio,
    Denied,
    /// The capture stopped on its own (the output went away).
    Ended,
    /// No monitor source to capture.
    Unsupported,
}

/// `UseSystemAudioOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemAudioOptions {
    /// The whole system's audio. Natively always the case. Default true.
    pub system_audio: bool,
    /// No tabs natively; accepted for parity. Default false.
    pub prefer_current_tab: bool,
    /// Capture this monitor device instead of the default output's.
    pub device_id: Option<String>,
    /// The analyser behind the meter and visual sources.
    pub analyser: AnalyserTapOptions,
}

impl Default for SystemAudioOptions {
    fn default() -> Self {
        Self {
            system_audio: true,
            prefer_current_tab: false,
            device_id: None,
            analyser: AnalyserTapOptions::default(),
        }
    }
}

/// Monitor sources, the default output's first.
pub fn monitor_devices() -> Vec<AudioDevice> {
    let Ok(inputs) = AudioDevices::list(AudioDeviceKind::AudioInput) else {
        return Vec::new();
    };
    let default_sink = with_host(|host| {
        use cpal::traits::HostTrait;
        host.default_output_device()
            .and_then(|device| super::devices::device_id(&device))
    });
    let mut monitors: Vec<AudioDevice> = inputs
        .into_iter()
        .filter(|device| is_monitor(&device.id, &device.label))
        .collect();
    if let Some(sink) = default_sink {
        let wanted = format!("{sink}.monitor");
        monitors.sort_by_key(|device| device.id.as_ref() != wanted);
    }
    monitors
}

/// The monitor of the default output, if the system exposes one.
pub fn find_monitor_device() -> Option<AudioDevice> {
    monitor_devices().into_iter().next()
}

/// `isSupported`: a monitor source exists.
pub fn is_supported() -> bool {
    find_monitor_device().is_some()
}

struct SystemInner {
    options: SystemAudioOptions,
    capture: Option<MicrophoneHandle>,
    status: SystemAudioStatus,
    error: Option<AudioIoError>,
}

/// A system-audio capture (`useSystemAudio`'s controller). Clones share it.
#[derive(Clone)]
pub struct SystemAudio {
    inner: Arc<Mutex<SystemInner>>,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
}

impl SystemAudio {
    pub fn new(options: SystemAudioOptions) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SystemInner {
                options,
                capture: None,
                status: SystemAudioStatus::Idle,
                error: None,
            })),
            meter: create_frame_emitter(),
            visual: create_frame_emitter(),
        }
    }

    /// Starts capturing (non-blocking: `Prompting`, then `Active`).
    pub fn start(&self) {
        let mut inner = self.inner.lock();
        if let Some(capture) = inner.capture.take() {
            capture.stop();
        }
        inner.error = None;
        let device_id = match inner.options.device_id.clone() {
            Some(id) => Some(id),
            None => find_monitor_device().map(|device| device.id.to_string()),
        };
        let Some(device_id) = device_id else {
            inner.status = SystemAudioStatus::Unsupported;
            inner.error = Some(AudioIoError::new(
                AudioIoErrorKind::Unsupported,
                "no monitor source: system audio needs PulseAudio or PipeWire",
            ));
            return;
        };
        let options = MicrophoneOptions {
            device_id: Some(device_id),
            analyser: inner.options.analyser,
            ..Default::default()
        };
        inner.capture = Some(Microphone::open_with(
            options,
            self.meter.clone(),
            self.visual.clone(),
        ));
        inner.status = SystemAudioStatus::Prompting;
    }

    /// Stops capturing (`Idle`).
    pub fn stop(&self) {
        let mut inner = self.inner.lock();
        if let Some(capture) = inner.capture.take() {
            capture.stop();
        }
        inner.status = SystemAudioStatus::Idle;
    }

    pub fn status(&self) -> SystemAudioStatus {
        let mut inner = self.inner.lock();
        let Some(capture) = inner.capture.clone() else {
            return inner.status;
        };
        let status = match capture.status() {
            MicrophoneStatus::Acquiring => SystemAudioStatus::Prompting,
            MicrophoneStatus::Active => SystemAudioStatus::Active,
            MicrophoneStatus::Denied => SystemAudioStatus::Denied,
            MicrophoneStatus::Unavailable if inner.status == SystemAudioStatus::Active => {
                SystemAudioStatus::Ended
            }
            MicrophoneStatus::Unavailable => SystemAudioStatus::Unsupported,
            MicrophoneStatus::Idle | MicrophoneStatus::Error => SystemAudioStatus::Idle,
        };
        if status != SystemAudioStatus::Active && status != SystemAudioStatus::Prompting {
            inner.error = capture.error();
        }
        inner.status = status;
        status
    }

    /// The capture's error, if any.
    pub fn error(&self) -> Option<AudioIoError> {
        self.status();
        self.inner.lock().error.clone()
    }

    /// The capture as a microphone-like handle (the hook's `stream`), e.g.
    /// for [`AudioInput::Microphone`](super::AudioInput).
    pub fn stream(&self) -> Option<MicrophoneHandle> {
        self.inner.lock().capture.clone()
    }

    pub fn meter_source(&self) -> FrameSource<MeterFrame> {
        self.meter.source()
    }

    pub fn visual_source(&self) -> FrameSource<VisualFrame> {
        self.visual.source()
    }
}

struct UseSystemAudioState {
    system: SystemAudio,
}

/// `useSystemAudio`: a [`SystemAudio`] in keyed state under `id`;
/// re-renders while it starts. Call `start()` from a click like React.
pub fn use_system_audio(
    id: impl Into<ElementId>,
    options: SystemAudioOptions,
    window: &mut Window,
    cx: &mut App,
) -> SystemAudio {
    let state = window.use_keyed_state(id, cx, |_, _| UseSystemAudioState {
        system: SystemAudio::new(options),
    });
    let system = state.read(cx).system.clone();
    if system.status() == SystemAudioStatus::Prompting {
        window.request_animation_frame();
    }
    system
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle_and_stops_to_idle() {
        let system = SystemAudio::new(SystemAudioOptions::default());
        assert_eq!(system.status(), SystemAudioStatus::Idle);
        system.stop();
        assert_eq!(system.status(), SystemAudioStatus::Idle);
        assert!(system.stream().is_none());
    }
}
