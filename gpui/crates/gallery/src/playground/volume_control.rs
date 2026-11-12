//! Playground page for `VolumeControl`: every prop, controlled and
//! uncontrolled value and mute, both curves, every part (mute with text,
//! fixed icon or level icon, slider, value) and the popover variant.

use audiocn::ui::icons::{Icon, IconName};
use audiocn::ui::{Button, ButtonSize, ButtonVariant, Popover, PopoverContent, Side};
use audiocn::{
    AudioSize, Orientation, VolumeControl, VolumeControlMute, VolumeControlSlider,
    VolumeControlValue, VolumeCurve, VolumeLevel,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "volume-control",
    component: "VolumeControl",
    source: SourceUse::None,
    controls,
    render,
};

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice(
            "presentation",
            "Presentation",
            0,
            &[("inline", ""), ("popover (volume-control-popover)", "")],
        )
        .group("Component")
        .no_code()
        .help("Popover: a speaker button opening the control."),
        Control::bool("popover_open", "Popover default open", true)
            .group("Component")
            .no_code()
            .remount(),
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled default_value / default_muted (remounts)."),
        Control::num("value", "Value", 0.6, 0.0, 1.0, 0.01)
            .group("Value")
            .help("0..1, like HTMLMediaElement.volume."),
        Control::num("default_value", "Default value", 1.0, 0.0, 1.0, 0.01)
            .group("Value")
            .remount(),
        Control::bool("muted", "Muted", false).group("Value"),
        Control::bool("default_muted", "Default muted", false)
            .group("Value")
            .remount(),
        // Behaviour
        Control::num("step", "Step", 0.05, 0.001, 0.5, 0.001)
            .group("Behaviour")
            .help("Slider step, in position."),
        Control::choice(
            "curve",
            "Curve",
            1,
            &[
                ("linear", "VolumeCurve::Linear"),
                ("perceptual", "VolumeCurve::Perceptual"),
            ],
        )
        .group("Behaviour")
        .help("Perceptual: position = √volume."),
        Control::bool("disabled", "Disabled", false).group("Behaviour"),
        // Look
        Control::choice(
            "orientation",
            "Orientation",
            0,
            &[
                ("horizontal", "Orientation::Horizontal"),
                ("vertical", "Orientation::Vertical"),
            ],
        )
        .group("Look")
        .help("Vertical stacks the slider above the mute button."),
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::num("length", "Length (px)", 320.0, 80.0, 720.0, 8.0)
            .group("Look")
            .no_code()
            .help("Inline only: max width when horizontal, height when vertical."),
        // Parts
        Control::bool("parts", "Compose parts", true)
            .group("Parts")
            .no_code()
            .help("Off: no children (the control's default parts)."),
        Control::bool("show_mute", "VolumeControlMute", true)
            .group("Parts")
            .no_code(),
        Control::choice(
            "mute_content",
            "Mute content",
            0,
            &[
                ("icon from level (content)", ""),
                ("fixed speaker icon", ""),
                ("default text (Mute / Unmute)", ""),
            ],
        )
        .group("Parts")
        .no_code(),
        Control::bool("show_slider", "VolumeControlSlider", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_value", "VolumeControlValue", true)
            .group("Parts")
            .no_code(),
    ]
}

fn level_icon(level: VolumeLevel) -> AnyElement {
    Icon::new(match level {
        VolumeLevel::Muted => IconName::SpeakerX,
        VolumeLevel::Low => IconName::SpeakerNone,
        VolumeLevel::Medium => IconName::SpeakerLow,
        VolumeLevel::High => IconName::SpeakerHigh,
    })
    .into_any_element()
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let vertical = p.choice("orientation") == "vertical";
    let mut control = VolumeControl::new(ctx.id("volume"))
        .step(p.num("step"))
        .curve(match p.choice("curve") {
            "linear" => VolumeCurve::Linear,
            _ => VolumeCurve::Perceptual,
        })
        .orientation(if vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        })
        .disabled(p.bool("disabled"))
        .config(ctx.config());
    control = match p.choice("size") {
        "sm" => control.size(AudioSize::Sm),
        "default" => control.size(AudioSize::Default),
        "lg" => control.size(AudioSize::Lg),
        _ => control,
    };
    control = if controlled {
        control.value(p.num("value")).muted(p.bool("muted"))
    } else {
        control
            .default_value(p.num("default_value"))
            .default_muted(p.bool("default_muted"))
    };

    let (set, commit, mute) = (ctx.clone(), ctx.clone(), ctx.clone());
    control = control
        .on_value_change(move |v, _, cx| {
            if controlled {
                set.set_num("value", v, cx);
            }
            set.log("on_value_change", format!("{v:.3}"), cx);
        })
        .on_value_committed(move |v, _, cx| commit.log("on_value_committed", format!("{v:.3}"), cx))
        .on_muted_change(move |m, _, cx| {
            if controlled {
                mute.set_bool("muted", m, cx);
            }
            mute.log("on_muted_change", m.to_string(), cx);
        });

    let length = px(p.num("length"));
    let inline = p.index("presentation") == 0;
    control = if !inline {
        // The popover sizes to the docs example's content.
        control
    } else if vertical {
        control.h(length)
    } else {
        control.w_full().max_w(length)
    };

    if p.bool("parts") {
        if p.bool("show_mute") {
            let mute = VolumeControlMute::new();
            control = control.child(match p.index("mute_content") {
                0 => mute.content(|state| level_icon(state.level)),
                1 => {
                    let silent = p.bool("muted") || p.num("value") == 0.0;
                    mute.child(Icon::new(if controlled && silent {
                        IconName::SpeakerX
                    } else {
                        IconName::SpeakerHigh
                    }))
                }
                _ => mute,
            });
        }
        if p.bool("show_slider") {
            control = control.child(VolumeControlSlider::new());
        }
        if p.bool("show_value") {
            control = control.child(VolumeControlValue::new());
        }
    }

    if p.index("presentation") == 0 {
        return control.into_any_element();
    }
    // `volume-control-popover.tsx`.
    let silent = controlled && (p.bool("muted") || p.num("value") == 0.0);
    let open = ctx.clone();
    Popover::new(ctx.id("popover"))
        .default_open(p.bool("popover_open"))
        .on_open_change(move |o, _, cx| open.log("popover.on_open_change", o.to_string(), cx))
        .trigger(
            Button::new(ctx.id("trigger"))
                .aria_label("Volume")
                .size(ButtonSize::Icon)
                .variant(ButtonVariant::Outline)
                .icon(if silent {
                    IconName::SpeakerX
                } else {
                    IconName::SpeakerHigh
                }),
        )
        .content(
            PopoverContent::new()
                .w_auto()
                .side(Side::Top)
                .child(control),
        )
        .into_any_element()
}
