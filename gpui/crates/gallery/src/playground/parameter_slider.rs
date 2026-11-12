//! Playground page for `ParameterSlider`: every prop, linear and log
//! scales, mark and format presets and every part (header, label, reset,
//! scrub input or value, control, marks, description).

use audiocn::{
    ParameterMark, ParameterScale, ParameterSlider, ParameterSliderControl,
    ParameterSliderDescription, ParameterSliderHeader, ParameterSliderInput, ParameterSliderLabel,
    ParameterSliderMarks, ParameterSliderReset, ParameterSliderValue,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "parameter-slider",
    component: "ParameterSlider",
    source: SourceUse::None,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    vec![
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::num("value", "Value", 0.0, -20_000.0, 20_000.0, 1.0).group("Value"),
        Control::num("default_value", "Default value", 0.0, -20_000.0, 20_000.0, 1.0)
            .group("Value")
            .remount(),
        Control::bool("set_reset_value", "Set reset_value", false)
            .group("Value")
            .no_code()
            .help("Off: reset goes to default_value, else min."),
        Control::num("reset_value", "Reset value", 0.0, -20_000.0, 20_000.0, 1.0).group("Value"),
        // Range
        Control::num("min", "Min", -1000.0, -20_000.0, 20_000.0, 1.0).group("Range"),
        Control::num("max", "Max", 1000.0, -20_000.0, 20_000.0, 1.0).group("Range"),
        Control::choice(
            "scale",
            "Scale",
            0,
            &[
                ("linear", "ParameterScale::Linear"),
                ("log (needs min > 0)", "ParameterScale::Log"),
            ],
        )
        .group("Range")
        .help("Log suits frequency and time; try min 20, max 20000."),
        Control::bool("set_origin", "Set origin", true)
            .group("Range")
            .no_code()
            .help("Off: the fill starts at min."),
        Control::num("origin", "Origin", 0.0, -20_000.0, 20_000.0, 1.0)
            .group("Range")
            .help("Where the fill starts; the middle of a bipolar range."),
        Control::num("step", "Step", 5.0, 0.001, 1000.0, 0.001).group("Steps"),
        Control::num("large_step", "Large step", 50.0, 0.01, 5000.0, 0.01).group("Steps"),
        // Text
        Control::text("unit", "Unit", "ms")
            .group("Text")
            .help("Suffix such as \"ms\", \"dB\" or \"Hz\" (empty = none)."),
        Control::choice(
            "decimals",
            "Decimals",
            0,
            &[
                ("from step", ""),
                ("0", "0"),
                ("1", "1"),
                ("2", "2"),
                ("3", "3"),
            ],
        )
        .group("Text"),
        Control::choice(
            "format",
            "Format",
            0,
            &[
                ("default (number + unit)", ""),
                ("Hz / kHz", "format_hz"),
                ("percent", "|v| format!(\"{:.0}%\", v)"),
                ("signed", "|v| format!(\"{v:+}\")"),
            ],
        )
        .group("Text"),
        Control::choice(
            "marks",
            "Marks",
            0,
            &[
                ("none", ""),
                (
                    "bipolar (-1000, 0, 1000)",
                    "[ParameterMark::new(-1000.0).label(\"-1s\"), ParameterMark::new(0.0).label(\"0\"), ParameterMark::new(1000.0).label(\"+1s\")]",
                ),
                (
                    "frequency (20, 200, 2k, 20k)",
                    "[ParameterMark::new(20.0).label(\"20\"), ParameterMark::new(200.0).label(\"200\"), ParameterMark::new(2000.0).label(\"2k\"), ParameterMark::new(20_000.0).label(\"20k\")]",
                ),
                (
                    "quarters, unlabelled",
                    "(0..=4).map(|i| ParameterMark::new(min + (max - min) * i as f32 / 4.0))",
                ),
            ],
        )
        .group("Text")
        .help("Shown by ParameterSliderMarks."),
        Control::bool("disabled", "Disabled", false).group("Interaction"),
        Control::num("width", "Max width (px)", 384.0, 160.0, 900.0, 8.0)
            .group("Look")
            .no_code(),
        // Parts
        Control::bool("show_header", "ParameterSliderHeader", true)
            .group("Parts")
            .no_code(),
        Control::text("label", "ParameterSliderLabel", "Sync offset")
            .group("Parts")
            .no_code()
            .help("Empty hides the label."),
        Control::bool("show_reset", "ParameterSliderReset", true)
            .group("Parts")
            .no_code(),
        Control::choice(
            "readout",
            "Readout",
            0,
            &[
                ("ParameterSliderInput", ""),
                ("ParameterSliderValue", ""),
                ("none", ""),
            ],
        )
        .group("Parts")
        .no_code(),
        Control::bool("scrub", "Input scrub", true)
            .group("Parts")
            .no_code()
            .help("Drag the unit sideways to change the value."),
        Control::bool("show_control", "ParameterSliderControl", true)
            .group("Parts")
            .no_code(),
        Control::text("control_aria_label", "Control aria label", "")
            .group("Parts")
            .no_code(),
        Control::bool("show_marks", "ParameterSliderMarks", true)
            .group("Parts")
            .no_code(),
        Control::text(
            "description",
            "ParameterSliderDescription",
            "Delays the microphone to line up with your camera.",
        )
        .group("Parts")
        .no_code()
        .help("Empty hides the description."),
    ]
}

fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        let decimals = if hz >= 10_000.0 { 0 } else { 1 };
        format!("{:.*} kHz", decimals, hz / 1000.0)
    } else {
        format!("{} Hz", (hz + 0.5).floor())
    }
}

fn marks(p: &Props) -> Vec<ParameterMark> {
    let (min, max) = (p.num("min"), p.num("max"));
    match p.index("marks") {
        1 => vec![
            ParameterMark::new(-1000.0).label("-1s"),
            ParameterMark::new(0.0).label("0"),
            ParameterMark::new(1000.0).label("+1s"),
        ],
        2 => vec![
            ParameterMark::new(20.0).label("20"),
            ParameterMark::new(200.0).label("200"),
            ParameterMark::new(2000.0).label("2k"),
            ParameterMark::new(20_000.0).label("20k"),
        ],
        3 => (0..=4)
            .map(|i| ParameterMark::new(min + (max - min) * i as f32 / 4.0))
            .collect(),
        _ => Vec::new(),
    }
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let mut slider = ParameterSlider::new(ctx.id("slider"))
        .w_full()
        .max_w(px(p.num("width")))
        .min(p.num("min"))
        .max(p.num("max"))
        .step(p.num("step"))
        .large_step(p.num("large_step"))
        .scale(match p.index("scale") {
            1 => ParameterScale::Log,
            _ => ParameterScale::Linear,
        })
        .marks(marks(p))
        .disabled(p.bool("disabled"))
        .config(ctx.config());
    let unit = p.text("unit");
    if !unit.is_empty() {
        slider = slider.unit(unit);
    }
    if p.index("decimals") > 0 {
        slider = slider.decimals(p.index("decimals") - 1);
    }
    if p.bool("set_origin") {
        slider = slider.origin(p.num("origin"));
    }
    if p.bool("set_reset_value") {
        slider = slider.reset_value(p.num("reset_value"));
    }
    slider = match p.index("format") {
        1 => slider.format(format_hz),
        2 => slider.format(|v| format!("{v:.0}%")),
        3 => slider.format(|v| format!("{v:+}")),
        _ => slider,
    };
    slider = if controlled {
        slider.value(p.num("value"))
    } else {
        slider.default_value(p.num("default_value"))
    };

    let set = ctx.clone();
    let commit = ctx.clone();
    slider = slider
        .on_value_change_with_details(move |v, details, _, cx| {
            if controlled {
                set.set_num("value", v, cx);
            }
            set.log("on_value_change", format!("{v} ({:?})", details.reason), cx);
        })
        .on_value_committed(move |v, _, cx| commit.log("on_value_committed", format!("{v}"), cx));

    if p.bool("show_header") {
        let mut header = ParameterSliderHeader::new();
        let label = p.text("label");
        if !label.is_empty() {
            header = header.child(ParameterSliderLabel::new(label));
        }
        if p.bool("show_reset") {
            header = header.child(ParameterSliderReset::new());
        }
        match p.index("readout") {
            0 => header = header.child(ParameterSliderInput::new().scrub(p.bool("scrub"))),
            1 => header = header.child(ParameterSliderValue::new()),
            _ => {}
        }
        slider = slider.child(header);
    }
    if p.bool("show_control") {
        let mut control = ParameterSliderControl::new();
        let aria = p.text("control_aria_label");
        if !aria.is_empty() {
            control = control.aria_label(aria);
        }
        slider = slider.child(control);
    }
    if p.bool("show_marks") {
        slider = slider.child(ParameterSliderMarks::new());
    }
    let description = p.text("description");
    if !description.is_empty() {
        slider = slider.child(ParameterSliderDescription::new(description));
    }
    slider.into_any_element()
}
