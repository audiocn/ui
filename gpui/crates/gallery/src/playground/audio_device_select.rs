//! Playground page for `AudioDeviceSelect`: device data sets (none / 2 / 6
//! with unavailable, permission-required and default devices and a
//! disconnected selection), both device kinds, every permission state,
//! loading, "None", trigger and content parts, and the waveform preview.

use audiocn::ui::{SelectGroup, SelectItem, SelectLabel, SelectSeparator, SelectSize};
use audiocn::{
    AudioDevice, AudioDeviceKind, AudioDeviceSelect, AudioDeviceSelectContent,
    AudioDeviceSelectPermission, AudioDeviceSelectPreview, AudioDeviceSelectTrigger,
    AudioDeviceSelectValue, AudioDeviceStatus, AudioPermission, LiveWaveform, LiveWaveformMode,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec, Value};

pub const SPEC: Spec = Spec {
    slug: "audio-device-select",
    component: "AudioDeviceSelect",
    source: SourceUse::Parts,
    controls,
    render,
};

/// Device ids the value controls can pick, in option order (index 0 = none).
/// `old-usb` is never in a data set: selecting it shows "(disconnected)".
const IDS: &[&str] = &[
    "", "default", "usb", "busy", "bt", "locked", "virtual", "old-usb",
];

const VALUE_OPTIONS: &[(&str, &str)] = &[
    ("none", "None"),
    ("default", "Some(\"default\".into())"),
    ("usb", "Some(\"usb\".into())"),
    ("busy (unavailable)", "Some(\"busy\".into())"),
    ("bt", "Some(\"bt\".into())"),
    ("locked (permission required)", "Some(\"locked\".into())"),
    ("virtual", "Some(\"virtual\".into())"),
    ("old-usb (disconnected)", "Some(\"old-usb\".into())"),
];

const DEFAULT_VALUE_OPTIONS: &[(&str, &str)] = &[
    ("unset", ""),
    ("default", "\"default\""),
    ("usb", "\"usb\""),
    ("busy (unavailable)", "\"busy\""),
    ("bt", "\"bt\""),
    ("locked (permission required)", "\"locked\""),
    ("virtual", "\"virtual\""),
    ("old-usb (disconnected)", "\"old-usb\""),
];

const PERMISSIONS: &[(&str, &str)] = &[
    ("granted", "AudioPermission::Granted"),
    ("prompt", "AudioPermission::Prompt"),
    ("denied", "AudioPermission::Denied"),
    ("unsupported", "AudioPermission::Unsupported"),
];

fn controls() -> Vec<Control> {
    vec![
        // Data
        Control::choice(
            "devices",
            "Devices",
            1,
            &[
                ("none", ""),
                ("2 (default + USB)", ""),
                ("6 (unavailable, permission-required, ...)", ""),
            ],
        )
        .group("Data")
        .no_code()
        .help("Select \"old-usb\" below to see a disconnected device."),
        Control::choice(
            "kind",
            "Device kind",
            0,
            &[
                ("audio input (microphones)", ""),
                ("audio output (speakers)", ""),
            ],
        )
        .group("Data")
        .no_code()
        .help("AudioDevice::with_kind; labels and placeholder follow."),
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::choice("value", "Value", 1, VALUE_OPTIONS).group("Value"),
        Control::choice("default_value", "Default value", 0, DEFAULT_VALUE_OPTIONS)
            .group("Value")
            .remount(),
        Control::bool("allow_none", "Allow none", false)
            .group("Value")
            .help("Adds a \"None\" item."),
        Control::text("none_label", "None label", "None").group("Value"),
        // State
        Control::choice("permission", "Permission", 0, PERMISSIONS)
            .group("State")
            .help("Unsupported is treated like denied."),
        Control::bool("request_handler", "on_request_permission", true)
            .group("State")
            .no_code()
            .help("Shows \"Allow access\" while permission is prompt."),
        Control::bool("grant_on_request", "Grant on request", true)
            .group("State")
            .no_code()
            .help("Allow access switches permission to granted."),
        Control::bool("loading", "Loading", false)
            .group("State")
            .help("Shows \"Finding devices…\"."),
        Control::bool("disabled", "Disabled", false).group("State"),
        // Trigger
        Control::choice(
            "trigger_size",
            "Trigger size",
            0,
            &[("default", "SelectSize::Default"), ("sm", "SelectSize::Sm")],
        )
        .group("Trigger")
        .code(".trigger(AudioDeviceSelectTrigger::new().size({}))"),
        Control::text("placeholder", "Placeholder", "")
            .group("Trigger")
            .code(".trigger(AudioDeviceSelectTrigger::new().child(AudioDeviceSelectValue::new().placeholder({})))")
            .help("Empty = \"Select a microphone\" / \"Select a speaker\"."),
        Control::num("width", "Max width (px)", 384.0, 160.0, 720.0, 8.0)
            .group("Trigger")
            .no_code(),
        // Content
        Control::choice(
            "content",
            "Content",
            0,
            &[
                ("default (AudioDeviceSelectContent::new)", ""),
                ("custom (label, items, separator)", ""),
            ],
        )
        .group("Content")
        .no_code(),
        Control::text("permission_message", "Permission message", "")
            .group("Content")
            .no_code()
            .help("Custom content only; empty = default message."),
        Control::text("action_label", "Permission action label", "Allow access")
            .group("Content")
            .no_code()
            .help("Custom content only."),
        // Preview
        Control::bool("preview", "AudioDeviceSelectPreview", true)
            .group("Preview")
            .no_code()
            .help("A live waveform from the source picker, active while a device is picked."),
    ]
}

fn devices(p: &Props) -> Vec<AudioDevice> {
    let output = p.index("kind") == 1;
    let kind = if output {
        AudioDeviceKind::AudioOutput
    } else {
        AudioDeviceKind::AudioInput
    };
    let pick = |mic: &'static str, speaker: &'static str| if output { speaker } else { mic };
    let two = vec![
        AudioDevice::new(
            "default",
            pick("MacBook Pro Microphone", "MacBook Pro Speakers"),
        )
        .with_default(true),
        AudioDevice::new("usb", pick("Shure MV7+", "Focusrite Scarlett 2i2"))
            .with_description("USB"),
    ];
    let list = match p.index("devices") {
        0 => Vec::new(),
        1 => two,
        _ => {
            let mut six = two;
            six[0] = six[0].clone().with_description("Built-in");
            six.extend([
                AudioDevice::new("busy", pick("Elgato Wave:3", "LG UltraFine Display"))
                    .with_description("In use by another app")
                    .with_status(AudioDeviceStatus::Unavailable),
                AudioDevice::new("bt", "AirPods Pro")
                    .with_description("Bluetooth")
                    .with_group_id("airpods"),
                AudioDevice::new(
                    "locked",
                    pick("Studio Display Microphone", "Studio Display Speakers"),
                )
                .with_description("Needs permission")
                .with_status(AudioDeviceStatus::PermissionRequired),
                AudioDevice::new("virtual", "BlackHole 2ch").with_description("Virtual"),
            ]);
            six
        }
    };
    list.into_iter().map(|d| d.with_kind(kind)).collect()
}

fn permission(p: &Props) -> AudioPermission {
    match p.index("permission") {
        1 => AudioPermission::Prompt,
        2 => AudioPermission::Denied,
        3 => AudioPermission::Unsupported,
        _ => AudioPermission::Granted,
    }
}

/// Custom popup content. `AudioDeviceSelectItem` can't be passed to
/// `AudioDeviceSelectContent::child` (no `SelectNode` conversion), so the
/// rows are plain `SelectItem`s.
fn custom_content(
    p: &Props,
    devices: &[AudioDevice],
    on_request: Option<impl Fn(&mut Window, &mut App) + 'static>,
) -> AudioDeviceSelectContent {
    let mut content = AudioDeviceSelectContent::new();
    let output = p.index("kind") == 1;
    let permission = permission(p);
    if permission != AudioPermission::Granted {
        let mut notice = AudioDeviceSelectPermission::new()
            .permission(permission)
            .action_label(p.text("action_label"));
        let message = p.text("permission_message");
        if !message.is_empty() {
            notice = notice.message(message);
        }
        if let Some(on_request) = on_request {
            notice = notice.on_request_permission(on_request);
        }
        content = content.child(notice.into_any_element());
    }
    let mut group = SelectGroup::new().child(SelectLabel::new(if output {
        "Speakers"
    } else {
        "Microphones"
    }));
    if p.bool("allow_none") {
        group = group.child(SelectItem::new("__none__", p.text("none_label")));
    }
    for d in devices {
        let label: SharedString = match (&d.description, d.is_default) {
            (Some(desc), true) => format!("{} · Default · {desc}", d.label).into(),
            (Some(desc), false) => format!("{} · {desc}", d.label).into(),
            (None, true) => format!("{} · Default", d.label).into(),
            (None, false) => d.label.clone(),
        };
        group = group.child(
            SelectItem::new(d.id.clone(), label).disabled(d.status != AudioDeviceStatus::Available),
        );
    }
    content
        .child(group)
        .child(SelectSeparator::new())
        .child(SelectItem::new("old-usb", "Old USB mic (custom row)").disabled(true))
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let devices = devices(p);
    let output = p.index("kind") == 1;

    let mut select = AudioDeviceSelect::new(ctx.id("select"), devices.clone())
        .allow_none(p.bool("allow_none"))
        .none_label(p.text("none_label"))
        .loading(p.bool("loading"))
        .permission(permission(p))
        .disabled(p.bool("disabled"));
    let selected: Option<SharedString> = if controlled {
        IDS.get(p.index("value"))
            .filter(|id| !id.is_empty())
            .map(|id| SharedString::from(*id))
    } else {
        None
    };
    select = if controlled {
        select.value(selected.clone())
    } else {
        match IDS.get(p.index("default_value")) {
            Some(id) if !id.is_empty() => select.default_value(*id),
            _ => select,
        }
    };

    let set = ctx.clone();
    select = select.on_value_change(move |id, _, cx| {
        if controlled {
            let index = id
                .as_ref()
                .and_then(|id| IDS.iter().position(|known| *known == id.as_ref()))
                .unwrap_or(0);
            set.set("value", Value::Choice(index), cx);
        }
        set.log("on_value_change", format!("{id:?}"), cx);
    });

    let request = p.bool("request_handler").then(|| {
        let c = ctx.clone();
        let grant = p.bool("grant_on_request");
        move |_: &mut Window, cx: &mut App| {
            c.log(
                "on_request_permission",
                if grant { "granted" } else { "(no change)" },
                cx,
            );
            if grant {
                c.set("permission", Value::Choice(0), cx);
            }
        }
    });
    if let Some(request) = request.clone() {
        select = select.on_request_permission(request);
    }

    let placeholder = p.text("placeholder");
    let placeholder = if !placeholder.is_empty() {
        placeholder
    } else if output {
        "Select a speaker".into()
    } else {
        "Select a microphone".into()
    };
    select = select.trigger(
        AudioDeviceSelectTrigger::new()
            .size(if p.choice("trigger_size") == "sm" {
                SelectSize::Sm
            } else {
                SelectSize::Default
            })
            .child(AudioDeviceSelectValue::new().placeholder(placeholder)),
    );
    select = select.content(if p.index("content") == 1 {
        custom_content(p, &devices, request)
    } else {
        AudioDeviceSelectContent::new()
    });

    let mut root = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(p.num("width")))
        .gap(px(8.0))
        .child(select);
    if p.bool("preview") {
        // Uncontrolled selections aren't visible here, so the preview runs.
        let active = !controlled || selected.is_some();
        let mut waveform = LiveWaveform::new(ctx.id("preview"))
            .active(active)
            .bar_width(2.0)
            .mode(LiveWaveformMode::Scrolling)
            .h(px(32.0));
        if let Some(visual) = ctx.visual() {
            waveform = waveform.source(visual);
        }
        root = root.child(AudioDeviceSelectPreview::new().child(waveform));
    }
    root.into_any_element()
}
