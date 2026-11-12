//! Playground page for `QuickAudioPopover`: placement, open state, the
//! device / permission seam, controlled and uncontrolled microphone, mute,
//! system audio and device, the bar source, children, and every callback.

use audiocn::QuickAudioPopover;
use audiocn::ui::{Align, Button, ButtonSize, ButtonVariant, Side};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div};

use super::mic_setup::{
    DEVICE_IDS, PERMISSIONS, REAL_AUDIO, device_id, device_index, device_set, devices_detail,
    permission, set_choice,
};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "quick-audio-popover",
    component: "QuickAudioPopover",
    source: SourceUse::Parts,
    controls,
    render,
};

/// Device lists: index 0 leaves `.devices` unset (the block's `demo_microphones()`).
const DEVICES: &[(&str, &str)] = &[
    ("unset (demo list)", ""),
    ("none (empty list)", "Vec::new()"),
    ("2 microphones", "devices"),
    ("6 (1 busy, 1 locked)", "devices"),
];

fn controls() -> Vec<Control> {
    let mut controls = vec![
        // Placement
        Control::choice(
            "side",
            "Side",
            1,
            &[
                ("top", "Side::Top"),
                ("bottom", "Side::Bottom"),
                ("left", "Side::Left"),
                ("right", "Side::Right"),
            ],
        )
        .group("Placement"),
        Control::choice(
            "align",
            "Align",
            1,
            &[
                ("start", "Align::Start"),
                ("center", "Align::Center"),
                ("end", "Align::End"),
            ],
        )
        .group("Placement"),
        // Open
        Control::bool("controlled_open", "Controlled open", false)
            .group("Open")
            .no_code()
            .remount()
            .help("On: open follows the switch below; off: default_open (remounts)."),
        Control::bool("open", "Open", true).group("Open"),
        Control::bool("default_open", "Default open", false)
            .group("Open")
            .remount(),
        // Audio seam
        Control::bool("use_source", "Use playground source", false)
            .group("Audio seam")
            .code(".source(signal.visual.clone())")
            .help("Bars read the Source panel's visual frames (while live)."),
        Control::bool("system_supported", "System audio supported", true).group("Audio seam"),
        // Devices
        Control::choice("devices", "Devices", 0, DEVICES).group("Devices"),
        Control::bool("loading", "Loading", false).group("Devices"),
        Control::choice("permission", "Permission", 0, PERMISSIONS).group("Devices"),
        // Value
        Control::bool("controlled", "Controlled", false)
            .group("Value")
            .no_code()
            .remount()
            .help("On: microphone, mute, system audio and device follow the panel."),
        Control::bool("microphone_active", "Microphone active", false).group("Value"),
        Control::bool("muted", "Muted", false).group("Value"),
        Control::bool("system_active", "System audio active", false).group("Value"),
        Control::bool("default_system_active", "Default system active", false)
            .group("Value")
            .remount(),
        Control::choice("device_id", "Device id", 0, DEVICE_IDS).group("Value"),
        // Children
        Control::bool("more_button", "\"More audio settings\" child", true)
            .group("Children")
            .no_code(),
    ];
    if REAL_AUDIO {
        controls.insert(
            0,
            Control::bool("real_audio", "Real audio", false)
                .group("Audio seam")
                .no_code()
                .remount()
                .help("Real input devices, microphone and system capture (audiocn::io)."),
        );
    }
    controls
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let controlled_open = p.bool("controlled_open");
    let mut block = QuickAudioPopover::new(ctx.id("quick-audio-popover"))
        .side(match p.choice("side") {
            "top" => Side::Top,
            "left" => Side::Left,
            "right" => Side::Right,
            _ => Side::Bottom,
        })
        .align(match p.choice("align") {
            "start" => Align::Start,
            "end" => Align::End,
            _ => Align::Center,
        })
        .loading(p.bool("loading"))
        .permission(permission(p.choice("permission")))
        .system_supported(p.bool("system_supported"))
        .default_system_active(p.bool("default_system_active"))
        .default_open(p.bool("default_open"));
    if p.index("devices") > 0 {
        block = block.devices(device_set(p.choice("devices")));
    }
    if controlled_open {
        block = block.open(p.bool("open"));
    }
    if controlled {
        block = block
            .microphone_active(p.bool("microphone_active"))
            .muted(p.bool("muted"))
            .system_active(p.bool("system_active"))
            .device_id(device_id(p.choice("device_id")));
    }
    if p.bool("use_source")
        && let Some(visual) = ctx.visual()
    {
        block = block.source(visual);
    }
    let (c_open, c_device, c_mic, c_mute, c_system, c_perm) = (
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
    );
    block = block
        .on_open_change(move |open, _, cx| {
            if controlled_open {
                c_open.set_bool("open", open, cx);
            }
            c_open.log("on_open_change", open.to_string(), cx);
        })
        .on_device_change(move |id, _, cx| {
            if controlled {
                set_choice(&c_device, "device_id", device_index(&id), cx);
            }
            c_device.log("on_device_change", devices_detail(&id), cx);
        })
        .on_microphone(move |on, _, cx| {
            if controlled {
                c_mic.set_bool("microphone_active", on, cx);
            }
            c_mic.log("on_microphone", if on { "turn on" } else { "turn off" }, cx);
        })
        .on_muted_change(move |muted, _, cx| {
            if controlled {
                c_mute.set_bool("muted", muted, cx);
            }
            c_mute.log("on_muted_change", muted.to_string(), cx);
        })
        .on_system_audio(move |on, _, cx| {
            if controlled {
                c_system.set_bool("system_active", on, cx);
            }
            c_system.log("on_system_audio", on.to_string(), cx);
        })
        .on_request_permission(move |_, cx| {
            c_perm.log("on_request_permission", "-> granted", cx);
            set_choice(&c_perm, "permission", 0, cx);
        });
    if REAL_AUDIO && p.bool("real_audio") {
        block = real::wire(block, p, ctx, window, cx);
    }
    if p.bool("more_button") {
        block = block.child(
            Button::new(ctx.id("more"))
                .w_full()
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .child("More audio settings"),
        );
    }
    // Room around the trigger so every side / align fits on the stage.
    div()
        .w_full()
        .min_h(gpui::px(420.0))
        .flex()
        .items_center()
        .justify_center()
        .child(block)
        .into_any_element()
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod real {
    use audiocn::QuickAudioPopover;
    use gpui::{App, Window};

    use super::super::{Ctx, Props};

    pub fn wire(
        block: QuickAudioPopover,
        _p: &Props,
        _ctx: &Ctx,
        _window: &mut Window,
        _cx: &mut App,
    ) -> QuickAudioPopover {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod real {
    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{
        MicrophoneOptions, MicrophoneStatus, SystemAudioOptions, SystemAudioStatus,
        use_audio_devices, use_microphone, use_system_audio,
    };
    use audiocn::{AudioDeviceKind, QuickAudioPopover};
    use gpui::{App, SharedString, Window};

    use super::super::{Ctx, Props};
    use super::{device_id, device_index, devices_detail, set_choice};

    struct Seen {
        device_id: Option<SharedString>,
        muted: bool,
        system_supported: bool,
    }

    /// `useAudioDevices`, `useMicrophone({ deviceId })`, the analyser and
    /// `useSystemAudio`, like the gallery demo. Every callback still logs.
    pub fn wire(
        block: QuickAudioPopover,
        p: &Props,
        ctx: &Ctx,
        window: &mut Window,
        cx: &mut App,
    ) -> QuickAudioPopover {
        let seen = window.use_keyed_state(ctx.id("real-seen"), cx, |_, _| Seen {
            device_id: None,
            muted: false,
            system_supported: is_supported(),
        });
        let controlled = p.bool("controlled");
        let (device, muted, system_supported) = {
            let s = seen.read(cx);
            if controlled {
                (
                    device_id(p.choice("device_id")),
                    p.bool("muted"),
                    s.system_supported,
                )
            } else {
                (s.device_id.clone(), s.muted, s.system_supported)
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
                ..Default::default()
            },
            window,
            cx,
        );
        if let Some(handle) = &microphone.handle {
            handle.set_muted(muted);
        }
        let system = use_system_audio(
            ctx.id("real-system"),
            SystemAudioOptions::default(),
            window,
            cx,
        );
        let refresh = devices.clone();
        let mic = microphone.clone();
        let capture = system.clone();
        let (s_device, s_mute) = (seen.clone(), seen);
        let (c_perm, c_device, c_mic, c_mute, c_system) = (
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
        );
        block
            .devices(devices.devices.clone())
            .loading(devices.is_loading)
            .permission(devices.permission)
            .on_request_permission(move |_, cx| {
                let granted = refresh.request_permission();
                c_perm.log("on_request_permission", format!("real: {granted}"), cx);
            })
            .device_id(device)
            .on_device_change(move |id, _, cx| {
                if controlled {
                    set_choice(&c_device, "device_id", device_index(&id), cx);
                }
                c_device.log("on_device_change", devices_detail(&id), cx);
                s_device.update(cx, |s, cx| {
                    s.device_id = id;
                    cx.notify();
                });
            })
            .microphone_active(microphone.status == MicrophoneStatus::Active)
            .on_microphone(move |on, _, cx| {
                c_mic.log("on_microphone", format!("real: {on}"), cx);
                if on {
                    mic.start(cx);
                } else {
                    mic.stop(cx);
                }
            })
            .muted(muted)
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
            .source(microphone.visual.clone())
            .system_supported(system_supported)
            .system_active(system.status() == SystemAudioStatus::Active)
            .on_system_audio(move |on, window, cx| {
                c_system.log("on_system_audio", format!("real: {on}"), cx);
                if on {
                    capture.start();
                } else {
                    capture.stop();
                }
                window.refresh();
            })
    }
}
