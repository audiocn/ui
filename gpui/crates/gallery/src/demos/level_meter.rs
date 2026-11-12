//! Gallery page for `level-meter`. One variant per docs example:
//! `components/examples/level-meter-demo.tsx`, `level-meter-ballistics.tsx`,
//! `level-meter-css-level.tsx`, `level-meter-custom-colors.tsx`,
//! `level-meter-dual.tsx`, `level-meter-microphone.tsx`,
//! `level-meter-simple.tsx`, `level-meter-values.tsx`,
//! `level-meter-variants.tsx`, `level-meter-vertical.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use audiocn::audio::ballistics::BallisticsPreset;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::IconName;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    AudioSize, LevelMeter, LevelMeterBar, LevelMeterChannel, LevelMeterChannels, LevelMeterClip,
    LevelMeterHold, LevelMeterMeasure, LevelMeterScale, LevelMeterTrack, LevelMeterValue,
    LevelMeterVariant, MeterZone, MeterZoneName, Orientation, use_level_meter,
};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "level-meter",
    title: "Level Meter",
    description: "A peak and RMS level meter with zones, peak hold, a scale, a readout and a clip light.",
    section: Section::Components,
    variants: &[
        "default",
        "ballistics",
        "css-level",
        "custom-colors",
        "dual",
        "microphone",
        "simple",
        "values",
        "variants",
        "vertical",
    ],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "ballistics" => ballistics(window, cx),
        "css-level" => css_level(window, cx),
        "custom-colors" => custom_colors(window, cx),
        "dual" => dual(window, cx),
        "microphone" => microphone(window, cx),
        "simple" => simple(window, cx),
        "values" => values(cx),
        "variants" => variants(window, cx),
        "vertical" => vertical(window, cx),
        _ => return None,
    })
}

fn signal(variant: &str, kind: DemoSignalKind) -> (gpui::SharedString, DemoSignalOptions) {
    (
        id("level-meter", &format!("{variant}/signal")),
        DemoSignalOptions::kind(kind),
    )
}

/// A track with a bar and the hold marker.
fn track(index: usize) -> LevelMeterChannel {
    LevelMeterChannel::new().index(index).child(
        LevelMeterTrack::new()
            .child(LevelMeterBar::new())
            .child(LevelMeterHold::new()),
    )
}

/// `level-meter-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("default", DemoSignalKind::Music);
    let signal = use_demo_signal(key, options.with_channels(2), window, cx);
    LevelMeter::new(id("level-meter", "default"))
        .aria_label("Program level")
        .max_w(px(448.0))
        .source(signal.meter)
        .child(
            LevelMeterChannels::new()
                .child(track(0))
                .child(track(1))
                .child(LevelMeterScale::new()),
        )
        .child(LevelMeterValue::new())
        .child(LevelMeterClip::new())
        .into_any_element()
}

/// `grid grid-cols-[<label>_1fr] items-center gap-3`.
fn labelled_row(label_width: f32, label: AnyElement, meter: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(div().flex_none().w(px(label_width)).child(label))
        .child(div().flex_1().min_w_0().flex().child(meter))
        .into_any_element()
}

/// `level-meter-ballistics.tsx`.
fn ballistics(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("ballistics", DemoSignalKind::Speech);
    let signal = use_demo_signal(key, options.with_seed(3), window, cx);
    let muted = cx.theme().muted_foreground;
    let mono = cx.theme().font_mono.clone();
    let presets = [
        ("peak", BallisticsPreset::Peak),
        ("vu", BallisticsPreset::Vu),
        ("instant", BallisticsPreset::Instant),
    ];
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(448.0))
        .gap(px(16.0))
        .children(presets.into_iter().map(|(name, preset)| {
            labelled_row(
                64.0,
                div()
                    .text_color(muted)
                    .font_family(mono.clone())
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .child(name)
                    .into_any_element(),
                LevelMeter::new(id("level-meter", &format!("ballistics/{name}")))
                    .aria_label(format!("{name} ballistics"))
                    .ballistics(preset)
                    .source(signal.meter.clone()),
            )
        }))
        .into_any_element()
}

/// The rings of `level-meter-css-level.tsx`, scaled by `--meter-level`.
#[derive(IntoElement)]
struct SpeakingRings;

impl RenderOnce for SpeakingRings {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let level = use_level_meter("SpeakingRings").channel_reading().level;
        let primary = cx.theme().primary;
        // `absolute size-N scale-[…] rounded-full`, centred in the 80px channel.
        let ring = |size: f32, scale: f32, alpha: f32| {
            let d = size * scale;
            div()
                .absolute()
                .left(px((80.0 - d) / 2.0))
                .top(px((80.0 - d) / 2.0))
                .size(px(d))
                .rounded_full()
                .bg(primary.opacity(alpha))
        };
        div()
            .relative()
            .size(px(80.0))
            .child(ring(80.0, 0.6 + level * 0.6, 0.15))
            .child(ring(56.0, 0.8 + level * 0.4, 0.3))
            .child(ring(40.0, 1.0, 1.0))
    }
}

/// `level-meter-css-level.tsx`.
fn css_level(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("css-level", DemoSignalKind::Speech);
    let signal = use_demo_signal(key, options.with_seed(5), window, cx);
    LevelMeter::new(id("level-meter", "css-level"))
        .aria_label("Speaking indicator")
        .w_auto()
        .source(signal.meter)
        .child(
            LevelMeterChannel::new()
                .relative()
                .size(px(80.0))
                .items_center()
                .justify_center()
                .child(SpeakingRings),
        )
        .into_any_element()
}

/// `level-meter-custom-colors.tsx`.
fn custom_colors(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("custom-colors", DemoSignalKind::Music);
    let signal = use_demo_signal(key, options.with_seed(9), window, cx);
    let theme = cx.theme();
    let zones = [
        MeterZone {
            from_db: f32::NEG_INFINITY,
            zone: MeterZoneName::Ok,
        },
        MeterZone {
            from_db: -12.0,
            zone: MeterZoneName::Warn,
        },
        MeterZone {
            from_db: -3.0,
            zone: MeterZoneName::Clip,
        },
    ];
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(448.0))
        .gap(px(20.0))
        .child(
            LevelMeter::new(id("level-meter", "custom-colors/mono"))
                .aria_label("Monochrome meter")
                .zone_color(MeterZoneName::Clip, theme.foreground)
                .zone_color(MeterZoneName::Ok, theme.muted_foreground)
                .zone_color(MeterZoneName::Warn, theme.foreground)
                .size(AudioSize::Lg)
                .source(signal.meter.clone())
                .variant(LevelMeterVariant::Segmented),
        )
        .child(
            LevelMeter::new(id("level-meter", "custom-colors/zones"))
                .aria_label("Meter with custom zones")
                .size(AudioSize::Lg)
                .source(signal.meter)
                .zones(zones),
        )
        .into_any_element()
}

/// `level-meter-dual.tsx`.
fn dual(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("dual", DemoSignalKind::Speech);
    let signal = use_demo_signal(key, options.with_seed(7), window, cx);
    LevelMeter::new(id("level-meter", "dual"))
        .aria_label("Voice level")
        .max_w(px(448.0))
        .size(AudioSize::Lg)
        .source(signal.meter)
        .child(
            LevelMeterChannels::new()
                .child(
                    LevelMeterChannel::new().child(
                        LevelMeterTrack::new()
                            .child(
                                LevelMeterBar::new()
                                    .opacity(0.4)
                                    .measure(LevelMeterMeasure::Peak),
                            )
                            .child(LevelMeterBar::new().measure(LevelMeterMeasure::Rms))
                            .child(LevelMeterHold::new()),
                    ),
                )
                .child(LevelMeterScale::new()),
        )
        .into_any_element()
}

/// `level-meter-microphone.tsx`. The gallery has no microphone, so "using
/// the microphone" feeds the meter the demo voice instead.
fn microphone(window: &mut Window, cx: &mut App) -> AnyElement {
    let listening =
        window.use_keyed_state(id("level-meter", "microphone/listening"), cx, |_, _| false);
    let on = *listening.read(cx);
    let (key, options) = signal("microphone", DemoSignalKind::Speech);
    let signal = use_demo_signal(key, options.with_playing(on), window, cx);
    let toggle = listening.clone();
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(448.0))
        .gap(px(16.0))
        .child(
            LevelMeter::new(id("level-meter", "microphone"))
                .aria_label("Microphone level")
                .source(on.then_some(signal.meter))
                .child(LevelMeterChannels::new().child(track(0)))
                .child(LevelMeterValue::new())
                .child(LevelMeterClip::new()),
        )
        .child(
            div().flex().child(
                Button::new(id("level-meter", "microphone/button"))
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

/// `level-meter-simple.tsx`.
fn simple(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("simple", DemoSignalKind::Speech);
    let signal = use_demo_signal(key, options, window, cx);
    LevelMeter::new(id("level-meter", "simple"))
        .aria_label("Microphone level")
        .max_w(px(384.0))
        .source(signal.meter)
        .into_any_element()
}

/// `level-meter-values.tsx`.
fn values(cx: &mut App) -> AnyElement {
    let levels = [
        ("Quiet", -42.0),
        ("Good", -16.0),
        ("Hot", -7.0),
        ("Clipping", 0.0),
    ];
    let muted = cx.theme().muted_foreground;
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(384.0))
        .gap(px(12.0))
        .children(levels.into_iter().map(|(label, peak_db)| {
            labelled_row(
                72.0,
                div()
                    .text_color(muted)
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .child(label)
                    .into_any_element(),
                LevelMeter::new(id("level-meter", &format!("values/{label}")))
                    .aria_label(format!("{label} level"))
                    .peak_db(peak_db),
            )
        }))
        .into_any_element()
}

/// `level-meter-variants.tsx`.
fn variants(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("variants", DemoSignalKind::Music);
    let signal = use_demo_signal(key, options.with_channels(2).with_seed(2), window, cx);
    let muted = cx.theme().muted_foreground;
    let variants = [
        ("solid", LevelMeterVariant::Solid),
        ("segmented", LevelMeterVariant::Segmented),
        ("gradient", LevelMeterVariant::Gradient),
    ];
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(448.0))
        .gap(px(20.0))
        .children(variants.into_iter().map(|(name, variant)| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_color(muted)
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(name),
                )
                .child(
                    LevelMeter::new(id("level-meter", &format!("variants/{name}")))
                        .aria_label(format!("{name} meter"))
                        .size(AudioSize::Lg)
                        .source(signal.meter.clone())
                        .variant(variant),
                )
        }))
        .into_any_element()
}

/// `level-meter-vertical.tsx`.
fn vertical(window: &mut Window, cx: &mut App) -> AnyElement {
    let (key, options) = signal("vertical", DemoSignalKind::Music);
    let signal = use_demo_signal(key, options.with_channels(2).with_seed(4), window, cx);
    LevelMeter::new(id("level-meter", "vertical"))
        .aria_label("Program level")
        .h(px(224.0))
        .orientation(Orientation::Vertical)
        .size(AudioSize::Lg)
        .source(signal.meter)
        .child(
            LevelMeterChannels::new()
                .children([0, 1].map(track))
                .child(LevelMeterScale::new()),
        )
        .child(LevelMeterValue::new())
        .into_any_element()
}
