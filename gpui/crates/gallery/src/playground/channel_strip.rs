//! Playground page for `ChannelStrip`: every prop, every part, live fader and toggles.

use audiocn::audio::decibels::{FormatDbOptions, format_db};
use audiocn::ui::icons::{Icon, IconName};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    AudioSize, ChannelStrip, ChannelStripActions, ChannelStripControls, ChannelStripDescription,
    ChannelStripHeader, ChannelStripIcon, ChannelStripNotice, ChannelStripNoticeVariant,
    ChannelStripStatus, ChannelStripStatusTone, ChannelStripText, ChannelStripTitle,
    ChannelStripValue, ChannelStripVariant, Fader, LevelMeter, MonitorToggle, MuteToggle,
    Orientation, SoloToggle,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "channel-strip",
    component: "ChannelStrip",
    source: SourceUse::Meter,
    controls,
    render,
};

const ORIENTATIONS: &[(&str, &str)] = &[
    ("unset", ""),
    ("horizontal", "Orientation::Horizontal"),
    ("vertical", "Orientation::Vertical"),
];

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

const VARIANTS: &[(&str, &str)] = &[
    ("default", "ChannelStripVariant::Default"),
    ("card", "ChannelStripVariant::Card"),
    ("ghost", "ChannelStripVariant::Ghost"),
    ("master", "ChannelStripVariant::Master"),
];

const STATUS_TONES: &[(&str, &str)] = &[
    ("follow mute", ""),
    ("default", "ChannelStripStatusTone::Default"),
    ("live", "ChannelStripStatusTone::Live"),
    ("muted", "ChannelStripStatusTone::Muted"),
    ("warning", "ChannelStripStatusTone::Warning"),
    ("error", "ChannelStripStatusTone::Error"),
];

const NOTICE_VARIANTS: &[(&str, &str)] = &[
    ("default", "ChannelStripNoticeVariant::Default"),
    ("warning", "ChannelStripNoticeVariant::Warning"),
    ("destructive", "ChannelStripNoticeVariant::Destructive"),
];

fn controls() -> Vec<Control> {
    vec![
        Control::bool("controlled", "Controlled", true)
            .group("Fader")
            .no_code()
            .remount()
            .help("Off: fader and toggles use default_* and keep their own state."),
        Control::num("gain", "Gain", 0.0, -60.0, 6.0, 0.1)
            .group("Fader")
            .no_code()
            .help("Controlled fader value, in dB."),
        Control::num("default_gain", "Default gain", 0.0, -60.0, 6.0, 0.1)
            .group("Fader")
            .no_code()
            .remount(),
        Control::num("min", "Min dB", -60.0, -120.0, 0.0, 1.0)
            .group("Fader")
            .no_code(),
        Control::num("max", "Max dB", 6.0, 0.0, 24.0, 1.0)
            .group("Fader")
            .no_code(),
        Control::choice("orientation", "Orientation", 0, ORIENTATIONS).group("Props"),
        Control::choice("variant", "Variant", 0, VARIANTS).group("Props"),
        Control::choice("size", "Size", 0, SIZES).group("Props"),
        Control::bool("muted", "Muted", false).group("Props"),
        Control::bool("solo", "Solo", false).group("Props"),
        Control::bool("dimmed", "Dimmed", false)
            .group("Props")
            .help("Silenced by another channel's solo."),
        Control::bool("selected", "Selected", false).group("Props"),
        Control::bool("disabled", "Disabled", false).group("Props"),
        Control::color("accent", "Accent").group("Props"),
        Control::px("header_width", "Header width", 192.0, 80.0, 420.0, 4.0)
            .group("Props")
            .help("Wide rows only (stage wider than about 576px)."),
        Control::bool("show_icon", "Icon", true)
            .group("Header")
            .no_code(),
        Control::bool("show_title", "Title", true)
            .group("Header")
            .no_code(),
        Control::text("title", "Title text", "Microphone")
            .group("Header")
            .no_code(),
        Control::bool("show_description", "Description", true)
            .group("Header")
            .no_code(),
        Control::text("description", "Description text", "Shure MV7+")
            .group("Header")
            .no_code(),
        Control::bool("show_status", "Status", true)
            .group("Header")
            .no_code(),
        Control::choice("status_tone", "Status tone", 0, STATUS_TONES)
            .group("Header")
            .no_code()
            .help("Follow mute switches Live / Muted with the mute toggle."),
        Control::text("status", "Status text", "Live")
            .group("Header")
            .no_code()
            .help("Used when the tone is not \"follow mute\"."),
        Control::bool("show_actions", "Actions", true)
            .group("Header")
            .no_code(),
        Control::bool("show_meter", "Meter", true)
            .group("Meter")
            .no_code()
            .help("Fed by the source picker (ctx.meter)."),
        Control::bool("show_fader", "Fader", true)
            .group("Fader")
            .no_code(),
        Control::bool("show_value", "Value", true)
            .group("Value")
            .no_code(),
        Control::bool("show_controls", "Controls", true)
            .group("Controls")
            .no_code(),
        Control::bool("show_mute", "Mute", true)
            .group("Controls")
            .no_code(),
        Control::bool("show_solo", "Solo", true)
            .group("Controls")
            .no_code(),
        Control::bool("monitor", "Monitor", false)
            .group("Controls")
            .no_code(),
        Control::bool("show_monitor", "Monitor toggle", true)
            .group("Controls")
            .no_code(),
        Control::bool("show_notice", "Notice", false)
            .group("Notice")
            .no_code(),
        Control::choice("notice_variant", "Notice variant", 1, NOTICE_VARIANTS)
            .group("Notice")
            .no_code(),
        Control::text("notice", "Notice text", "Paused to prevent an echo.")
            .group("Notice")
            .no_code(),
    ]
}

fn orientation_of(label: &str) -> Option<Orientation> {
    match label {
        "horizontal" => Some(Orientation::Horizontal),
        "vertical" => Some(Orientation::Vertical),
        _ => None,
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

fn variant_of(label: &str) -> ChannelStripVariant {
    match label {
        "card" => ChannelStripVariant::Card,
        "ghost" => ChannelStripVariant::Ghost,
        "master" => ChannelStripVariant::Master,
        _ => ChannelStripVariant::Default,
    }
}

fn status_tone(label: &str, muted: bool) -> (ChannelStripStatusTone, Option<&'static str>) {
    match label {
        "follow mute" if muted => (ChannelStripStatusTone::Muted, Some("Muted")),
        "follow mute" => (ChannelStripStatusTone::Live, Some("Live")),
        "default" => (ChannelStripStatusTone::Default, None),
        "muted" => (ChannelStripStatusTone::Muted, None),
        "warning" => (ChannelStripStatusTone::Warning, None),
        "error" => (ChannelStripStatusTone::Error, None),
        _ => (ChannelStripStatusTone::Live, None),
    }
}

fn notice_variant(label: &str) -> ChannelStripNoticeVariant {
    match label {
        "destructive" => ChannelStripNoticeVariant::Destructive,
        "default" => ChannelStripNoticeVariant::Default,
        _ => ChannelStripNoticeVariant::Warning,
    }
}

fn notice_icon(variant: ChannelStripNoticeVariant) -> IconName {
    match variant {
        ChannelStripNoticeVariant::Destructive => IconName::Warning,
        ChannelStripNoticeVariant::Warning => IconName::Warning,
        ChannelStripNoticeVariant::Default => IconName::Waveform,
    }
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let gain = if controlled {
        p.num("gain")
    } else {
        p.num("default_gain")
    };
    let mut strip = ChannelStrip::new(ctx.id("strip"))
        .config(ctx.config())
        .variant(variant_of(p.choice("variant")))
        .muted(p.bool("muted"))
        .solo(p.bool("solo"))
        .dimmed(p.bool("dimmed"))
        .selected(p.bool("selected"))
        .header_width(px(p.num("header_width")));
    if let Some(orientation) = orientation_of(p.choice("orientation")) {
        strip = strip.orientation(orientation);
    }
    if let Some(size) = size_of(p.choice("size")) {
        strip = strip.size(size);
    }
    if p.bool("disabled") {
        strip = strip.disabled(true);
    }
    if let Some(accent) = p.color("accent", cx) {
        strip = strip.accent(accent);
    }

    if let Some(header) = header(p, ctx) {
        strip = strip.child(header);
    }
    if p.bool("show_meter") {
        strip = strip.meter(
            LevelMeter::new(ctx.id("meter"))
                .aria_label("Channel level")
                .channel_count(ctx.channels())
                .source(ctx.meter()),
        );
    }
    if p.bool("show_fader") {
        strip = strip.fader(fader(p, ctx, controlled));
    }
    if p.bool("show_value") {
        strip = strip
            .child(ChannelStripValue::new().child(format_db(gain, FormatDbOptions::default())));
    }
    if p.bool("show_controls") {
        strip = strip.child(controls_part(p, ctx, controlled));
    }
    if p.bool("show_notice") {
        let variant = notice_variant(p.choice("notice_variant"));
        strip = strip.child(
            ChannelStripNotice::new()
                .variant(variant)
                .icon(notice_icon(variant))
                .child(div().min_w_0().flex_1().child(p.text("notice"))),
        );
    }

    let vertical = p.choice("orientation") == "vertical";
    let element = strip.into_any_element();
    if vertical {
        div()
            .w_full()
            .flex()
            .justify_center()
            .child(element)
            .into_any_element()
    } else {
        element
    }
}

fn header(p: &Props, ctx: &Ctx) -> Option<ChannelStripHeader> {
    let any = p.bool("show_icon")
        || p.bool("show_title")
        || p.bool("show_description")
        || p.bool("show_status")
        || p.bool("show_actions");
    if !any {
        return None;
    }
    let mut header = ChannelStripHeader::new();
    if p.bool("show_icon") {
        header = header.child(ChannelStripIcon::new().child(Icon::new(IconName::Microphone)));
    }
    if p.bool("show_title") || p.bool("show_description") {
        let mut text = ChannelStripText::new();
        if p.bool("show_title") {
            text = text.child(ChannelStripTitle::new(p.text("title")));
        }
        if p.bool("show_description") {
            text = text.child(ChannelStripDescription::new(p.text("description")));
        }
        header = header.child(text);
    }
    if p.bool("show_status") {
        let (tone, follow) = status_tone(p.choice("status_tone"), p.bool("muted"));
        let label = match follow {
            Some(text) => text.into(),
            None => p.text("status"),
        };
        header = header.child(ChannelStripStatus::new().tone(tone).child(label));
    }
    if p.bool("show_actions") {
        let log = ctx.clone();
        header = header.child(
            ChannelStripActions::new().child(
                Button::new(ctx.id("action"))
                    .aria_label("Channel actions")
                    .icon(IconName::DotsThree)
                    .size(ButtonSize::IconXs)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| log.log("on_click", "header action", cx)),
            ),
        );
    }
    Some(header)
}

fn fader(p: &Props, ctx: &Ctx, controlled: bool) -> Fader {
    let change = ctx.clone();
    let commit = ctx.clone();
    let mut fader = Fader::new(ctx.id("fader"))
        .aria_label("Channel volume")
        .min(p.num("min"))
        .max(p.num("max"))
        .on_value_change(move |db, _, cx| {
            if controlled {
                change.set_num("gain", db, cx);
            }
            change.log("on_value_change", format!("{db:.1} dB"), cx);
        })
        .on_value_committed(move |db, _, cx| {
            commit.log("on_value_committed", format!("{db:.1} dB"), cx);
        });
    fader = if controlled {
        fader.value(p.num("gain"))
    } else {
        fader.default_value(p.num("default_gain"))
    };
    fader
}

fn controls_part(p: &Props, ctx: &Ctx, controlled: bool) -> ChannelStripControls {
    let mut part = ChannelStripControls::new();
    if p.bool("show_mute") {
        part = part.child(toggle(
            ctx,
            "mute",
            "muted",
            "M",
            p.bool("muted"),
            controlled,
            MuteToggle::new(ctx.id("mute")),
        ));
    }
    if p.bool("show_solo") {
        part = part.child(toggle(
            ctx,
            "solo",
            "solo",
            "S",
            p.bool("solo"),
            controlled,
            SoloToggle::new(ctx.id("solo")),
        ));
    }
    if p.bool("show_monitor") {
        part = part.child(toggle(
            ctx,
            "monitor",
            "monitor",
            "Mon",
            p.bool("monitor"),
            controlled,
            MonitorToggle::new(ctx.id("monitor")),
        ));
    }
    part
}

fn toggle(
    ctx: &Ctx,
    name: &'static str,
    key: &'static str,
    label: &'static str,
    pressed: bool,
    controlled: bool,
    toggle: audiocn::ChannelToggle,
) -> audiocn::ChannelToggle {
    let change = ctx.clone();
    let toggle = toggle.child(label).on_pressed_change(move |next, _, cx| {
        if controlled {
            change.set_bool(key, next, cx);
        }
        change.log("on_pressed_change", format!("{name} {next}"), cx);
    });
    if controlled {
        toggle.pressed(pressed)
    } else {
        toggle.default_pressed(pressed)
    }
}
