//! Playground page for `PanControl`: every prop, controlled and
//! uncontrolled, format presets, the demo's L / R frame, and the docs' knob
//! alternative (`pan-control-knob.tsx`).

use audiocn::theme::ActiveTheme;
use audiocn::{
    AudioSize, Knob, KnobDial, KnobLabel, KnobPointer, KnobRange, KnobTrack, KnobValue, PanControl,
    describe_pan, format_pan, parse_pan,
};
use gpui::{AnyElement, App, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "pan-control",
    component: "PanControl",
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
            "render_as",
            "Render as",
            0,
            &[("PanControl", ""), ("Knob (pan-control-knob)", "")],
        )
        .group("Component")
        .no_code()
        .help("The docs' knob alternative: Knob with format_pan / parse_pan."),
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::num("value", "Value", -0.3, -1.0, 1.0, 0.01)
            .group("Value")
            .help("-1 (left) to 1 (right)."),
        Control::num("default_value", "Default value", 0.0, -1.0, 1.0, 0.01)
            .group("Value")
            .remount(),
        // Steps
        Control::num("step", "Step", 0.05, 0.001, 0.5, 0.001).group("Steps"),
        Control::num("large_step", "Large step", 0.25, 0.01, 1.0, 0.01)
            .group("Steps")
            .help("Shift+arrow and Page Up/Down."),
        Control::bool("detent", "Centre detent", true)
            .group("Steps")
            .help("Snap to the centre while dragging near it."),
        // Look
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::bool("disabled", "Disabled", false).group("Look"),
        Control::choice(
            "format",
            "Format",
            0,
            &[
                ("format_pan (L30 / C / R30)", ""),
                ("describe_pan (30% left)", "describe_pan"),
                ("percent (-100..100)", "|v| format!(\"{:+.0}\", v * 100.0)"),
            ],
        )
        .group("Look")
        .help("Assistive text; shown in the frame's readout here."),
        Control::num("width", "Max width (px)", 320.0, 120.0, 720.0, 8.0)
            .group("Look")
            .no_code(),
        // Parts
        Control::bool("frame", "Label + readout + L/R", true)
            .group("Parts")
            .no_code()
            .help("The pan-control-demo frame around the control."),
    ]
}

fn formatter(p: &Props) -> fn(f32) -> String {
    match p.index("format") {
        1 => describe_pan,
        2 => |v| format!("{:+.0}", v * 100.0),
        _ => format_pan,
    }
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let format = formatter(p);
    let set = ctx.clone();
    let commit = ctx.clone();
    let on_change = move |v: f32, cx: &mut App| {
        if controlled {
            set.set_num("value", v, cx);
        }
        set.log("on_value_change", format!("{v:.3} ({})", format(v)), cx);
    };
    let on_commit = move |v: f32, cx: &mut App| {
        commit.log("on_value_committed", format!("{v:.3} ({})", format(v)), cx)
    };

    if p.index("render_as") == 1 {
        let mut knob = Knob::new(ctx.id("knob"))
            .format(format)
            .parse(parse_pan)
            .min(-1.0)
            .max(1.0)
            .origin(0.0)
            .reset_value(0.0)
            .step(p.num("step"))
            .large_step(p.num("large_step"))
            .disabled(p.bool("disabled"))
            .config(ctx.config())
            .on_value_change(move |v, _, _, cx| on_change(v, cx))
            .on_value_committed(move |v, _, cx| on_commit(v, cx));
        knob = match p.choice("size") {
            "sm" => knob.size(AudioSize::Sm),
            "default" => knob.size(AudioSize::Default),
            "lg" => knob.size(AudioSize::Lg),
            _ => knob,
        };
        knob = if controlled {
            knob.value(p.num("value"))
        } else {
            knob.default_value(p.num("default_value"))
        };
        return knob
            .child(
                KnobDial::new()
                    .child(KnobTrack::new())
                    .child(KnobRange::new())
                    .child(KnobPointer::new()),
            )
            .child(KnobValue::new())
            .child(KnobLabel::new("Pan"))
            .into_any_element();
    }

    let mut pan = PanControl::new(ctx.id("pan"))
        .step(p.num("step"))
        .large_step(p.num("large_step"))
        .detent(p.bool("detent"))
        .format(format)
        .disabled(p.bool("disabled"))
        .config(ctx.config())
        .on_value_change(move |v, _, cx| on_change(v, cx))
        .on_value_committed(move |v, _, cx| on_commit(v, cx));
    pan = match p.choice("size") {
        "sm" => pan.size(AudioSize::Sm),
        "default" => pan.size(AudioSize::Default),
        "lg" => pan.size(AudioSize::Lg),
        _ => pan,
    };
    pan = if controlled {
        pan.value(p.num("value"))
    } else {
        pan.default_value(p.num("default_value"))
    };

    let width = px(p.num("width"));
    if !p.bool("frame") {
        return div().w_full().max_w(width).child(pan).into_any_element();
    }
    // `pan-control-demo.tsx`.
    let theme = cx.theme();
    let readout = if controlled {
        format(p.num("value"))
    } else {
        "(uncontrolled)".to_string()
    };
    div()
        .grid()
        .w_full()
        .max_w(width)
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .child(div().font_weight(FontWeight::MEDIUM).child("Pan"))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .font_family(theme.font_mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(readout),
                ),
        )
        .child(pan)
        .child(
            div()
                .flex()
                .justify_between()
                .text_color(theme.muted_foreground)
                .text_size(px(12.0))
                .line_height(px(16.0))
                .child("L")
                .child("R"),
        )
        .into_any_element()
}
