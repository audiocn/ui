//! Gallery page for `mixer`. One variant per docs example:
//! `components/examples/mixer-demo.tsx`, `components/examples/mixer-console.tsx`,
//! `components/examples/mixer-empty.tsx`.

use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    prelude::FluentBuilder, px,
};

use audiocn::audio::ballistics::BallisticsPreset;
use audiocn::audio::decibels::{FormatDbOptions, format_db};
use audiocn::signal::demo_signal::{
    DemoMixerChannel, DemoSignalKind, DemoSignalOptions, use_demo_mixer, use_demo_signal,
};
use audiocn::signal::mixer::{MixerChannelInit, MixerOptions, MixerStore, use_mixer};
use audiocn::ui::icons::{Icon, IconName};
use audiocn::{
    AudioSize, ChannelStrip, ChannelStripHeader, ChannelStripIcon, ChannelStripText,
    ChannelStripTitle, ChannelStripValue, ChannelStripVariant, ChannelToggleSize, Fader,
    FrameSource, LevelMeter, MeterFrame, Mixer, MixerChannels, MixerEmpty, MixerHeader,
    MixerMaster, MixerSeparator, MixerTitle, MuteToggle, Orientation, SoloToggle,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "mixer",
    title: "Mixer",
    description: "The container for channel strips, with shared meter settings and keyboard navigation.",
    variants: &["default", "console", "empty"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "console" => console(window, cx),
        "empty" => empty(),
        _ => return None,
    })
}

fn set_gain(
    mixer: &Entity<MixerStore>,
    id: &'static str,
) -> impl Fn(f32, &mut Window, &mut App) + use<> {
    let mixer = mixer.clone();
    move |db, _, cx| {
        mixer.update(cx, |m, cx| {
            m.set_gain(id, db);
            cx.notify();
        })
    }
}

fn set_muted(
    mixer: &Entity<MixerStore>,
    id: &'static str,
) -> impl Fn(bool, &mut Window, &mut App) + use<> {
    let mixer = mixer.clone();
    move |muted, _, cx| {
        mixer.update(cx, |m, cx| {
            m.set_muted(id, muted);
            cx.notify();
        })
    }
}

fn set_solo(
    mixer: &Entity<MixerStore>,
    id: &'static str,
) -> impl Fn(bool, &mut Window, &mut App) + use<> {
    let mixer = mixer.clone();
    move |solo, _, cx| {
        mixer.update(cx, |m, cx| {
            m.set_solo(id, solo, false);
            cx.notify();
        })
    }
}

/// The parts every strip of these examples shares: a small meter fed by
/// `source`, a fader on the mixer's gain, its value and mute / solo.
fn strip(
    base: &SharedString,
    mixer: &Entity<MixerStore>,
    id: &'static str,
    title: &str,
    header: ChannelStripHeader,
    source: FrameSource<MeterFrame>,
    cx: &App,
) -> Option<ChannelStrip> {
    let state = mixer.read(cx);
    let channel = state.channel(id)?.clone();
    let audible = state.is_audible(id);
    Some(
        ChannelStrip::new(format!("{base}/{id}"))
            .dimmed(state.is_dimmed(id))
            .muted(channel.muted)
            .solo(channel.solo)
            .child(header)
            .meter(
                LevelMeter::new(format!("{base}/{id}/meter"))
                    .aria_label(format!("{title} level"))
                    .when(!audible, |meter| {
                        meter.ballistics(BallisticsPreset::Instant)
                    })
                    .size(AudioSize::Sm)
                    .source(source),
            )
            .fader(
                Fader::new(format!("{base}/{id}/fader"))
                    .aria_label(format!("{title} volume"))
                    .on_value_change(set_gain(mixer, id))
                    .size(AudioSize::Sm)
                    .value(channel.gain_db),
            )
            .child(
                ChannelStripValue::new()
                    .child(format_db(channel.gain_db, FormatDbOptions::default())),
            )
            .controls([
                MuteToggle::new(format!("{base}/{id}/mute"))
                    .on_pressed_change(set_muted(mixer, id))
                    .pressed(channel.muted)
                    .size(ChannelToggleSize::Sm)
                    .child("M"),
                SoloToggle::new(format!("{base}/{id}/solo"))
                    .on_pressed_change(set_solo(mixer, id))
                    .pressed(channel.solo)
                    .size(ChannelToggleSize::Sm)
                    .child("S"),
            ]),
    )
}

/// `mixer-demo.tsx`: microphone, system audio and music rows over a master
/// strip, metered by the docs' demo mixer.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("mixer", "default");
    let channels: [(&'static str, IconName, DemoSignalKind, usize, &str); 3] = [
        (
            "mic",
            IconName::Microphone,
            DemoSignalKind::Speech,
            1,
            "Microphone",
        ),
        (
            "system",
            IconName::Desktop,
            DemoSignalKind::Noise,
            2,
            "System audio",
        ),
        (
            "music",
            IconName::MusicNotes,
            DemoSignalKind::Music,
            2,
            "Music",
        ),
    ];
    let mixer = use_mixer(
        format!("{base}/mixer"),
        MixerOptions {
            channels: channels
                .iter()
                .map(|(id, ..)| MixerChannelInit::new(*id))
                .collect(),
            ..Default::default()
        },
        window,
        cx,
    );
    let demo_channels: Vec<DemoMixerChannel> = channels
        .iter()
        .map(|(id, _, kind, count, _)| {
            DemoMixerChannel::new(
                *id,
                DemoSignalOptions {
                    kind: *kind,
                    channels: *count,
                    ..Default::default()
                },
            )
        })
        .collect();
    let state = mixer.read(cx).state.clone();
    let demo = use_demo_mixer(format!("{base}/demo"), &demo_channels, &state, window, cx);
    let audible = channels.iter().any(|(id, ..)| state.is_audible(id));

    let strips: Vec<ChannelStrip> = channels
        .iter()
        .filter_map(|(id, icon, _, _, title)| {
            let header = ChannelStripHeader::new()
                .child(ChannelStripIcon::new().child(Icon::new(*icon)))
                .child(ChannelStripText::new().child(ChannelStripTitle::new(*title)));
            strip(&base, &mixer, id, title, header, demo.source(id)?, cx)
        })
        .collect();
    let master = mixer.clone();

    Mixer::new(format!("{base}/root"))
        .w_full()
        .max_w(px(672.0))
        .child(MixerHeader::new().child(MixerTitle::new("Audio mixer")))
        .child(MixerChannels::new().children(strips))
        .child(MixerSeparator::new())
        .child(
            MixerMaster::new().child(
                ChannelStrip::new(format!("{base}/master"))
                    .variant(ChannelStripVariant::Master)
                    .child(
                        ChannelStripHeader::new()
                            .child(ChannelStripIcon::new().child(Icon::new(IconName::SpeakerHigh)))
                            .child(ChannelStripTitle::new("Master")),
                    )
                    .meter(
                        LevelMeter::new(format!("{base}/master/meter"))
                            .aria_label("Master level")
                            .when(!audible, |meter| {
                                meter.ballistics(BallisticsPreset::Instant)
                            })
                            .channel_count(2)
                            .size(AudioSize::Sm)
                            .source(demo.master),
                    )
                    .fader(
                        Fader::new(format!("{base}/master/fader"))
                            .aria_label("Master volume")
                            .on_value_change(move |db, _, cx| {
                                master.update(cx, |m, cx| {
                                    m.set_master_gain(db);
                                    cx.notify();
                                })
                            })
                            .size(AudioSize::Sm)
                            .value(state.master.gain_db),
                    ),
            ),
        )
        .into_any_element()
}

const CHANNEL_COUNT: usize = 16;
const KINDS: [DemoSignalKind; 4] = [
    DemoSignalKind::Speech,
    DemoSignalKind::Music,
    DemoSignalKind::Noise,
    DemoSignalKind::Music,
];
const INPUT_IDS: [&str; CHANNEL_COUNT] = [
    "input-1", "input-2", "input-3", "input-4", "input-5", "input-6", "input-7", "input-8",
    "input-9", "input-10", "input-11", "input-12", "input-13", "input-14", "input-15", "input-16",
];

/// `mixer-console.tsx`: sixteen console strips side by side, scrolling.
fn console(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("mixer", "console");
    let mixer = use_mixer(
        format!("{base}/mixer"),
        MixerOptions {
            channels: INPUT_IDS.into_iter().map(MixerChannelInit::new).collect(),
            ..Default::default()
        },
        window,
        cx,
    );
    let mut strips = Vec::with_capacity(CHANNEL_COUNT);
    for (index, id) in INPUT_IDS.into_iter().enumerate() {
        let title = format!("In {}", index + 1);
        let (gain_db, audible) = {
            let state = mixer.read(cx);
            (
                state.channel(id).map(|channel| channel.gain_db),
                state.is_audible(id),
            )
        };
        let signal = use_demo_signal(
            format!("{base}/{id}/signal"),
            DemoSignalOptions {
                gain_db: gain_db.unwrap_or(0.0),
                kind: KINDS[index % KINDS.len()],
                playing: audible,
                seed: (index + 1) as f64,
                ..Default::default()
            },
            window,
            cx,
        );
        let header = ChannelStripHeader::new().child(ChannelStripTitle::new(title.clone()));
        strips.extend(strip(&base, &mixer, id, &title, header, signal.meter, cx));
    }

    Mixer::new(format!("{base}/root"))
        .w_full()
        .orientation(Orientation::Vertical)
        .child(MixerHeader::new().child(MixerTitle::new("Console")))
        .child(MixerChannels::new().children(strips))
        .into_any_element()
}

/// `mixer-empty.tsx`: the empty state in the channels' place.
fn empty() -> AnyElement {
    let base = id("mixer", "empty");
    Mixer::new(format!("{base}/root"))
        .w_full()
        .max_w(px(448.0))
        .child(MixerHeader::new().child(MixerTitle::new("Audio mixer")))
        .child(MixerChannels::new())
        .child(MixerEmpty::new().child("No audio sources yet. Add a microphone to get started."))
        .into_any_element()
}
