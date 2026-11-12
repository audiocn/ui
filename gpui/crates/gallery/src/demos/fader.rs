//! Gallery page for `fader`. One variant per docs example:
//! `components/examples/fader-demo.tsx`, `fader-bipolar.tsx`, `fader-silence.tsx`,
//! `fader-sizes.tsx`, `fader-vertical.tsx`, `fader-with-meter.tsx`.

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::audio::taper::TaperInput;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::{
    AudioSize, Fader, FaderLabel, FaderRange, FaderReset, FaderScale, FaderThumb, FaderTrack,
    FaderValue, FaderVariant, LevelMeter, Orientation,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "fader",
    title: "Fader",
    description: "A volume fader in decibels, with tapers, detents, a scale, reset and an editable value.",
    variants: &[
        "default",
        "bipolar",
        "silence",
        "sizes",
        "vertical",
        "with-meter",
    ],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "bipolar" => bipolar(window, cx),
        "silence" => silence(window, cx),
        "sizes" => sizes(),
        "vertical" => vertical(window, cx),
        "with-meter" => with_meter(window, cx),
        _ => return None,
    })
}

/// `useState(initial)` for a dB value.
fn use_db(key: &str, initial: f32, window: &mut Window, cx: &mut App) -> (f32, Entity<f32>) {
    let state = window.use_keyed_state(format!("fader/{key}/state"), cx, move |_, _| initial);
    (*state.read(cx), state)
}

fn setter(state: Entity<f32>) -> impl Fn(f32, &mut Window, &mut App) + 'static {
    move |db, _, cx| {
        state.update(cx, |v, cx| {
            *v = db;
            cx.notify();
        })
    }
}

/// `fader-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let (gain_db, state) = use_db("default", -6.0, window, cx);
    Fader::new(id("fader", "default"))
        .max_w(px(384.0))
        .on_value_change(setter(state))
        .value(gain_db)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(FaderLabel::new("Microphone").mr_auto())
                .child(FaderReset::new())
                .child(FaderValue::new()),
        )
        .child(
            FaderTrack::new()
                .child(FaderRange::new())
                .child(FaderThumb::new()),
        )
        .child(FaderScale::new())
        .into_any_element()
}

/// `fader-bipolar.tsx`.
fn bipolar(window: &mut Window, cx: &mut App) -> AnyElement {
    let (gain_db, state) = use_db("bipolar", 0.0, window, cx);
    Fader::new(id("fader", "bipolar"))
        .max_w(px(384.0))
        .max(24.0)
        .min(-24.0)
        .on_value_change(setter(state))
        .origin(0.0)
        .step(1.0)
        .value(gain_db)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(FaderLabel::new("Input gain"))
                .child(FaderValue::new().editable(true)),
        )
        .child(
            FaderTrack::new()
                .child(FaderRange::new())
                .child(FaderThumb::new()),
        )
        .child(FaderScale::new().ticks([-24.0, -12.0, 0.0, 12.0, 24.0]))
        .into_any_element()
}

/// `fader-silence.tsx`.
fn silence(window: &mut Window, cx: &mut App) -> AnyElement {
    let (volume_db, state) = use_db("silence", -10.0, window, cx);
    Fader::new(id("fader", "silence"))
        .max_w(px(384.0))
        .max(6.0)
        .min(-80.0)
        .on_value_change(setter(state))
        .silence_at_min(true)
        .taper(TaperInput::Audio)
        .value(volume_db)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(FaderLabel::new("Output"))
                .child(FaderValue::new()),
        )
        .child(
            FaderTrack::new()
                .child(FaderRange::new())
                .child(FaderThumb::new()),
        )
        .child(FaderScale::new().ticks([6.0, 0.0, -10.0, -20.0, -40.0, -80.0]))
        .into_any_element()
}

/// `fader-sizes.tsx`.
fn sizes() -> AnyElement {
    let sizes = [
        ("sm", AudioSize::Sm),
        ("default", AudioSize::Default),
        ("lg", AudioSize::Lg),
    ];
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap_6()
        .children(sizes.into_iter().map(|(name, size)| {
            Fader::new(id("fader", &format!("sizes/{name}")))
                .aria_label(format!("{name} fader"))
                .default_value(-12.0)
                .size(size)
        }))
        .child(
            Fader::new(id("fader", "sizes/disabled"))
                .aria_label("Disabled fader")
                .default_value(-24.0)
                .disabled(true),
        )
        .into_any_element()
}

/// `fader-vertical.tsx`.
fn vertical(window: &mut Window, cx: &mut App) -> AnyElement {
    let channels = [("Mic", 0.0), ("Music", -18.0), ("Game", -12.0)];
    let muted = cx.theme().muted_foreground;
    let mut row = div().flex().h(px(256.0)).gap_6();
    for (channel, initial) in channels {
        let (level, state) = use_db(&format!("vertical/{channel}"), initial, window, cx);
        row = row.child(
            Fader::new(id("fader", &format!("vertical/{channel}")))
                .flex_col()
                .items_center()
                .on_value_change(setter(state))
                .orientation(Orientation::Vertical)
                .taper(TaperInput::Audio)
                .value(level)
                .variant(FaderVariant::Console)
                .child(FaderValue::new())
                .child(
                    div()
                        .flex()
                        .min_h_0()
                        .flex_1()
                        .gap_1()
                        .child(
                            FaderTrack::new()
                                .child(FaderRange::new())
                                .child(FaderThumb::new()),
                        )
                        .child(FaderScale::new()),
                )
                .child(
                    FaderLabel::new(channel)
                        .text_color(muted)
                        .text_size(px(12.0))
                        .line_height(px(16.0)),
                ),
        );
    }
    row.into_any_element()
}

/// `fader-with-meter.tsx`.
fn with_meter(window: &mut Window, cx: &mut App) -> AnyElement {
    let (gain_db, state) = use_db("with-meter", 0.0, window, cx);
    let signal = use_demo_signal(
        "fader/with-meter/signal",
        DemoSignalOptions::kind(DemoSignalKind::Music)
            .with_channels(2)
            .with_gain_db(gain_db),
        window,
        cx,
    );
    Fader::new(id("fader", "with-meter"))
        .aria_label("Program gain")
        .h(px(256.0))
        .flex_col()
        .items_center()
        .max(6.0)
        .min(-60.0)
        .on_value_change(setter(state))
        .orientation(Orientation::Vertical)
        .size(AudioSize::Lg)
        .value(gain_db)
        .variant(FaderVariant::Console)
        .child(FaderValue::new())
        .child(
            FaderTrack::new()
                .w(px(24.0))
                .bg(gpui::transparent_black())
                .child(
                    LevelMeter::new("fader/with-meter/meter")
                        .aria_label("Program level")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h_full()
                        .min_h_0()
                        .max_db(6.0)
                        .orientation(Orientation::Vertical)
                        .size(AudioSize::Sm)
                        .source(signal.meter),
                )
                .child(FaderThumb::new()),
        )
        .into_any_element()
}
