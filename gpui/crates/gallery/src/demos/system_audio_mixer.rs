//! Gallery page for `system-audio-mixer`. One variant per docs example:
//! `components/examples/system-audio-mixer-demo.tsx`,
//! `components/examples/system-audio-mixer-console.tsx`.
//!
//! The docs play synthesised tracks and sound pads (`useDemoTracks`,
//! `useDemoSounds`). Here those are the same titles, artists, hotkeys, accents
//! and durations, and the block's built-in simulated players.
//!
//! With the `device` feature (native) the microphone and system-audio strips
//! open the real devices (`audiocn::io`): the device list, the statuses and
//! the meters are real, the meters after the strip's fader and mute like
//! `useWebAudioMixer`'s. Without it (the wasm build) demo signals stand in.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::theme::oklch;
use audiocn::{MixerSound, MixerTrack, Orientation, SystemAudioMixer};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "system-audio-mixer",
    title: "System Audio Mixer",
    description: "The complete mixer, with microphone, system audio, music, sound pads and a master, running on Web Audio.",
    variants: &["default", "console"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => mixer(variant, false, window, cx),
        "console" => mixer(variant, true, window, cx),
        _ => return None,
    })
}

/// `DEMO_TRACKS` of `lib/docs/demo-audio.ts`.
fn tracks() -> Vec<MixerTrack> {
    vec![
        MixerTrack::new("night-drive", "Night Drive")
            .artist("audiocn")
            .src("night-drive")
            .duration(19.2),
        MixerTrack::new("low-tide", "Low Tide")
            .artist("audiocn")
            .src("low-tide")
            .duration(24.0),
        MixerTrack::new("arcade", "Arcade")
            .artist("audiocn")
            .src("arcade")
            .duration(13.7),
    ]
}

/// `DEMO_SOUNDS` of `lib/docs/demo-audio.ts`, with each sound's length.
fn sounds() -> Vec<MixerSound> {
    let sound = |id, label, hotkey, accent: (f32, f32, f32), seconds| {
        MixerSound::new(id, label)
            .src(id)
            .hotkey(hotkey)
            .accent(oklch(accent.0, accent.1, accent.2))
            .duration(seconds)
    };
    vec![
        sound("airhorn", "Airhorn", "1", (0.7, 0.2, 30.0), 1.4),
        sound("applause", "Applause", "2", (0.75, 0.15, 85.0), 2.4),
        sound("ding", "Ding", "3", (0.7, 0.15, 250.0), 1.7),
        sound("laser", "Laser", "4", (0.7, 0.2, 320.0), 0.5),
        sound("success", "Success", "5", (0.72, 0.17, 150.0), 1.3),
        sound("fail", "Fail", "6", (0.65, 0.2, 20.0), 2.0),
        sound("whoosh", "Whoosh", "7", (0.7, 0.1, 200.0), 1.2),
        sound("drumroll", "Drum roll", "8", (0.68, 0.16, 60.0), 2.8),
    ]
}

/// `system-audio-mixer-demo.tsx` (`w-full`) and `system-audio-mixer-console.tsx`
/// (`h-[30rem] w-full`, vertical).
fn mixer(variant: &str, console: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("system-audio-mixer", variant);
    let mut mixer = device::wire(SystemAudioMixer::new(base.clone()), &base, window, cx)
        .sounds(sounds())
        .tracks(tracks())
        .w_full();
    if console {
        mixer = mixer
            .default_orientation(Orientation::Vertical)
            .h(px(480.0));
    }
    mixer.into_any_element()
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod device {
    use gpui::{App, Window};

    use audiocn::SystemAudioMixer;

    /// No devices: the block's demo signals.
    pub fn wire(
        block: SystemAudioMixer,
        _base: &str,
        _window: &mut Window,
        _cx: &mut App,
    ) -> SystemAudioMixer {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod device {
    use gpui::{App, SharedString, Window};

    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{
        AnalyserChannels, AnalyserTapOptions, MicrophoneHandle, MicrophoneOptions,
        SystemAudioOptions, use_audio_devices, use_microphone, use_system_audio,
    };
    use audiocn::signal::mixer::{MixerOptions, MixerStore, use_mixer};
    use audiocn::{AudioDeviceKind, MixerSourceId, SystemAudioMixer, system_audio_mixer_channels};

    struct DeviceUi {
        device_id: Option<SharedString>,
        /// `useSystemAudio().isSupported`, listed once.
        system_supported: bool,
    }

    /// The strip's fader and mute (and solo elsewhere) on the input, so the
    /// meter reads after them like `useWebAudioMixer`'s per-channel meter.
    fn follow_strip(handle: Option<MicrophoneHandle>, mixer: &MixerStore, channel: &str) {
        let (Some(handle), Some(strip)) = (handle, mixer.channel(channel)) else {
            return;
        };
        handle.set_gain_db(strip.gain_db);
        handle.set_muted(!mixer.is_audible(channel));
    }

    /// `useAudioDevices`, `useMicrophone({ deviceId })` and `useSystemAudio`,
    /// with the mixer state owned here so their meters follow the faders.
    pub fn wire(
        block: SystemAudioMixer,
        base: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> SystemAudioMixer {
        let ui = window.use_keyed_state(format!("{base}/device-ui"), cx, |_, _| DeviceUi {
            device_id: None,
            system_supported: is_supported(),
        });
        let (device_id, system_supported) = {
            let ui = ui.read(cx);
            (ui.device_id.clone(), ui.system_supported)
        };
        let mixer = use_mixer(
            format!("{base}/device-mixer"),
            MixerOptions {
                channels: system_audio_mixer_channels(),
                ..Default::default()
            },
            window,
            cx,
        );
        let devices = use_audio_devices(
            format!("{base}/devices"),
            AudioDeviceKind::AudioInput,
            window,
            cx,
        );
        let stereo = AnalyserTapOptions {
            channels: AnalyserChannels::Stereo,
            ..Default::default()
        };
        let microphone = use_microphone(
            format!("{base}/microphone"),
            MicrophoneOptions {
                device_id: device_id.as_ref().map(ToString::to_string),
                analyser: stereo,
                ..Default::default()
            },
            window,
            cx,
        );
        let system = use_system_audio(
            format!("{base}/system"),
            SystemAudioOptions {
                analyser: stereo,
                ..Default::default()
            },
            window,
            cx,
        );
        {
            let store = mixer.read(cx);
            follow_strip(microphone.handle.clone(), store, "microphone");
            follow_strip(system.stream(), store, "system");
        }
        let refresh = devices.clone();
        let (start, stop) = (microphone.clone(), microphone.clone());
        let capture = system.clone();
        block
            .mixer(mixer)
            .devices(devices.devices.clone())
            .devices_loading(devices.is_loading)
            .permission(devices.permission)
            .on_request_permission(move |_, _| {
                refresh.request_permission();
            })
            .device_id(device_id)
            .on_device_change(move |id, _, cx| {
                ui.update(cx, |ui, cx| {
                    ui.device_id = id;
                    cx.notify();
                });
            })
            .microphone_status(microphone.status.into())
            .on_microphone_start(move |_, cx| start.start(cx))
            .on_microphone_stop(move |_, cx| stop.stop(cx))
            .meter(MixerSourceId::Microphone, microphone.meter.clone())
            .system_supported(system_supported)
            .system_status(system.status().into())
            .on_system_change(move |on, window, _| {
                if on {
                    capture.start();
                } else {
                    capture.stop();
                }
                window.refresh();
            })
            .meter(MixerSourceId::System, system.meter_source())
    }
}
