//! Port of `hooks/use-audio-devices.ts`: lists audio devices as
//! [`AudioDevice`]s (the type [`AudioDeviceSelect`](crate::AudioDeviceSelect)
//! takes) and keeps the list current as devices come and go.
//!
//! Device ids are cpal's `"<host>:<device>"` strings (e.g.
//! `pulseaudio:alsa_input.usb-…analog-stereo`); pass one to
//! [`MicrophoneOptions::device_id`](super::MicrophoneOptions) or
//! [`AudioOutput::open`](super::AudioOutput::open). Desktop systems don't
//! gate device labels behind a permission, so `permission` is always
//! [`AudioPermission::Granted`]. Browsers poll nothing (they get a
//! `devicechange` event); here a background thread re-lists every second.
//!
//! ```ignore
//! let devices = use_audio_devices("mics", AudioDeviceKind::AudioInput, window, cx);
//! AudioDeviceSelect::new("mic", devices.devices.clone()).loading(devices.is_loading)
//! ```

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use gpui::{App, ElementId, Window};
use parking_lot::Mutex;

use super::{AudioIoError, AudioIoErrorKind, Result};
use crate::components::{AudioDevice, AudioDeviceKind, AudioPermission};

/// `DEFAULT_DEVICE_ID`: also accepted wherever a device id is.
pub const DEFAULT_DEVICE_ID: &str = "default";
/// How often [`DeviceWatcher`] re-lists devices.
pub const DEVICE_POLL_INTERVAL: Duration = Duration::from_secs(1);

// ---------------------------------------------------------------------------
// Host access
// ---------------------------------------------------------------------------

static HOST: Mutex<Option<cpal::Host>> = Mutex::new(None);

/// Runs `f` with the process-wide cpal host (PulseAudio/PipeWire when its
/// server is reachable, else ALSA), connecting on first use.
pub(crate) fn with_host<R>(f: impl FnOnce(&cpal::Host) -> R) -> R {
    let mut host = HOST.lock();
    let host = host.get_or_insert_with(cpal::default_host);
    f(host)
}

/// The name of the cpal host in use (`"PulseAudio"`, `"ALSA"`, ...).
pub fn host_name() -> String {
    with_host(|host| host.id().name().to_string())
}

pub(crate) fn device_label(device: &cpal::Device) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_default()
}

pub(crate) fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

/// A PulseAudio/PipeWire monitor source: what a sink is playing.
pub(crate) fn is_monitor(id: &str, label: &str) -> bool {
    id.ends_with(".monitor") || label.to_lowercase().starts_with("monitor of")
}

fn fallback_label(kind: AudioDeviceKind, index: usize) -> String {
    match kind {
        AudioDeviceKind::AudioInput => format!("Microphone {}", index + 1),
        AudioDeviceKind::AudioOutput => format!("Speaker {}", index + 1),
    }
}

fn not_found(kind: AudioDeviceKind, id: &str) -> AudioIoError {
    let what = match kind {
        AudioDeviceKind::AudioInput => "input",
        AudioDeviceKind::AudioOutput => "output",
    };
    AudioIoError::new(
        AudioIoErrorKind::Unavailable,
        format!("no audio {what} device \"{id}\""),
    )
}

fn default_cpal_device(host: &cpal::Host, kind: AudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        AudioDeviceKind::AudioInput => host.default_input_device(),
        AudioDeviceKind::AudioOutput => host.default_output_device(),
    }
}

fn cpal_devices(host: &cpal::Host, kind: AudioDeviceKind) -> Result<Vec<cpal::Device>> {
    Ok(match kind {
        AudioDeviceKind::AudioInput => host.input_devices()?.collect(),
        AudioDeviceKind::AudioOutput => host.output_devices()?.collect(),
    })
}

/// Finds a device by id (or label); `None` / `"default"` is the default device.
pub(crate) fn find_device(kind: AudioDeviceKind, id: Option<&str>) -> Result<cpal::Device> {
    with_host(|host| {
        let id = id.filter(|id| !id.is_empty() && *id != DEFAULT_DEVICE_ID);
        let Some(id) = id else {
            return default_cpal_device(host, kind).ok_or_else(|| not_found(kind, "default"));
        };
        if let Ok(parsed) = cpal::DeviceId::from_str(id)
            && let Some(device) = host.device_by_id(&parsed)
        {
            return Ok(device);
        }
        cpal_devices(host, kind)?
            .into_iter()
            .find(|device| device_id(device).as_deref() == Some(id) || device_label(device) == id)
            .ok_or_else(|| not_found(kind, id))
    })
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// Device enumeration (`navigator.mediaDevices.enumerateDevices`).
pub struct AudioDevices;

impl AudioDevices {
    /// Every device of `kind`, the system default marked `is_default`.
    /// Inputs include monitor sources ("Monitor of …", description
    /// "System audio") when the sound server exposes them.
    pub fn list(kind: AudioDeviceKind) -> Result<Vec<AudioDevice>> {
        with_host(|host| {
            let default_id = default_cpal_device(host, kind).and_then(|d| device_id(&d));
            let devices = cpal_devices(host, kind)?;
            Ok(devices
                .iter()
                .enumerate()
                .filter_map(|(index, device)| {
                    let id = device_id(device)?;
                    let mut label = device_label(device);
                    if label.is_empty() {
                        label = fallback_label(kind, index);
                    }
                    let monitor = is_monitor(&id, &label);
                    let mut info = AudioDevice::new(id.clone(), label)
                        .with_kind(kind)
                        .with_default(default_id.as_deref() == Some(id.as_str()));
                    if monitor {
                        info = info.with_description("System audio");
                    }
                    Some(info)
                })
                .collect())
        })
    }

    /// The system default device of `kind`.
    pub fn default_device(kind: AudioDeviceKind) -> Option<AudioDevice> {
        Self::list(kind)
            .ok()?
            .into_iter()
            .find(|device| device.is_default)
    }

    /// Lists in the background and keeps the list current.
    pub fn watch(kind: AudioDeviceKind) -> DeviceWatcher {
        DeviceWatcher::new(kind, DEVICE_POLL_INTERVAL)
    }
}

// ---------------------------------------------------------------------------
// Watching
// ---------------------------------------------------------------------------

#[derive(Default)]
struct WatchState {
    devices: Mutex<Vec<AudioDevice>>,
    error: Mutex<Option<AudioIoError>>,
    loaded: AtomicBool,
    version: AtomicU64,
    refresh: AtomicBool,
    stop: AtomicBool,
}

/// A device list kept current by a background thread (the `devicechange`
/// listener of the hook). Clones share it; the thread stops with the last.
#[derive(Clone)]
pub struct DeviceWatcher {
    kind: AudioDeviceKind,
    state: Arc<WatchState>,
    _owner: Arc<WatchOwner>,
}

struct WatchOwner(Arc<WatchState>);

impl Drop for WatchOwner {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Release);
    }
}

impl DeviceWatcher {
    pub fn new(kind: AudioDeviceKind, interval: Duration) -> Self {
        let state = Arc::new(WatchState::default());
        let worker = state.clone();
        let spawned = thread::Builder::new()
            .name("audiocn-devices".into())
            .spawn(move || watch_loop(kind, interval, &worker));
        if let Err(error) = spawned {
            *state.error.lock() = Some(AudioIoError::new(
                AudioIoErrorKind::Other,
                error.to_string(),
            ));
            state.loaded.store(true, Ordering::Release);
        }
        Self {
            kind,
            _owner: Arc::new(WatchOwner(state.clone())),
            state,
        }
    }

    pub fn kind(&self) -> AudioDeviceKind {
        self.kind
    }

    /// The latest list.
    pub fn devices(&self) -> Vec<AudioDevice> {
        self.state.devices.lock().clone()
    }

    pub fn error(&self) -> Option<AudioIoError> {
        self.state.error.lock().clone()
    }

    /// True until the first listing finishes.
    pub fn is_loading(&self) -> bool {
        !self.state.loaded.load(Ordering::Acquire)
    }

    /// Increments whenever the list or error changes.
    pub fn version(&self) -> u64 {
        self.state.version.load(Ordering::Acquire)
    }

    /// Re-lists now instead of at the next poll.
    pub fn refresh(&self) {
        self.state.refresh.store(true, Ordering::Release);
    }
}

fn watch_loop(kind: AudioDeviceKind, interval: Duration, state: &WatchState) {
    const SLICE: Duration = Duration::from_millis(50);
    while !state.stop.load(Ordering::Acquire) {
        let changed = match AudioDevices::list(kind) {
            Ok(devices) => {
                let mut current = state.devices.lock();
                let had_error = state.error.lock().take().is_some();
                let changed = *current != devices || had_error;
                *current = devices;
                changed
            }
            Err(error) => {
                let mut current = state.error.lock();
                let changed = current.as_ref() != Some(&error);
                *current = Some(error);
                changed
            }
        };
        let first = !state.loaded.swap(true, Ordering::AcqRel);
        if changed || first {
            state.version.fetch_add(1, Ordering::AcqRel);
        }
        let mut waited = Duration::ZERO;
        while waited < interval
            && !state.stop.load(Ordering::Acquire)
            && !state.refresh.swap(false, Ordering::AcqRel)
        {
            thread::sleep(SLICE);
            waited += SLICE;
        }
    }
}

/// `UseAudioDevicesResult`.
#[derive(Clone)]
pub struct UseAudioDevicesResult {
    pub devices: Vec<AudioDevice>,
    /// Always `Granted` natively.
    pub permission: AudioPermission,
    pub is_loading: bool,
    pub error: Option<AudioIoError>,
    watcher: DeviceWatcher,
}

impl UseAudioDevicesResult {
    /// `refresh()`: re-lists now.
    pub fn refresh(&self) {
        self.watcher.refresh();
    }

    /// `requestPermission()`: nothing to ask for natively; re-lists and
    /// returns true.
    pub fn request_permission(&self) -> bool {
        self.watcher.refresh();
        true
    }
}

struct DevicesState {
    watcher: DeviceWatcher,
    seen: u64,
    _poll: gpui::Task<()>,
}

fn spawn_poll(cx: &mut gpui::Context<DevicesState>) -> gpui::Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let alive = this.update(cx, |state, cx| {
                let version = state.watcher.version();
                if version != state.seen {
                    state.seen = version;
                    cx.notify();
                }
            });
            if alive.is_err() {
                return;
            }
        }
    })
}

/// `useAudioDevices({ kind })`: the device list, re-rendering the view when it
/// changes. Keeps a [`DeviceWatcher`] in keyed state under `id`.
pub fn use_audio_devices(
    id: impl Into<ElementId>,
    kind: AudioDeviceKind,
    window: &mut Window,
    cx: &mut App,
) -> UseAudioDevicesResult {
    let state = window.use_keyed_state(id, cx, |_, cx| DevicesState {
        watcher: AudioDevices::watch(kind),
        seen: 0,
        _poll: spawn_poll(cx),
    });
    if state.read(cx).watcher.kind() != kind {
        state.update(cx, |state, _| {
            state.watcher = AudioDevices::watch(kind);
            state.seen = 0;
        });
    }
    let watcher = state.read(cx).watcher.clone();
    UseAudioDevicesResult {
        devices: watcher.devices(),
        permission: AudioPermission::Granted,
        is_loading: watcher.is_loading(),
        error: watcher.error(),
        watcher,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_monitor_sources() {
        assert!(is_monitor(
            "pulseaudio:alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
            "Monitor of Built-in Audio"
        ));
        assert!(is_monitor("alsa:loop", "Monitor of Speakers"));
        assert!(!is_monitor(
            "pulseaudio:alsa_input.usb-mic.analog-stereo",
            "USB Mic"
        ));
    }

    #[test]
    fn fallback_labels_match_the_hook() {
        assert_eq!(
            fallback_label(AudioDeviceKind::AudioInput, 0),
            "Microphone 1"
        );
        assert_eq!(fallback_label(AudioDeviceKind::AudioOutput, 2), "Speaker 3");
    }
}
