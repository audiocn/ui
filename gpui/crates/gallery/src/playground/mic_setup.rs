//! Playground page for `MicSetup`: the whole audio seam (devices, permission,
//! capture status, analyser sources), controlled and uncontrolled device /
//! gain / mute, and every callback. Also home of the device data sets the
//! other block pages share.

use std::time::Duration;

use audiocn::{AudioDevice, AudioDeviceStatus, AudioPermission, MicSetup, MicrophoneStatus};
use gpui::{AnyElement, App, IntoElement, SharedString, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec, Value};

pub const SPEC: Spec = Spec {
    slug: "mic-setup",
    component: "MicSetup",
    source: SourceUse::Parts,
    controls,
    render,
};

/// Whether this build can open real devices (gallery `device` feature, native).
pub(super) const REAL_AUDIO: bool = cfg!(all(feature = "device", not(target_family = "wasm")));

// ----- shared data sets ------------------------------------------------------

/// Device lists for `.devices(..)`. The expression is a placeholder variable.
pub(super) const DEVICE_SETS: &[(&str, &str)] = &[
    ("none (empty list)", "Vec::new()"),
    ("2 microphones", "devices"),
    ("6 (1 busy, 1 locked)", "devices"),
];

pub(super) fn device_set(label: &str) -> Vec<AudioDevice> {
    let two = vec![
        AudioDevice::new("default", "MacBook Pro Microphone").with_default(true),
        AudioDevice::new("usb", "Shure MV7+").with_description("USB"),
    ];
    match label {
        "2 microphones" => two,
        "6 (1 busy, 1 locked)" => {
            let mut all = two;
            all.extend([
                AudioDevice::new("interface", "Scarlett 2i2 USB")
                    .with_description("Audio interface"),
                AudioDevice::new("airpods", "AirPods Pro").with_description("Bluetooth"),
                AudioDevice::new("webcam", "Logitech C920")
                    .with_description("In use by another app")
                    .with_status(AudioDeviceStatus::Unavailable),
                AudioDevice::new("display", "Studio Display Microphone")
                    .with_status(AudioDeviceStatus::PermissionRequired),
            ]);
            all
        }
        _ => Vec::new(),
    }
}

/// Selected-device choices: the ids of [`device_set`], plus one that is not listed.
pub(super) const DEVICE_IDS: &[(&str, &str)] = &[
    ("none", "None"),
    ("default", "Some(\"default\".into())"),
    ("usb", "Some(\"usb\".into())"),
    ("interface", "Some(\"interface\".into())"),
    ("airpods", "Some(\"airpods\".into())"),
    ("webcam", "Some(\"webcam\".into())"),
    ("display", "Some(\"display\".into())"),
    ("gone (not listed)", "Some(\"gone\".into())"),
];

pub(super) fn device_id(label: &str) -> Option<SharedString> {
    match label {
        "none" | "" => None,
        "gone (not listed)" => Some("gone".into()),
        id => Some(SharedString::from(id.to_string())),
    }
}

/// The [`DEVICE_IDS`] index of an id (unknown ids map to "gone").
pub(super) fn device_index(id: &Option<SharedString>) -> usize {
    match id {
        None => 0,
        Some(id) => DEVICE_IDS
            .iter()
            .position(|(label, _)| *label == id.as_ref())
            .unwrap_or(DEVICE_IDS.len() - 1),
    }
}

pub(super) fn devices_detail(id: &Option<SharedString>) -> String {
    match id {
        Some(id) => format!("{id:?}"),
        None => "None".into(),
    }
}

pub(super) const PERMISSIONS: &[(&str, &str)] = &[
    ("granted", "AudioPermission::Granted"),
    ("prompt", "AudioPermission::Prompt"),
    ("denied", "AudioPermission::Denied"),
    ("unsupported", "AudioPermission::Unsupported"),
];

pub(super) fn permission(label: &str) -> AudioPermission {
    match label {
        "prompt" => AudioPermission::Prompt,
        "denied" => AudioPermission::Denied,
        "unsupported" => AudioPermission::Unsupported,
        _ => AudioPermission::Granted,
    }
}

/// `MicrophoneStatus` choices; index 0 leaves the prop unset (the block's demo).
pub(super) const MIC_STATUSES: &[(&str, &str)] = &[
    ("unset (block demo)", ""),
    ("idle", "MicrophoneStatus::Idle"),
    ("acquiring", "MicrophoneStatus::Acquiring"),
    ("active", "MicrophoneStatus::Active"),
    ("denied", "MicrophoneStatus::Denied"),
    ("unavailable", "MicrophoneStatus::Unavailable"),
    ("error", "MicrophoneStatus::Error"),
];
pub(super) const MIC_IDLE: usize = 1;
pub(super) const MIC_ACQUIRING: usize = 2;
pub(super) const MIC_ACTIVE: usize = 3;

pub(super) fn mic_status(label: &str) -> Option<MicrophoneStatus> {
    Some(match label {
        "idle" => MicrophoneStatus::Idle,
        "acquiring" => MicrophoneStatus::Acquiring,
        "active" => MicrophoneStatus::Active,
        "denied" => MicrophoneStatus::Denied,
        "unavailable" => MicrophoneStatus::Unavailable,
        "error" => MicrophoneStatus::Error,
        _ => return None,
    })
}

/// Runs `f` after `ms` milliseconds (a host whose capture takes a moment).
pub(super) fn later(cx: &mut App, ms: u64, f: impl FnOnce(&mut App) + 'static) {
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(ms))
            .await;
        cx.update(f);
    })
    .detach();
}

/// Sets a choice prop by index.
pub(super) fn set_choice(ctx: &Ctx, key: &'static str, index: usize, cx: &mut App) {
    ctx.set(key, Value::Choice(index), cx);
}

// ----- page ------------------------------------------------------------------

fn controls() -> Vec<Control> {
    let mut controls = vec![
        // Audio seam
        Control::choice("status", "Status", 0, MIC_STATUSES)
            .group("Audio seam")
            .help("Unset: idle until \"Turn on microphone\", then the block's speech demo."),
        Control::bool("use_source", "Use playground source", false)
            .group("Audio seam")
            .code(".meter_source(signal.meter.clone())\n    .visual_source(signal.visual.clone())")
            .help("meter_source / visual_source from the Source panel (needs a status)."),
        Control::bool("simulate_host", "Simulate host", true)
            .group("Audio seam")
            .no_code()
            .help("With a status set, on_start moves it acquiring -> active."),
        Control::bool("auto_start", "Auto start", false)
            .group("Audio seam")
            .remount(),
        // Devices
        Control::choice("devices", "Devices", 2, DEVICE_SETS).group("Devices"),
        Control::bool("loading", "Loading", false).group("Devices"),
        Control::choice("permission", "Permission", 0, PERMISSIONS)
            .group("Devices")
            .help("Prompt shows the request button; unsupported reads as denied."),
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: the block keeps device, gain and mute itself (remounts)."),
        Control::choice("device_id", "Device id", 0, DEVICE_IDS).group("Value"),
        Control::num("gain_db", "Gain (dB)", 0.0, -24.0, 24.0, 0.5).group("Value"),
        Control::bool("muted", "Muted", false).group("Value"),
        // Layout
        Control::px("max_width", "Max width (px)", 448.0, 280.0, 900.0, 8.0)
            .group("Layout")
            .method("max_w"),
    ];
    if REAL_AUDIO {
        controls.insert(
            0,
            Control::bool("real_audio", "Real audio", false)
                .group("Audio seam")
                .no_code()
                .remount()
                .help("Real input devices, permission, status and analyser (audiocn::io)."),
        );
    }
    controls
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let mut block = MicSetup::new(ctx.id("mic-setup"))
        .auto_start(p.bool("auto_start"))
        .devices(device_set(p.choice("devices")))
        .loading(p.bool("loading"))
        .permission(permission(p.choice("permission")));
    let status = mic_status(p.choice("status"));
    if let Some(status) = status {
        block = block.status(status);
    }
    if p.bool("use_source") {
        if let Some(meter) = ctx.meter() {
            block = block.meter_source(meter);
        }
        if let Some(visual) = ctx.visual() {
            block = block.visual_source(visual);
        }
    }
    if controlled {
        block = block
            .device_id(device_id(p.choice("device_id")))
            .gain_db(p.num("gain_db"))
            .muted(p.bool("muted"));
    }

    let (c_device, c_gain, c_mute, c_perm, c_start) = (
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
    );
    let simulate = p.bool("simulate_host") && status.is_some();
    block = block
        .on_device_change(move |id, _, cx| {
            if controlled {
                set_choice(&c_device, "device_id", device_index(&id), cx);
            }
            c_device.log("on_device_change", devices_detail(&id), cx);
        })
        .on_gain_change(move |db, _, cx| {
            if controlled {
                c_gain.set_num("gain_db", db, cx);
            }
            c_gain.log("on_gain_change", format!("{db:.1} dB"), cx);
        })
        .on_muted_change(move |muted, _, cx| {
            if controlled {
                c_mute.set_bool("muted", muted, cx);
            }
            c_mute.log("on_muted_change", muted.to_string(), cx);
        })
        .on_request_permission(move |_, cx| {
            c_perm.log("on_request_permission", "-> granted", cx);
            set_choice(&c_perm, "permission", 0, cx);
        })
        .on_start(move |_, cx| {
            c_start.log("on_start", "open the microphone", cx);
            if simulate {
                set_choice(&c_start, "status", MIC_ACQUIRING, cx);
                let c = c_start.clone();
                later(cx, 700, move |cx| set_choice(&c, "status", MIC_ACTIVE, cx));
            }
        });
    if REAL_AUDIO && p.bool("real_audio") {
        block = real::wire(block, p, ctx, window, cx);
    }
    block
        .w_full()
        .max_w(px(p.num("max_width")))
        .into_any_element()
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod real {
    use audiocn::MicSetup;
    use gpui::{App, Window};

    use super::super::{Ctx, Props};

    pub fn wire(
        block: MicSetup,
        _p: &Props,
        _ctx: &Ctx,
        _window: &mut Window,
        _cx: &mut App,
    ) -> MicSetup {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod real {
    use audiocn::io::{AnalyserTapOptions, MicrophoneOptions, use_audio_devices, use_microphone};
    use audiocn::{AudioDeviceKind, MicSetup};
    use gpui::{App, SharedString, Window};

    use super::super::{Ctx, Props};
    use super::{device_id, devices_detail};

    /// What the block reported, for the uncontrolled case.
    struct Seen {
        started: bool,
        device_id: Option<SharedString>,
        gain_db: f32,
        muted: bool,
    }

    /// `useAudioDevices` + `useMicrophone({ deviceId, enabled: started })` +
    /// the gain node in front of the analyser, like the gallery demo. The
    /// panel's callbacks still log; these wrap them.
    pub fn wire(
        block: MicSetup,
        p: &Props,
        ctx: &Ctx,
        window: &mut Window,
        cx: &mut App,
    ) -> MicSetup {
        let seen = window.use_keyed_state(ctx.id("real-seen"), cx, |_, _| Seen {
            started: false,
            device_id: None,
            gain_db: 0.0,
            muted: false,
        });
        let controlled = p.bool("controlled");
        let (started, device, gain_db, muted) = {
            let s = seen.read(cx);
            if controlled {
                (
                    s.started,
                    device_id(p.choice("device_id")),
                    p.num("gain_db"),
                    p.bool("muted"),
                )
            } else {
                (s.started, s.device_id.clone(), s.gain_db, s.muted)
            }
        };
        let devices = use_audio_devices(
            ctx.id("real-devices"),
            AudioDeviceKind::AudioInput,
            window,
            cx,
        );
        let microphone = use_microphone(
            ctx.id("real-microphone"),
            MicrophoneOptions {
                device_id: device.as_ref().map(ToString::to_string),
                enabled: started,
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
        let log = ctx.clone();
        let (s_device, s_gain, s_mute, s_start) = (seen.clone(), seen.clone(), seen.clone(), seen);
        let (c_device, c_gain, c_mute, c_start) =
            (ctx.clone(), ctx.clone(), ctx.clone(), ctx.clone());
        block
            .devices(devices.devices.clone())
            .loading(devices.is_loading)
            .permission(devices.permission)
            .on_request_permission(move |_, cx| {
                let granted = refresh.request_permission();
                log.log("on_request_permission", format!("real: {granted}"), cx);
            })
            .status(microphone.status.into())
            .meter_source(microphone.meter.clone())
            .visual_source(microphone.visual.clone())
            .on_device_change(move |id, _, cx| {
                if controlled {
                    super::set_choice(&c_device, "device_id", super::device_index(&id), cx);
                }
                c_device.log("on_device_change", devices_detail(&id), cx);
                s_device.update(cx, |s, cx| {
                    s.device_id = id;
                    cx.notify();
                });
            })
            .on_gain_change(move |db, _, cx| {
                if controlled {
                    c_gain.set_num("gain_db", db, cx);
                }
                c_gain.log("on_gain_change", format!("{db:.1} dB"), cx);
                s_gain.update(cx, |s, cx| {
                    s.gain_db = db;
                    cx.notify();
                });
            })
            .on_muted_change(move |muted, _, cx| {
                if controlled {
                    c_mute.set_bool("muted", muted, cx);
                }
                c_mute.log("on_muted_change", muted.to_string(), cx);
                s_mute.update(cx, |s, cx| {
                    s.muted = muted;
                    cx.notify();
                });
            })
            .on_start(move |_, cx| {
                c_start.log("on_start", "real: opening the microphone", cx);
                s_start.update(cx, |s, cx| {
                    s.started = true;
                    cx.notify();
                });
            })
    }
}
