//! Gallery page for `channel-strip`. One variant per docs example:
//! `components/examples/channel-strip-demo.tsx`,
//! `components/examples/channel-strip-console.tsx`,
//! `components/examples/channel-strip-notices.tsx`.

use gpui::{
    AnyElement, App, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use audiocn::audio::ballistics::BallisticsPreset;
use audiocn::audio::decibels::{FormatDbOptions, format_db};
use audiocn::audio::taper::TaperInput;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::signal::mixer::{MixerChannelInit, MixerOptions, MixerStore, use_mixer};
use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::{Icon, IconName};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    AudioSize, ChannelStrip, ChannelStripDescription, ChannelStripHeader, ChannelStripIcon,
    ChannelStripNotice, ChannelStripNoticeVariant, ChannelStripStatus, ChannelStripStatusTone,
    ChannelStripText, ChannelStripTitle, ChannelStripValue, ChannelStripVariant, ChannelToggleSize,
    DbReadout, Fader, LevelMeter, MuteToggle, Orientation, SoloToggle,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "channel-strip",
    title: "Channel Strip",
    description: "One channel of a mixer, as a row or a console strip, composed from the parts you need.",
    variants: &["default", "console", "notices"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "console" => console(window, cx),
        "notices" => notices(),
        _ => return None,
    })
}

#[derive(Clone, Copy, Default)]
struct DemoState {
    gain_db: f32,
    muted: bool,
    solo: bool,
}

fn update(state: &Entity<DemoState>, cx: &mut App, f: impl FnOnce(&mut DemoState)) {
    state.update(cx, |state, cx| {
        f(state);
        cx.notify();
    });
}

/// `channel-strip-demo.tsx`: a microphone row with a live meter, fader,
/// readout and mute / solo.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("channel-strip", "default");
    let state = window.use_keyed_state(base.clone(), cx, |_, _| DemoState::default());
    let DemoState {
        gain_db,
        muted,
        solo,
    } = *state.read(cx);
    let signal = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions {
            gain_db,
            kind: DemoSignalKind::Speech,
            playing: !muted,
            ..Default::default()
        },
        window,
        cx,
    );
    let (gain, mute, solo_state) = (state.clone(), state.clone(), state);

    ChannelStrip::new(format!("{base}/strip"))
        .max_w(px(672.0))
        .muted(muted)
        .solo(solo)
        .child(
            ChannelStripHeader::new()
                .child(ChannelStripIcon::new().child(Icon::new(IconName::Microphone)))
                .child(
                    ChannelStripText::new()
                        .child(ChannelStripTitle::new("Microphone"))
                        .child(ChannelStripDescription::new("Shure MV7+")),
                )
                .child(
                    ChannelStripStatus::new()
                        .tone(if muted {
                            ChannelStripStatusTone::Muted
                        } else {
                            ChannelStripStatusTone::Live
                        })
                        .child(if muted { "Muted" } else { "Live" }),
                ),
        )
        .meter(
            LevelMeter::new(format!("{base}/meter"))
                .aria_label("Microphone level")
                .when(muted, |meter| meter.ballistics(BallisticsPreset::Instant))
                .size(AudioSize::Sm)
                .source(signal.meter.clone()),
        )
        .fader(
            Fader::new(format!("{base}/fader"))
                .aria_label("Microphone volume")
                .on_value_change(move |db, _, cx| update(&gain, cx, |s| s.gain_db = db))
                .size(AudioSize::Sm)
                .value(gain_db),
        )
        .child(
            ChannelStripValue::new()
                .child(DbReadout::new(format!("{base}/readout")).source(signal.meter)),
        )
        .controls([
            MuteToggle::new(format!("{base}/mute"))
                .on_pressed_change(move |pressed, _, cx| update(&mute, cx, |s| s.muted = pressed))
                .pressed(muted)
                .size(ChannelToggleSize::Sm)
                .child("M"),
            SoloToggle::new(format!("{base}/solo"))
                .on_pressed_change(move |pressed, _, cx| {
                    update(&solo_state, cx, |s| s.solo = pressed)
                })
                .pressed(solo)
                .size(ChannelToggleSize::Sm)
                .child("S"),
        ])
        .into_any_element()
}

/// One strip of `channel-strip-console.tsx`.
struct ConsoleStrip {
    id: &'static str,
    title: &'static str,
    icon: IconName,
    accent: Hsla,
    kind: DemoSignalKind,
    seed: Option<f64>,
}

/// `channel-strip-console.tsx`: three card console strips side by side, with
/// colour tags, sharing one mixer's mute and solo.
fn console(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("channel-strip", "console");
    let mixer = use_mixer(
        format!("{base}/mixer"),
        MixerOptions {
            channels: ["mic", "music", "game"]
                .into_iter()
                .map(MixerChannelInit::new)
                .collect(),
            ..Default::default()
        },
        window,
        cx,
    );
    let theme = cx.theme();
    let strips = [
        ConsoleStrip {
            id: "mic",
            title: "Mic",
            icon: IconName::Microphone,
            accent: theme.chart_2,
            kind: DemoSignalKind::Speech,
            seed: None,
        },
        ConsoleStrip {
            id: "music",
            title: "Music",
            icon: IconName::MusicNotes,
            accent: theme.chart_1,
            kind: DemoSignalKind::Music,
            seed: None,
        },
        ConsoleStrip {
            id: "game",
            title: "Game",
            icon: IconName::Desktop,
            accent: theme.chart_4,
            kind: DemoSignalKind::Noise,
            seed: Some(3.0),
        },
    ];
    let children: Vec<AnyElement> = strips
        .into_iter()
        .filter_map(|strip| console_strip(&base, &mixer, strip, window, cx))
        .collect();
    div()
        .id(SharedString::from(format!("{base}/scroll")))
        .flex()
        .h(px(384.0))
        .max_w_full()
        .gap(px(12.0))
        .overflow_x_scroll()
        .children(children)
        .into_any_element()
}

fn console_strip(
    base: &SharedString,
    mixer: &Entity<MixerStore>,
    strip: ConsoleStrip,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let ConsoleStrip {
        id,
        title,
        icon,
        accent,
        kind,
        seed,
    } = strip;
    let (channel, audible, dimmed) = {
        let state = mixer.read(cx);
        (
            state.channel(id).cloned(),
            state.is_audible(id),
            state.is_dimmed(id),
        )
    };
    let signal = use_demo_signal(
        format!("{base}/{id}/signal"),
        DemoSignalOptions {
            channels: 2,
            gain_db: channel.as_ref().map_or(0.0, |channel| channel.gain_db),
            kind,
            playing: audible,
            seed: seed.unwrap_or(1.0),
            ..Default::default()
        },
        window,
        cx,
    );
    let channel = channel?;
    let (gain, mute, solo) = (mixer.clone(), mixer.clone(), mixer.clone());
    Some(
        ChannelStrip::new(format!("{base}/{id}"))
            .accent(accent)
            .dimmed(dimmed)
            .muted(channel.muted)
            .solo(channel.solo)
            .orientation(Orientation::Vertical)
            .variant(ChannelStripVariant::Card)
            .child(
                ChannelStripHeader::new()
                    .child(ChannelStripIcon::new().child(Icon::new(icon)))
                    .child(ChannelStripTitle::new(title)),
            )
            .meter(
                LevelMeter::new(format!("{base}/{id}/meter"))
                    .aria_label(format!("{title} level"))
                    .when(!audible, |meter| {
                        meter.ballistics(BallisticsPreset::Instant)
                    })
                    .channel_count(2)
                    .h_full()
                    .size(AudioSize::Sm)
                    .source(signal.meter),
            )
            .fader(
                Fader::new(format!("{base}/{id}/fader"))
                    .aria_label(format!("{title} volume"))
                    .on_value_change(move |db, _, cx| {
                        gain.update(cx, |m, cx| {
                            m.set_gain(id, db);
                            cx.notify();
                        })
                    })
                    .size(AudioSize::Sm)
                    .taper(TaperInput::Audio)
                    .value(channel.gain_db),
            )
            .child(
                ChannelStripValue::new()
                    .child(format_db(channel.gain_db, FormatDbOptions::default())),
            )
            .controls([
                MuteToggle::new(format!("{base}/{id}/mute"))
                    .aria_label(format!("Mute {title}"))
                    .on_pressed_change(move |muted, _, cx| {
                        mute.update(cx, |m, cx| {
                            m.set_muted(id, muted);
                            cx.notify();
                        })
                    })
                    .pressed(channel.muted)
                    .size(ChannelToggleSize::Sm)
                    .child("M"),
                SoloToggle::new(format!("{base}/{id}/solo"))
                    .aria_label(format!("Solo {title}"))
                    .on_pressed_change(move |soloed, _, cx| {
                        solo.update(cx, |m, cx| {
                            m.set_solo(id, soloed, false);
                            cx.notify();
                        })
                    })
                    .pressed(channel.solo)
                    .size(ChannelToggleSize::Sm)
                    .child("S"),
            ])
            .into_any_element(),
    )
}

/// `channel-strip-notices.tsx`: a blocked microphone and a paused system
/// audio source, each with a notice under the strip.
fn notices() -> AnyElement {
    let base = id("channel-strip", "notices");
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(672.0))
        .gap(px(12.0))
        .child(
            ChannelStrip::new(format!("{base}/mic"))
                .disabled(true)
                .child(
                    ChannelStripHeader::new()
                        .child(
                            ChannelStripIcon::new().child(Icon::new(IconName::MicrophoneSlash)),
                        )
                        .child(ChannelStripText::new().child(ChannelStripTitle::new("Microphone")))
                        .child(
                            ChannelStripStatus::new()
                                .tone(ChannelStripStatusTone::Error)
                                .child("Blocked"),
                        ),
                )
                .meter(
                    LevelMeter::new(format!("{base}/mic/meter"))
                        .aria_label("Microphone level")
                        .size(AudioSize::Sm),
                )
                .child(
                    ChannelStripNotice::new()
                        .variant(ChannelStripNoticeVariant::Destructive)
                        .child(
                            "Microphone access is blocked. Allow it in your browser's site settings.",
                        ),
                ),
        )
        .child(
            ChannelStrip::new(format!("{base}/system"))
                .child(
                    ChannelStripHeader::new()
                        .child(ChannelStripIcon::new().child(Icon::new(IconName::Desktop)))
                        .child(
                            ChannelStripText::new().child(ChannelStripTitle::new("System audio")),
                        )
                        .child(
                            ChannelStripStatus::new()
                                .tone(ChannelStripStatusTone::Warning)
                                .child("Paused"),
                        ),
                )
                .meter(
                    LevelMeter::new(format!("{base}/system/meter"))
                        .aria_label("System audio level")
                        .size(AudioSize::Sm),
                )
                .child(
                    ChannelStripNotice::new()
                        .variant(ChannelStripNoticeVariant::Warning)
                        .icon(IconName::Warning)
                        .child(
                            div()
                                .flex_1()
                                .child("Paused to prevent an echo from your own stream."),
                        )
                        .child(
                            Button::new(format!("{base}/system/resume"))
                                .size(ButtonSize::Xs)
                                .variant(ButtonVariant::Outline)
                                .child("Resume"),
                        ),
                ),
        )
        .into_any_element()
}
