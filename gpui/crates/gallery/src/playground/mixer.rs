//! Playground page for `Mixer`: channel presets, shared meter settings, live `MixerState`.

use audiocn::audio::ballistics::BallisticsPreset;
use audiocn::audio::decibels::{FormatDbOptions, format_db};
use audiocn::audio::zones::DEFAULT_ZONES;
use audiocn::signal::demo_signal::{
    DemoMixerChannel, DemoSignalKind, DemoSignalOptions, use_demo_mixer,
};
use audiocn::signal::mixer::{MixerChannelInit, MixerOptions, MixerState, MixerStore, use_mixer};
use audiocn::ui::icons::{Icon, IconName};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    AudioSize, ChannelStrip, ChannelStripHeader, ChannelStripIcon, ChannelStripText,
    ChannelStripTitle, ChannelStripValue, ChannelStripVariant, Fader, FrameSource, LevelMeter,
    MeterFrame, MeterZone, MeterZoneName, Mixer, MixerActions, MixerChannels, MixerEmpty,
    MixerHeader, MixerMaster, MixerSeparator, MixerTitle, MuteToggle, Orientation, SoloToggle,
};
use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "mixer",
    component: "Mixer",
    source: SourceUse::None,
    controls,
    render,
};

const COUNTS: &[(&str, &str)] = &[("0", "0"), ("3", "3"), ("8", "8"), ("16", "16")];

const ORIENTATIONS: &[(&str, &str)] = &[
    ("horizontal", "Orientation::Horizontal"),
    ("vertical", "Orientation::Vertical"),
];

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

const ZONES: &[(&str, &str)] = &[
    ("default", ""),
    (
        "broadcast −20/−9",
        "audiocn::audio::zones::DEFAULT_ZONES.to_vec()",
    ),
    (
        "tight −12/−3",
        "vec![MeterZone { from_db: f32::NEG_INFINITY, zone: MeterZoneName::Ok }, MeterZone { from_db: -12.0, zone: MeterZoneName::Warn }, MeterZone { from_db: -3.0, zone: MeterZoneName::Clip }]",
    ),
    (
        "wide −30/−18",
        "vec![MeterZone { from_db: f32::NEG_INFINITY, zone: MeterZoneName::Ok }, MeterZone { from_db: -30.0, zone: MeterZoneName::Warn }, MeterZone { from_db: -18.0, zone: MeterZoneName::Clip }]",
    ),
];

const BALLISTICS: &[(&str, &str)] = &[
    ("default", ""),
    ("peak", "BallisticsPreset::Peak"),
    ("vu", "BallisticsPreset::Vu"),
    ("instant", "BallisticsPreset::Instant"),
];

struct Channel {
    id: &'static str,
    title: &'static str,
    icon: IconName,
    kind: DemoSignalKind,
    channels: usize,
    seed: f64,
}

const CHANNELS: [Channel; 16] = [
    Channel {
        id: "mic",
        title: "Microphone",
        icon: IconName::Microphone,
        kind: DemoSignalKind::Speech,
        channels: 1,
        seed: 1.0,
    },
    Channel {
        id: "system",
        title: "System",
        icon: IconName::Desktop,
        kind: DemoSignalKind::Noise,
        channels: 2,
        seed: 2.0,
    },
    Channel {
        id: "music",
        title: "Music",
        icon: IconName::MusicNotes,
        kind: DemoSignalKind::Music,
        channels: 2,
        seed: 3.0,
    },
    Channel {
        id: "in-4",
        title: "In 4",
        icon: IconName::Headphones,
        kind: DemoSignalKind::Tone,
        channels: 1,
        seed: 4.0,
    },
    Channel {
        id: "in-5",
        title: "In 5",
        icon: IconName::Megaphone,
        kind: DemoSignalKind::Speech,
        channels: 1,
        seed: 5.0,
    },
    Channel {
        id: "in-6",
        title: "In 6",
        icon: IconName::Waveform,
        kind: DemoSignalKind::Music,
        channels: 2,
        seed: 6.0,
    },
    Channel {
        id: "in-7",
        title: "In 7",
        icon: IconName::SpeakerHigh,
        kind: DemoSignalKind::Noise,
        channels: 2,
        seed: 7.0,
    },
    Channel {
        id: "in-8",
        title: "In 8",
        icon: IconName::Playlist,
        kind: DemoSignalKind::Tone,
        channels: 1,
        seed: 8.0,
    },
    Channel {
        id: "in-9",
        title: "In 9",
        icon: IconName::Microphone,
        kind: DemoSignalKind::Speech,
        channels: 1,
        seed: 9.0,
    },
    Channel {
        id: "in-10",
        title: "In 10",
        icon: IconName::Desktop,
        kind: DemoSignalKind::Noise,
        channels: 2,
        seed: 10.0,
    },
    Channel {
        id: "in-11",
        title: "In 11",
        icon: IconName::MusicNotes,
        kind: DemoSignalKind::Music,
        channels: 2,
        seed: 11.0,
    },
    Channel {
        id: "in-12",
        title: "In 12",
        icon: IconName::Headphones,
        kind: DemoSignalKind::Tone,
        channels: 1,
        seed: 12.0,
    },
    Channel {
        id: "in-13",
        title: "In 13",
        icon: IconName::Megaphone,
        kind: DemoSignalKind::Speech,
        channels: 1,
        seed: 13.0,
    },
    Channel {
        id: "in-14",
        title: "In 14",
        icon: IconName::Waveform,
        kind: DemoSignalKind::Music,
        channels: 2,
        seed: 14.0,
    },
    Channel {
        id: "in-15",
        title: "In 15",
        icon: IconName::SpeakerHigh,
        kind: DemoSignalKind::Noise,
        channels: 2,
        seed: 15.0,
    },
    Channel {
        id: "in-16",
        title: "In 16",
        icon: IconName::Playlist,
        kind: DemoSignalKind::Tone,
        channels: 1,
        seed: 16.0,
    },
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("channels", "Channels", 1, COUNTS)
            .group("Channels")
            .no_code()
            .remount()
            .help("0, 3, 8 or 16 strips. Remounts MixerState."),
        Control::choice("orientation", "Orientation", 0, ORIENTATIONS).group("Props"),
        Control::choice("size", "Size", 0, SIZES).group("Props"),
        Control::num("min_db", "Min dB", -60.0, -120.0, -12.0, 1.0).group("Meter"),
        Control::num("max_db", "Max dB", 0.0, -12.0, 24.0, 1.0).group("Meter"),
        Control::choice("zones", "Zones", 0, ZONES).group("Meter"),
        Control::choice("ballistics", "Ballistics", 0, BALLISTICS).group("Meter"),
        Control::bool("disabled", "Disabled", false).group("Props"),
        Control::bool("show_header", "Header", true)
            .group("Parts")
            .no_code(),
        Control::text("title", "Title", "Audio mixer")
            .group("Parts")
            .no_code(),
        Control::bool("show_actions", "Actions", true)
            .group("Parts")
            .no_code()
            .help("Reset returns MixerState to the preset."),
        Control::bool("scrollable", "Scrollable", true)
            .group("Parts")
            .no_code()
            .help("MixerChannels::scrollable. Default true."),
        Control::bool("show_separator", "Separator", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_master", "Master", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_empty", "Empty", true)
            .group("Parts")
            .no_code()
            .help("Visible when the channel preset is 0."),
        Control::text(
            "empty",
            "Empty text",
            "No audio sources yet. Add a microphone to get started.",
        )
        .group("Parts")
        .no_code(),
    ]
}

fn channel_count(label: &str) -> usize {
    match label {
        "0" => 0,
        "8" => 8,
        "16" => 16,
        _ => 3,
    }
}

fn size_of(label: &str) -> Option<AudioSize> {
    match label {
        "sm" => Some(AudioSize::Sm),
        "default" => Some(AudioSize::Default),
        "lg" => Some(AudioSize::Lg),
        _ => None,
    }
}

fn zones_of(label: &str) -> Option<Vec<MeterZone>> {
    match label {
        "broadcast −20/−9" => Some(DEFAULT_ZONES.to_vec()),
        "tight −12/−3" => Some(vec![
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
        ]),
        "wide −30/−18" => Some(vec![
            MeterZone {
                from_db: f32::NEG_INFINITY,
                zone: MeterZoneName::Ok,
            },
            MeterZone {
                from_db: -30.0,
                zone: MeterZoneName::Warn,
            },
            MeterZone {
                from_db: -18.0,
                zone: MeterZoneName::Clip,
            },
        ]),
        _ => None,
    }
}

fn ballistics_of(label: &str) -> Option<BallisticsPreset> {
    match label {
        "peak" => Some(BallisticsPreset::Peak),
        "vu" => Some(BallisticsPreset::Vu),
        "instant" => Some(BallisticsPreset::Instant),
        _ => None,
    }
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let count = channel_count(p.choice("channels"));
    let specs = &CHANNELS[..count];
    let mixer_state = use_mixer(
        ctx.id("state"),
        MixerOptions {
            channels: specs
                .iter()
                .map(|channel| MixerChannelInit::new(channel.id))
                .collect(),
            ..Default::default()
        },
        window,
        cx,
    );
    let state: MixerState = mixer_state.read(cx).state.clone();
    let demo_channels: Vec<DemoMixerChannel> = specs
        .iter()
        .map(|channel| {
            DemoMixerChannel::new(
                channel.id,
                DemoSignalOptions {
                    channels: channel.channels,
                    kind: channel.kind,
                    seed: channel.seed,
                    ..Default::default()
                },
            )
        })
        .collect();
    let demo = use_demo_mixer(ctx.id("demo"), &demo_channels, &state, window, cx);

    let strips: Vec<ChannelStrip> = specs
        .iter()
        .filter_map(|channel| {
            let source = demo.source(channel.id)?;
            let lane = state.channel(channel.id)?.clone();
            Some(input_strip(
                ctx,
                &mixer_state,
                channel,
                &lane,
                state.is_dimmed(channel.id),
                state.is_audible(channel.id),
                source,
            ))
        })
        .collect();

    let orientation = if p.choice("orientation") == "vertical" {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    };
    let scrollable = p.bool("scrollable");
    let mut channels = MixerChannels::new().scrollable(scrollable);
    if scrollable && orientation == Orientation::Horizontal {
        channels = channels.max_h(px(420.0));
    }
    channels = channels.children(strips);

    let mut mixer = Mixer::new(ctx.id("mixer"))
        .w_full()
        .orientation(orientation)
        .min_db(p.num("min_db"))
        .max_db(p.num("max_db"))
        .disabled(p.bool("disabled"));
    if let Some(size) = size_of(p.choice("size")) {
        mixer = mixer.size(size);
    }
    if let Some(zones) = zones_of(p.choice("zones")) {
        mixer = mixer.zones(zones);
    }
    if let Some(ballistics) = ballistics_of(p.choice("ballistics")) {
        mixer = mixer.ballistics(ballistics);
    }
    if orientation == Orientation::Vertical {
        mixer = mixer.h(px(440.0));
    }
    if p.bool("show_header") {
        mixer = mixer.child(header(p, ctx, &mixer_state));
    }
    mixer = mixer.child(channels);
    if p.bool("show_separator") {
        mixer = mixer.child(MixerSeparator::new());
    }
    if p.bool("show_master") {
        let audible =
            !state.master.muted && specs.iter().any(|channel| state.is_audible(channel.id));
        mixer = mixer.child(MixerMaster::new().child(master_strip(
            ctx,
            &mixer_state,
            &state,
            demo.master,
            audible,
        )));
    }
    if p.bool("show_empty") {
        mixer = mixer.child(MixerEmpty::new().child(p.text("empty")));
    }
    mixer.into_any_element()
}

fn header(p: &Props, ctx: &Ctx, mixer: &Entity<MixerStore>) -> MixerHeader {
    let mut header = MixerHeader::new().child(MixerTitle::new(p.text("title")));
    if p.bool("show_actions") {
        let mixer = mixer.clone();
        let log = ctx.clone();
        header = header.child(
            MixerActions::new().child(
                Button::new(ctx.id("reset"))
                    .child("Reset")
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| {
                        mixer.update(cx, |state, cx| {
                            state.reset();
                            cx.notify();
                        });
                        log.log("reset", "MixerState", cx);
                    }),
            ),
        );
    }
    header
}

fn input_strip(
    ctx: &Ctx,
    mixer: &Entity<MixerStore>,
    channel: &Channel,
    lane: &audiocn::signal::mixer::MixerChannelState,
    dimmed: bool,
    audible: bool,
    source: FrameSource<MeterFrame>,
) -> ChannelStrip {
    let id = channel.id;
    let title = channel.title;
    let gain = ctx.clone();
    let mute = ctx.clone();
    let solo = ctx.clone();
    let gain_mixer = mixer.clone();
    let mute_mixer = mixer.clone();
    let solo_mixer = mixer.clone();
    let mut meter = LevelMeter::new(ctx.id(&format!("{id}/meter")))
        .aria_label(format!("{title} level"))
        .source(source);
    if !audible {
        meter = meter.ballistics(BallisticsPreset::Instant);
    }
    ChannelStrip::new(ctx.id(id))
        .config(ctx.config())
        .dimmed(dimmed)
        .muted(lane.muted)
        .solo(lane.solo)
        .child(
            ChannelStripHeader::new()
                .child(ChannelStripIcon::new().child(Icon::new(channel.icon)))
                .child(ChannelStripText::new().child(ChannelStripTitle::new(title))),
        )
        .meter(meter)
        .fader(
            Fader::new(ctx.id(&format!("{id}/fader")))
                .aria_label(format!("{title} volume"))
                .on_value_change(move |db, _, cx| {
                    gain_mixer.update(cx, |state, cx| {
                        state.set_gain(id, db);
                        cx.notify();
                    });
                    gain.log("set_gain", format!("{id} {db:.1} dB"), cx);
                })
                .value(lane.gain_db),
        )
        .child(ChannelStripValue::new().child(format_db(lane.gain_db, FormatDbOptions::default())))
        .controls([
            MuteToggle::new(ctx.id(&format!("{id}/mute")))
                .child("M")
                .on_pressed_change(move |pressed, _, cx| {
                    mute_mixer.update(cx, |state, cx| {
                        state.set_muted(id, pressed);
                        cx.notify();
                    });
                    mute.log("set_muted", format!("{id} {pressed}"), cx);
                })
                .pressed(lane.muted),
            SoloToggle::new(ctx.id(&format!("{id}/solo")))
                .child("S")
                .on_pressed_change(move |pressed, _, cx| {
                    solo_mixer.update(cx, |state, cx| {
                        state.set_solo(id, pressed, false);
                        cx.notify();
                    });
                    solo.log("set_solo", format!("{id} {pressed}"), cx);
                })
                .pressed(lane.solo),
        ])
}

fn master_strip(
    ctx: &Ctx,
    mixer: &Entity<MixerStore>,
    state: &MixerState,
    source: FrameSource<MeterFrame>,
    audible: bool,
) -> ChannelStrip {
    let gain = ctx.clone();
    let mute = ctx.clone();
    let gain_mixer = mixer.clone();
    let mute_mixer = mixer.clone();
    let mut meter = LevelMeter::new(ctx.id("master/meter"))
        .aria_label("Master level")
        .channel_count(2)
        .source(source);
    if !audible {
        meter = meter.ballistics(BallisticsPreset::Instant);
    }
    ChannelStrip::new(ctx.id("master"))
        .config(ctx.config())
        .muted(state.master.muted)
        .variant(ChannelStripVariant::Master)
        .child(
            ChannelStripHeader::new()
                .child(ChannelStripIcon::new().child(Icon::new(IconName::SpeakerHigh)))
                .child(ChannelStripTitle::new("Master")),
        )
        .meter(meter)
        .fader(
            Fader::new(ctx.id("master/fader"))
                .aria_label("Master volume")
                .on_value_change(move |db, _, cx| {
                    gain_mixer.update(cx, |state, cx| {
                        state.set_master_gain(db);
                        cx.notify();
                    });
                    gain.log("set_master_gain", format!("{db:.1} dB"), cx);
                })
                .value(state.master.gain_db),
        )
        .child(
            ChannelStripValue::new()
                .child(format_db(state.master.gain_db, FormatDbOptions::default())),
        )
        .controls([MuteToggle::new(ctx.id("master/mute"))
            .child("M")
            .on_pressed_change(move |pressed, _, cx| {
                mute_mixer.update(cx, |state, cx| {
                    state.set_master_muted(pressed);
                    cx.notify();
                });
                mute.log("set_master_muted", format!("{pressed}"), cx);
            })
            .pressed(state.master.muted)])
}
