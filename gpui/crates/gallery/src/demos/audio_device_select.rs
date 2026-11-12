//! Gallery page for `audio-device-select`. One variant per docs example:
//! `components/examples/audio-device-select-demo.tsx`,
//! `components/examples/audio-device-select-states.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::{
    AudioDevice, AudioDeviceSelect, AudioDeviceSelectContent, AudioDeviceSelectPreview,
    AudioDeviceSelectTrigger, AudioDeviceSelectValue, AudioDeviceStatus, AudioPermission,
    LiveWaveform, LiveWaveformMode,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "audio-device-select",
    title: "Audio Device Select",
    description: "A microphone, speaker or source picker with permission, loading and disconnected states.",
    variants: &["default", "states"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "states" => states(),
        _ => return None,
    })
}

/// The gallery has no microphone access: "Allow access" grants a fixed list
/// (standing in for `useAudioDevices`), and the preview is fed by the demo
/// signal (standing in for `useMicrophone` + `useAudioAnalyser`).
struct DemoState {
    permission: AudioPermission,
    devices: Vec<AudioDevice>,
    device_id: Option<SharedString>,
}

fn granted_devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice::new("default", "MacBook Pro Microphone").with_default(true),
        AudioDevice::new("usb", "Shure MV7+").with_description("USB"),
    ]
}

/// `audio-device-select-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("audio-device-select", "default");
    let state = window.use_keyed_state(format!("{base}/state"), cx, |_, _| DemoState {
        permission: AudioPermission::Prompt,
        devices: Vec::new(),
        device_id: None,
    });
    let (permission, devices, device_id) = {
        let s = state.read(cx);
        (s.permission, s.devices.clone(), s.device_id.clone())
    };
    let active = device_id.is_some();
    let mut waveform = LiveWaveform::new(format!("{base}/preview"))
        .active(active)
        .bar_width(2.0)
        .mode(LiveWaveformMode::Scrolling)
        .h(px(32.0));
    if active {
        let signal = use_demo_signal(
            format!("{base}/signal"),
            DemoSignalOptions::kind(DemoSignalKind::Speech).with_history_size(120),
            window,
            cx,
        );
        waveform = waveform.source(signal.visual);
    }
    let (s1, s2) = (state.clone(), state);
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(8.0))
        .child(
            AudioDeviceSelect::new(format!("{base}/select"), devices)
                .loading(false)
                .permission(permission)
                .on_request_permission(move |_, cx| {
                    s1.update(cx, |s, cx| {
                        s.permission = AudioPermission::Granted;
                        s.devices = granted_devices();
                        cx.notify();
                    })
                })
                .on_value_change(move |id, _, cx| {
                    s2.update(cx, |s, cx| {
                        s.device_id = id;
                        cx.notify();
                    })
                })
                .value(device_id)
                .trigger(
                    AudioDeviceSelectTrigger::new()
                        .child(AudioDeviceSelectValue::new().placeholder("Select a microphone")),
                )
                .content(AudioDeviceSelectContent::new()),
        )
        .child(AudioDeviceSelectPreview::new().child(waveform))
        .into_any_element()
}

/// `audio-device-select-states.tsx`.
fn states() -> AnyElement {
    let base = id("audio-device-select", "states");
    let devices = vec![
        AudioDevice::new("default", "MacBook Pro Microphone").with_default(true),
        AudioDevice::new("usb", "Shure MV7+").with_description("USB"),
        AudioDevice::new("busy", "Elgato Wave:3")
            .with_description("In use by another app")
            .with_status(AudioDeviceStatus::Unavailable),
    ];
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(16.0))
        .child(AudioDeviceSelect::new(format!("{base}/usb"), devices.clone()).default_value("usb"))
        .child(
            AudioDeviceSelect::new(format!("{base}/none"), devices.clone())
                .allow_none(true)
                .none_label("No microphone"),
        )
        .child(AudioDeviceSelect::new(format!("{base}/rode"), devices).default_value("rode"))
        .child(AudioDeviceSelect::new(format!("{base}/loading"), []).loading(true))
        .into_any_element()
}
