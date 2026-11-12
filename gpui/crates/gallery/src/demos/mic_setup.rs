//! Gallery page for `mic-setup`. One variant per docs example: `components/examples/mic-setup-demo.tsx`.
//!
//! With the `device` feature (native) the block lists the real input devices
//! and opens the real microphone (`audiocn::io`), its gain and mute applied
//! before the meter like React's gain node. Without it (the wasm build) the
//! block's demo signal starts when the preview button is pressed.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::MicSetup;

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "mic-setup",
    title: "Microphone Setup",
    description: "Choose and check a microphone, with a live preview, a meter, gain, mute and a level check.",
    variants: &["default"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        // `mic-setup-demo.tsx`: `<MicSetup className="w-full max-w-md" />`.
        "default" => device::wire(MicSetup::new(id("mic-setup", "default")), window, cx)
            .w_full()
            .max_w(px(448.0))
            .into_any_element(),
        _ => return None,
    })
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod device {
    use gpui::{App, Window};

    use audiocn::MicSetup;

    /// No microphone: the block's demo signal.
    pub fn wire(block: MicSetup, _window: &mut Window, _cx: &mut App) -> MicSetup {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod device {
    use gpui::{App, SharedString, Window};

    use audiocn::io::{AnalyserTapOptions, MicrophoneOptions, use_audio_devices, use_microphone};
    use audiocn::{AudioDeviceKind, MicSetup};

    use super::id;

    struct MicUi {
        started: bool,
        device_id: Option<SharedString>,
        gain_db: f32,
        muted: bool,
    }

    /// `useAudioDevices` + `useMicrophone({ deviceId, enabled: started })`
    /// + the gain node in front of `useAudioAnalyser`.
    pub fn wire(block: MicSetup, window: &mut Window, cx: &mut App) -> MicSetup {
        let base = id("mic-setup", "default");
        let ui = window.use_keyed_state(format!("{base}/device-ui"), cx, |_, _| MicUi {
            started: false,
            device_id: None,
            gain_db: 0.0,
            muted: false,
        });
        let (started, device_id, gain_db, muted) = {
            let ui = ui.read(cx);
            (ui.started, ui.device_id.clone(), ui.gain_db, ui.muted)
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
                enabled: started,
                // `useAudioAnalyser(..., { historySize: 120 })`.
                analyser: AnalyserTapOptions {
                    history_size: 120,
                    ..Default::default()
                },
                ..Default::default()
            },
            window,
            cx,
        );
        if let Some(handle) = &microphone.handle {
            handle.set_gain_db(gain_db);
            handle.set_muted(muted);
        }
        let refresh = devices.clone();
        let (ui_device, ui_gain, ui_mute, ui_start) = (ui.clone(), ui.clone(), ui.clone(), ui);
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
            .gain_db(gain_db)
            .on_gain_change(move |db, _, cx| {
                ui_gain.update(cx, |ui, cx| {
                    ui.gain_db = db;
                    cx.notify();
                });
            })
            .muted(muted)
            .on_muted_change(move |muted, _, cx| {
                ui_mute.update(cx, |ui, cx| {
                    ui.muted = muted;
                    cx.notify();
                });
            })
            .status(microphone.status.into())
            .meter_source(microphone.meter.clone())
            .visual_source(microphone.visual.clone())
            .on_start(move |_, cx| {
                ui_start.update(cx, |ui, cx| {
                    ui.started = true;
                    cx.notify();
                });
            })
    }
}
