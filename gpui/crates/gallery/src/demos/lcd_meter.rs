//! Gallery page for `lcd-meter`. One variant per docs example:
//! `components/examples/lcd-meter-demo.tsx`,
//! `components/examples/lcd-meter-inks.tsx`,
//! `components/examples/lcd-meter-microphone.tsx`,
//! `components/examples/lcd-meter-segments.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::ui::icons::IconName;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    LcdDisplay, LcdDisplayAppearance, LcdDisplayLabel, LcdDisplayRow, LcdDisplaySelect,
    LcdDisplaySelectContent, LcdDisplaySelectItem, LcdDisplaySelectTrigger, LcdDisplaySeparator,
    LcdDisplaySize, LcdDisplayStatus, LcdDisplayStatusState, LcdDisplayVariant, LcdMeter,
    LcdMeterBar, LcdMeterChannel, LcdMeterClip, LcdMeterHold, LcdMeterLabel, LcdMeterScale,
    LcdMeterTrack, LcdMeterValue,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "lcd-meter",
    title: "LCD Meter",
    description: "A level meter on an LCD Display's screen, with block segments, a held peak, readouts, a printed scale and a CLIP flag.",
    variants: &["default", "inks", "microphone", "segments"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "inks" => inks(window, cx),
        "microphone" => microphone(window, cx),
        "segments" => segments(window, cx),
        _ => return None,
    })
}

const INPUTS: [(&str, &str, DemoSignalKind); 3] = [
    ("line", "Line In", DemoSignalKind::Music),
    ("usb", "USB Mic", DemoSignalKind::Speech),
    ("tone", "Test Tone", DemoSignalKind::Tone),
];

/// `lcd-meter-demo.tsx`: an input select over a stereo meter.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("lcd-meter", "default");
    let input = window.use_keyed_state(base.clone(), cx, |_, _| SharedString::from("line"));
    let current = input.read(cx).clone();
    let kind = INPUTS
        .iter()
        .find(|(value, _, _)| *value == current.as_ref())
        .map_or(INPUTS[0].2, |(_, _, kind)| *kind);
    let signal = use_demo_signal(
        SharedString::from(format!("{base}/signal")),
        DemoSignalOptions {
            channels: 2,
            kind,
            seed: 3.0,
            ..Default::default()
        },
        window,
        cx,
    );
    LcdDisplay::new(base)
        .w_full()
        .max_w(px(384.0))
        .child(
            LcdDisplayRow::new()
                .child(LcdDisplayLabel::new("Input"))
                .child(
                    LcdDisplaySelect::new("input")
                        .items(INPUTS.iter().map(|(value, label, _)| (*value, *label)))
                        .value(current)
                        .on_value_change(move |next, _, cx| {
                            input.update(cx, |input, cx| {
                                *input = next;
                                cx.notify();
                            })
                        })
                        .child(LcdDisplaySelectTrigger::new())
                        .child(
                            LcdDisplaySelectContent::new().children(INPUTS.iter().map(
                                |(value, label, _)| LcdDisplaySelectItem::new(*value, *label),
                            )),
                        ),
                ),
        )
        .child(LcdDisplaySeparator::new())
        .child(
            LcdMeter::new("meter")
                .aria_label("Input level")
                .source(signal.meter),
        )
        .child(LcdDisplayStatus::new())
        .into_any_element()
}

/// One ink of `lcd-meter-inks.tsx`.
struct Ink {
    kind: DemoSignalKind,
    label: &'static str,
    seed: f64,
    variant: LcdDisplayVariant,
    appearance: LcdDisplayAppearance,
}

const INKS: [Ink; 4] = [
    Ink {
        kind: DemoSignalKind::Music,
        label: "Drums",
        seed: 1.0,
        variant: LcdDisplayVariant::Amber,
        appearance: LcdDisplayAppearance::Auto,
    },
    Ink {
        kind: DemoSignalKind::Speech,
        label: "Vocal",
        seed: 2.0,
        variant: LcdDisplayVariant::Green,
        appearance: LcdDisplayAppearance::Auto,
    },
    Ink {
        kind: DemoSignalKind::Music,
        label: "Keys",
        seed: 5.0,
        variant: LcdDisplayVariant::Cyan,
        appearance: LcdDisplayAppearance::Auto,
    },
    // A dark screen in both themes.
    Ink {
        kind: DemoSignalKind::Music,
        label: "Bass",
        seed: 8.0,
        variant: LcdDisplayVariant::Red,
        appearance: LcdDisplayAppearance::Dark,
    },
];

/// `lcd-meter-inks.tsx`: four small displays, one per hardware ink.
fn inks(window: &mut Window, cx: &mut App) -> AnyElement {
    let mut grid = div()
        .grid()
        .grid_cols(2)
        .gap(px(16.0))
        .w_full()
        .max_w(px(576.0));
    for ink in &INKS {
        let base = id("lcd-meter", &format!("inks/{}", ink.label));
        let signal = use_demo_signal(
            SharedString::from(format!("{base}/signal")),
            DemoSignalOptions {
                kind: ink.kind,
                seed: ink.seed,
                ..Default::default()
            },
            window,
            cx,
        );
        grid = grid.child(
            LcdDisplay::new(base)
                .appearance(ink.appearance)
                .size(LcdDisplaySize::Sm)
                .variant(ink.variant)
                .child(
                    LcdMeter::new("meter")
                        .aria_label(format!("{} level", ink.label))
                        .source(signal.meter),
                ),
        );
    }
    grid.into_any_element()
}

/// `lcd-meter-microphone.tsx`: a custom row named Mic, with the status pixel
/// lit while listening. The gallery has no microphone: the button starts a
/// demo speech signal in its place.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("lcd-meter", "microphone");
    let listening = window.use_keyed_state(
        SharedString::from(format!("{base}/listening")),
        cx,
        |_, _| false,
    );
    let on = *listening.read(cx);
    let signal = use_demo_signal(
        SharedString::from(format!("{base}/signal")),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_playing(on),
        window,
        cx,
    );
    let toggle = listening.clone();
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(16.0))
        .child(
            LcdDisplay::new(base.clone())
                .child(
                    LcdMeter::new("meter")
                        .aria_label("Microphone level")
                        .source(on.then_some(signal.meter))
                        .child(
                            LcdMeterChannel::new()
                                .child(LcdMeterLabel::new("Mic"))
                                .child(
                                    LcdMeterTrack::new()
                                        .child(LcdMeterBar::new())
                                        .child(LcdMeterHold::new()),
                                )
                                .child(LcdMeterValue::new()),
                        )
                        .child(LcdMeterScale::new())
                        .child(LcdMeterClip::new()),
                )
                .child(LcdDisplayStatus::new().state(if on {
                    LcdDisplayStatusState::Idle
                } else {
                    LcdDisplayStatusState::Off
                })),
        )
        .child(
            div().flex().child(
                Button::new(SharedString::from(format!("{base}/button")))
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Outline)
                    .icon_start(if on {
                        IconName::MicrophoneSlash
                    } else {
                        IconName::Microphone
                    })
                    .child(if on {
                        "Stop microphone"
                    } else {
                        "Use my microphone"
                    })
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |on, cx| {
                            *on = !*on;
                            cx.notify();
                        })
                    }),
            ),
        )
        .into_any_element()
}

/// `lcd-meter-segments.tsx`: blocks, a fine ladder and a solid bar.
fn segments(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("lcd-meter", "segments");
    let signal = use_demo_signal(
        SharedString::from(format!("{base}/signal")),
        DemoSignalOptions {
            kind: DemoSignalKind::Music,
            seed: 4.0,
            ..Default::default()
        },
        window,
        cx,
    );
    let pitch = px(2.0);
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(16.0))
        .child(
            LcdDisplay::new(SharedString::from(format!("{base}/blocks")))
                .size(LcdDisplaySize::Sm)
                .child(
                    LcdMeter::new("meter")
                        .aria_label("Blocks")
                        .source(signal.meter.clone()),
                ),
        )
        .child(
            // One dot wide, one dot apart.
            LcdDisplay::new(SharedString::from(format!("{base}/fine")))
                .size(LcdDisplaySize::Sm)
                .child(
                    LcdMeter::new("meter")
                        .aria_label("Fine ladder")
                        .segment(pitch)
                        .source(signal.meter.clone()),
                ),
        )
        .child(
            LcdDisplay::new(SharedString::from(format!("{base}/solid")))
                .size(LcdDisplaySize::Sm)
                .child(
                    LcdMeter::new("meter")
                        .aria_label("Solid bar")
                        .gap(px(0.0))
                        .source(signal.meter),
                ),
        )
        .into_any_element()
}
