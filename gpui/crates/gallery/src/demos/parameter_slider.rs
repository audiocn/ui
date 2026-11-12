//! Gallery page for `parameter-slider`. One variant per docs example:
//! `components/examples/parameter-slider-demo.tsx`,
//! `components/examples/parameter-slider-frequency.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, px};

use audiocn::{
    ParameterMark, ParameterScale, ParameterSlider, ParameterSliderControl,
    ParameterSliderDescription, ParameterSliderHeader, ParameterSliderInput, ParameterSliderLabel,
    ParameterSliderMarks, ParameterSliderReset, ParameterSliderValue,
};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "parameter-slider",
    title: "Parameter Slider",
    description: "A labelled slider with a numeric input, unit, marks and reset, for everything that is not a volume.",
    section: Section::Components,
    variants: &["default", "frequency"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "frequency" => frequency(),
        _ => return None,
    })
}

/// `parameter-slider-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state("parameter-slider/default/state", cx, |_, _| 0.0_f32);
    let offset_ms = *state.read(cx);
    ParameterSlider::new(id("parameter-slider", "default"))
        .max_w(px(384.0))
        .large_step(50.0)
        .max(1000.0)
        .min(-1000.0)
        .on_value_change(move |v, _, cx| {
            state.update(cx, |s, cx| {
                *s = v;
                cx.notify();
            })
        })
        .origin(0.0)
        .step(5.0)
        .unit("ms")
        .value(offset_ms)
        .child(
            ParameterSliderHeader::new()
                .child(ParameterSliderLabel::new("Sync offset"))
                .child(ParameterSliderReset::new())
                .child(ParameterSliderInput::new()),
        )
        .child(ParameterSliderControl::new())
        .child(ParameterSliderDescription::new(
            "Delays the microphone to line up with your camera.",
        ))
        .into_any_element()
}

fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        let decimals = if hz >= 10_000.0 { 0 } else { 1 };
        format!("{:.*} kHz", decimals, hz / 1000.0)
    } else {
        format!("{} Hz", (hz + 0.5).floor())
    }
}

/// `parameter-slider-frequency.tsx`.
fn frequency() -> AnyElement {
    let marks = [
        ParameterMark::new(20.0).label("20"),
        ParameterMark::new(200.0).label("200"),
        ParameterMark::new(2000.0).label("2k"),
        ParameterMark::new(20_000.0).label("20k"),
    ];
    ParameterSlider::new(id("parameter-slider", "frequency"))
        .max_w(px(384.0))
        .default_value(1000.0)
        .format(format_hz)
        .marks(marks)
        .max(20_000.0)
        .min(20.0)
        .scale(ParameterScale::Log)
        .step(1.0)
        .child(
            ParameterSliderHeader::new()
                .child(ParameterSliderLabel::new("High-pass filter"))
                .child(ParameterSliderValue::new()),
        )
        .child(ParameterSliderControl::new())
        .child(ParameterSliderMarks::new())
        .into_any_element()
}
