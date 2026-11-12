//! Gallery page for `channel-toggle`. One variant per docs example:
//! `components/examples/channel-toggle-demo.tsx`,
//! `components/examples/channel-toggle-variants.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::theme::ActiveTheme;
use audiocn::ui::icons::IconName;
use audiocn::{
    ChannelToggle, ChannelToggleSize, ChannelToggleVariant, MonitorToggle, MuteToggle, SoloToggle,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "channel-toggle",
    title: "Channel Toggle",
    description: "Mute, solo and monitor toggle buttons with their own pressed colours.",
    variants: &["default", "variants"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "variants" => variants(cx),
        _ => return None,
    })
}

/// `channel-toggle-demo.tsx`: controlled mute (on), solo and monitor.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("channel-toggle", "default");
    let state = window.use_keyed_state(base.clone(), cx, |_, _| (true, false, false));
    let (muted, solo, monitor) = *state.read(cx);
    let (s1, s2, s3) = (state.clone(), state.clone(), state);
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            MuteToggle::new(format!("{base}/mute"))
                .pressed(muted)
                .on_pressed_change(move |v, _, cx| {
                    s1.update(cx, |s, cx| {
                        s.0 = v;
                        cx.notify();
                    })
                })
                .child("M"),
        )
        .child(
            SoloToggle::new(format!("{base}/solo"))
                .pressed(solo)
                .on_pressed_change(move |v, _, cx| {
                    s2.update(cx, |s, cx| {
                        s.1 = v;
                        cx.notify();
                    })
                })
                .child("S"),
        )
        .child(
            MonitorToggle::new(format!("{base}/monitor"))
                .pressed(monitor)
                .size(ChannelToggleSize::Icon)
                .on_pressed_change(move |v, _, cx| {
                    s3.update(cx, |s, cx| {
                        s.2 = v;
                        cx.notify();
                    })
                })
                .icon(IconName::Headphones),
        )
        .into_any_element()
}

/// `channel-toggle-variants.tsx`: default, outline and ghost rows.
fn variants(cx: &mut App) -> AnyElement {
    let base = id("channel-toggle", "variants");
    let muted_fg = cx.theme().muted_foreground;
    let mono = cx.theme().font_mono.clone();
    let rows = [
        ("default", ChannelToggleVariant::Default),
        ("outline", ChannelToggleVariant::Outline),
        ("ghost", ChannelToggleVariant::Ghost),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .children(rows.into_iter().map(|(name, variant)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    MuteToggle::new(format!("{base}/{name}/muted"))
                        .default_pressed(true)
                        .variant(variant)
                        .icon(IconName::MicrophoneSlash)
                        .child("Muted"),
                )
                .child(
                    MuteToggle::new(format!("{base}/{name}/mute"))
                        .variant(variant)
                        .child("Mute"),
                )
                .child(
                    ChannelToggle::new(format!("{base}/{name}/arm"))
                        .aria_label("Record arm")
                        .default_pressed(true)
                        .variant(variant)
                        .child("R"),
                )
                .child(
                    div()
                        .text_color(muted_fg)
                        .font_family(mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(name),
                )
        }))
        .into_any_element()
}
