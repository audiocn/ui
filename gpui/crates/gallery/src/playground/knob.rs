//! Playground page for `Knob`: every prop, both dial styles, every part.

use audiocn::AudioSize;
use audiocn::components::{
    Knob, KnobCap, KnobCapVariant, KnobDial, KnobDragDirection, KnobLabel, KnobPointer, KnobRange,
    KnobScale, KnobScaleType, KnobTrack, KnobValue,
};
use gpui::{AnyElement, App, IntoElement, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "knob",
    component: "Knob",
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
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::num("value", "Value", 0.0, -60.0, 60.0, 0.1).group("Value"),
        Control::num("default_value", "Default value", 0.0, -60.0, 60.0, 0.1)
            .group("Value")
            .remount(),
        Control::num("reset_value", "Reset value", 0.0, -60.0, 60.0, 0.1).group("Value"),
        // Range
        Control::num("min", "Min", -24.0, -120.0, 0.0, 1.0).group("Range"),
        Control::num("max", "Max", 24.0, 0.0, 120.0, 1.0).group("Range"),
        Control::num("origin", "Origin", 0.0, -60.0, 60.0, 0.5)
            .group("Range")
            .help("Where the range fill starts (bipolar when inside the range)."),
        Control::choice(
            "scale",
            "Scale",
            0,
            &[
                ("linear", "KnobScaleType::Linear"),
                ("log", "KnobScaleType::Log"),
            ],
        )
        .group("Range"),
        Control::num("step", "Step", 0.1, 0.01, 10.0, 0.01).group("Steps"),
        Control::num("large_step", "Large step", 1.0, 0.1, 20.0, 0.1).group("Steps"),
        Control::num("fine_step", "Fine step", 0.01, 0.001, 1.0, 0.001).group("Steps"),
        // Interaction
        Control::choice(
            "drag_direction",
            "Drag direction",
            0,
            &[
                ("vertical", "KnobDragDirection::Vertical"),
                ("horizontal", "KnobDragDirection::Horizontal"),
                ("circular", "KnobDragDirection::Circular"),
            ],
        )
        .group("Interaction"),
        Control::num("sensitivity", "Sensitivity", 1.0, 0.1, 5.0, 0.1).group("Interaction"),
        Control::num("arc", "Arc (degrees)", 270.0, 90.0, 360.0, 5.0).group("Interaction"),
        Control::bool("allow_wheel", "Allow wheel", true).group("Interaction"),
        Control::bool("click_sound", "Click sound", false).group("Interaction"),
        Control::bool("disabled", "Disabled", false).group("Interaction"),
        // Look
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::num(
            "knob_size",
            "Knob size (px, 0 = from size)",
            0.0,
            0.0,
            320.0,
            2.0,
        )
        .group("Look"),
        Control::choice(
            "dial",
            "Dial",
            0,
            &[
                ("track + pointer", ""),
                ("scale + cap", ""),
                ("track + mini cap", ""),
            ],
        )
        .group("Look")
        .no_code(),
        Control::num("ticks", "Scale ticks", 48.0, 2.0, 120.0, 1.0)
            .group("Look")
            .no_code(),
        Control::color("range_color", "Range colour")
            .group("Look")
            .no_code(),
        Control::bool("show_value", "Show value", true)
            .group("Parts")
            .no_code(),
        Control::bool("editable", "Editable value", true)
            .group("Parts")
            .no_code(),
        Control::text("label", "Label", "Gain")
            .group("Parts")
            .no_code(),
    ]
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let mut knob = Knob::new(ctx.id("knob"))
        .min(p.num("min"))
        .max(p.num("max"))
        .step(p.num("step"))
        .large_step(p.num("large_step"))
        .fine_step(p.num("fine_step"))
        .reset_value(p.num("reset_value"))
        .origin(p.num("origin"))
        .arc(p.num("arc"))
        .sensitivity(p.num("sensitivity"))
        .allow_wheel(p.bool("allow_wheel"))
        .click_sound(p.bool("click_sound"))
        .disabled(p.bool("disabled"))
        .config(ctx.config())
        .drag_direction(match p.choice("drag_direction") {
            "horizontal" => KnobDragDirection::Horizontal,
            "circular" => KnobDragDirection::Circular,
            _ => KnobDragDirection::Vertical,
        })
        .scale(match p.choice("scale") {
            "log" => KnobScaleType::Log,
            _ => KnobScaleType::Linear,
        });
    knob = if controlled {
        knob.value(p.num("value"))
    } else {
        knob.default_value(p.num("default_value"))
    };
    knob = match p.choice("size") {
        "sm" => knob.size(AudioSize::Sm),
        "default" => knob.size(AudioSize::Default),
        "lg" => knob.size(AudioSize::Lg),
        _ => knob,
    };
    if p.num("knob_size") > 0.0 {
        knob = knob.knob_size(px(p.num("knob_size")));
    }

    let set = ctx.clone();
    let commit = ctx.clone();
    knob = knob
        .on_value_change(move |value, details, _, cx| {
            if controlled {
                set.set_num("value", value, cx);
            }
            set.log(
                "on_value_change",
                format!("{value:.3} ({:?})", details.reason),
                cx,
            );
        })
        .on_value_committed(move |value, _, cx| {
            commit.log("on_value_committed", format!("{value:.3}"), cx)
        });

    let mut range = KnobRange::new();
    if let Some(color) = p.color("range_color", cx) {
        range = range.color(color);
    }
    let dial = match p.choice("dial") {
        "scale + cap" => KnobDial::new()
            .child(KnobScale::new().ticks(p.num("ticks") as u32))
            .child(KnobCap::new()),
        "track + mini cap" => KnobDial::new()
            .child(KnobTrack::new())
            .child(range)
            .child(KnobCap::new().variant(KnobCapVariant::Mini)),
        _ => KnobDial::new()
            .child(KnobTrack::new())
            .child(range)
            .child(KnobPointer::new()),
    };
    knob = knob.child(dial);
    if p.bool("show_value") {
        knob = knob.child(KnobValue::new().editable(p.bool("editable")));
    }
    let label = p.text("label");
    if !label.is_empty() {
        knob = knob.child(KnobLabel::new(label));
    }
    knob.into_any_element()
}
