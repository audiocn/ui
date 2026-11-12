//! Playground page for `ChannelToggle`: tone, variant, size, pressed
//! (controlled and uncontrolled), the Mute / Solo / Monitor shortcuts and
//! text, icon or icon + text content.

use audiocn::ui::icons::IconName;
use audiocn::{
    ChannelToggle, ChannelToggleSize, ChannelToggleTone, ChannelToggleVariant, MonitorToggle,
    MuteToggle, SoloToggle,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "channel-toggle",
    component: "ChannelToggle",
    source: SourceUse::None,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    vec![
        Control::choice(
            "constructor",
            "Constructor",
            0,
            &[
                ("ChannelToggle::new", ""),
                ("MuteToggle::new", ""),
                ("SoloToggle::new", ""),
                ("MonitorToggle::new", ""),
            ],
        )
        .group("Component")
        .no_code()
        .remount()
        .help("The shortcuts preset tone and aria-label (tone below still overrides)."),
        Control::bool("demo_row", "Mute / Solo / Monitor row", false)
            .group("Component")
            .no_code()
            .help("channel-toggle-demo: three toggles sharing these props."),
        // Pressed
        Control::bool("controlled", "Controlled", true)
            .group("Pressed")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_pressed (remounts)."),
        Control::bool("pressed", "Pressed", true).group("Pressed"),
        Control::bool("default_pressed", "Default pressed", false)
            .group("Pressed")
            .remount(),
        // Look
        Control::choice(
            "tone",
            "Tone",
            0,
            &[
                ("unset (constructor's)", ""),
                ("neutral", "ChannelToggleTone::Neutral"),
                ("mute", "ChannelToggleTone::Mute"),
                ("solo", "ChannelToggleTone::Solo"),
                ("monitor", "ChannelToggleTone::Monitor"),
            ],
        )
        .group("Look")
        .help("The pressed colour."),
        Control::choice(
            "variant",
            "Variant",
            0,
            &[
                ("default", "ChannelToggleVariant::Default"),
                ("outline", "ChannelToggleVariant::Outline"),
                ("ghost", "ChannelToggleVariant::Ghost"),
            ],
        )
        .group("Look"),
        Control::choice(
            "size",
            "Size",
            0,
            &[
                ("default", "ChannelToggleSize::Default"),
                ("sm", "ChannelToggleSize::Sm"),
                ("lg", "ChannelToggleSize::Lg"),
                ("icon", "ChannelToggleSize::Icon"),
            ],
        )
        .group("Look"),
        Control::bool("disabled", "Disabled", false).group("Look"),
        Control::text("aria_label", "Aria label", "").group("Look"),
        // Content
        Control::choice(
            "content",
            "Content",
            0,
            &[("text", ""), ("icon", ""), ("icon + text", "")],
        )
        .group("Content")
        .no_code()
        .help("Use size \"icon\" for an icon-only square button."),
        Control::text("text", "Text", "M")
            .group("Content")
            .code(".child({})"),
        Control::choice(
            "icon",
            "Icon",
            0,
            &[
                ("MicrophoneSlash", "IconName::MicrophoneSlash"),
                ("Headphones", "IconName::Headphones"),
                ("SpeakerX", "IconName::SpeakerX"),
                ("Microphone", "IconName::Microphone"),
                ("Record", "IconName::Record"),
            ],
        )
        .group("Content"),
    ]
}

fn icon(p: &Props) -> IconName {
    match p.index("icon") {
        1 => IconName::Headphones,
        2 => IconName::SpeakerX,
        3 => IconName::Microphone,
        4 => IconName::Record,
        _ => IconName::MicrophoneSlash,
    }
}

/// One toggle with the shared props; `pressed` is its controlled state
/// (ignored when uncontrolled) and `on_pressed` stores it.
fn toggle(
    p: &Props,
    ctx: &Ctx,
    constructor: usize,
    part: &'static str,
    text: &str,
    pressed: bool,
    on_pressed: impl Fn(bool, &mut App) + 'static,
) -> ChannelToggle {
    let id = ctx.id(part);
    let mut t = match constructor {
        1 => MuteToggle::new(id),
        2 => SoloToggle::new(id),
        3 => MonitorToggle::new(id),
        _ => ChannelToggle::new(id),
    };
    t = match p.choice("tone") {
        "neutral" => t.tone(ChannelToggleTone::Neutral),
        "mute" => t.tone(ChannelToggleTone::Mute),
        "solo" => t.tone(ChannelToggleTone::Solo),
        "monitor" => t.tone(ChannelToggleTone::Monitor),
        _ => t,
    };
    t = t
        .variant(match p.choice("variant") {
            "outline" => ChannelToggleVariant::Outline,
            "ghost" => ChannelToggleVariant::Ghost,
            _ => ChannelToggleVariant::Default,
        })
        .size(match p.choice("size") {
            "sm" => ChannelToggleSize::Sm,
            "lg" => ChannelToggleSize::Lg,
            "icon" => ChannelToggleSize::Icon,
            _ => ChannelToggleSize::Default,
        })
        .disabled(p.bool("disabled"))
        .config(ctx.config());
    let aria = p.text("aria_label");
    if !aria.is_empty() {
        t = t.aria_label(aria);
    }
    let controlled = p.bool("controlled");
    t = if controlled {
        t.pressed(pressed)
    } else {
        t.default_pressed(p.bool("default_pressed"))
    };
    let c = ctx.clone();
    t = t.on_pressed_change(move |pressed, _, cx| {
        if controlled {
            on_pressed(pressed, cx);
        }
        c.log("on_pressed_change", format!("{part}: {pressed}"), cx);
    });
    match p.index("content") {
        1 => t.icon(icon(p)),
        2 => t.icon(icon(p)).child(text.to_string()),
        _ => t.child(text.to_string()),
    }
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let set = ctx.clone();
    let write_pressed = move |v: bool, cx: &mut App| set.set_bool("pressed", v, cx);
    if !p.bool("demo_row") {
        let text = p.text("text");
        return toggle(
            p,
            ctx,
            p.index("constructor"),
            "toggle",
            &text,
            p.bool("pressed"),
            write_pressed,
        )
        .into_any_element();
    }
    // `channel-toggle-demo.tsx`: mute follows the panel's `pressed`; solo
    // and monitor keep their own controlled state.
    let state = window.use_keyed_state(ctx.id("row-state"), cx, |_, _| (false, false));
    let (solo, monitor) = *state.read(cx);
    let (s1, s2) = (state.clone(), state);
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(toggle(
            p,
            ctx,
            1,
            "mute",
            "M",
            p.bool("pressed"),
            write_pressed,
        ))
        .child(toggle(p, ctx, 2, "solo", "S", solo, move |v, cx| {
            s1.update(cx, |s, cx| {
                s.0 = v;
                cx.notify();
            })
        }))
        .child(toggle(p, ctx, 3, "monitor", "", monitor, move |v, cx| {
            s2.update(cx, |s, cx| {
                s.1 = v;
                cx.notify();
            })
        }))
        .into_any_element()
}
