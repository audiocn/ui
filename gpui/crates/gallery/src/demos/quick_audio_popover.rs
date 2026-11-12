//! Gallery page for `quick-audio-popover`. One variant per docs example:
//! `components/examples/quick-audio-popover-demo.tsx`.
//!
//! With the `device` feature (native) the popover lists the real input
//! devices, "Turn on" opens the real microphone (its analyser drives the
//! trigger's bars) and the switch captures the system output
//! (`audiocn::io`). Without it (the wasm build) the microphone stays off and
//! "Turn on" feeds the bars with the speech demo signal.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window};

use audiocn::QuickAudioPopover;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "quick-audio-popover",
    title: "Quick Audio Popover",
    description: "Microphone and system audio controls behind one toolbar button that shows a live level.",
    variants: &["default"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        _ => return None,
    })
}

/// `quick-audio-popover-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    device::wire(
        QuickAudioPopover::new(id("quick-audio-popover", "default")),
        window,
        cx,
    )
    .default_open(OPEN)
    .child(
        Button::new("quick-audio-popover/more")
            .w_full()
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Ghost)
            .child("More audio settings"),
    )
    .into_any_element()
}

/// Closed, like the docs. (Flip to screenshot the panel.)
const OPEN: bool = false;

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod device {
    use gpui::{App, Window};

    use audiocn::QuickAudioPopover;

    /// No devices: the block's demo microphone list and signal.
    pub fn wire(
        block: QuickAudioPopover,
        _window: &mut Window,
        _cx: &mut App,
    ) -> QuickAudioPopover {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod device {
    use gpui::{App, SharedString, Window};

    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{
        MicrophoneOptions, SystemAudioOptions, SystemAudioStatus, use_audio_devices,
        use_microphone, use_system_audio,
    };
    use audiocn::{AudioDeviceKind, QuickAudioPopover};

    use super::id;

    struct AudioUi {
        device_id: Option<SharedString>,
        muted: bool,
        /// `useSystemAudio().isSupported`, listed once.
        system_supported: bool,
    }

    /// `useAudioDevices`, `useMicrophone({ deviceId })`,
    /// `useAudioAnalyser(stream, { enabled: !muted })` and `useSystemAudio`.
    pub fn wire(block: QuickAudioPopover, window: &mut Window, cx: &mut App) -> QuickAudioPopover {
        let base = id("quick-audio-popover", "default");
        let ui = window.use_keyed_state(format!("{base}/device-ui"), cx, |_, _| AudioUi {
            device_id: None,
            muted: false,
            system_supported: is_supported(),
        });
        let (device_id, muted, system_supported) = {
            let ui = ui.read(cx);
            (ui.device_id.clone(), ui.muted, ui.system_supported)
        };
        let devices = use_audio_devices(
            format!("{base}/devices"),
            AudioDeviceKind::AudioInput,
            window,
            cx,
        );
        let microphone = use_microphone(
            format!("{base}/microphone"),
            MicrophoneOptions {
                device_id: device_id.as_ref().map(ToString::to_string),
                ..Default::default()
            },
            window,
            cx,
        );
        let system = use_system_audio(
            format!("{base}/system"),
            SystemAudioOptions::default(),
            window,
            cx,
        );
        let active = microphone.status == audiocn::io::MicrophoneStatus::Active;
        let refresh = devices.clone();
        let mic = microphone.clone();
        let capture = system.clone();
        let (ui_device, ui_mute) = (ui.clone(), ui);
        block
            .devices(devices.devices.clone())
            .loading(devices.is_loading)
            .permission(devices.permission)
            .on_request_permission(move |_, _| {
                refresh.request_permission();
            })
            .device_id(device_id)
            .on_device_change(move |id, _, cx| {
                ui_device.update(cx, |ui, cx| {
                    ui.device_id = id;
                    cx.notify();
                });
            })
            .microphone_active(active)
            .on_microphone(move |on, _, cx| {
                if on {
                    mic.start(cx);
                } else {
                    mic.stop(cx);
                }
            })
            .muted(muted)
            .on_muted_change(move |muted, _, cx| {
                ui_mute.update(cx, |ui, cx| {
                    ui.muted = muted;
                    cx.notify();
                });
            })
            .source(microphone.visual.clone())
            .system_supported(system_supported)
            .system_active(system.status() == SystemAudioStatus::Active)
            .on_system_audio(move |on, window, _| {
                if on {
                    capture.start();
                } else {
                    capture.stop();
                }
                window.refresh();
            })
    }
}
