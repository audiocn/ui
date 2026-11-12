//! Gallery page for `volume-control`. One variant per docs example:
//! `components/examples/volume-control-demo.tsx`,
//! `components/examples/volume-control-popover.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, px};

use audiocn::ui::{Button, ButtonSize, ButtonVariant, Popover, PopoverContent, Side};

use audiocn::ui::icons::{Icon, IconName};
use audiocn::{
    Orientation, VolumeControl, VolumeControlMute, VolumeControlSlider, VolumeControlValue,
    VolumeLevel,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "volume-control",
    title: "Volume Control",
    description: "A simple volume slider with a mute button, for players.",
    variants: &["default", "popover"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(),
        "popover" => popover(window, cx),
        _ => return None,
    })
}

/// `volume-control-demo.tsx`: the speaker icon follows the level.
fn demo() -> AnyElement {
    VolumeControl::new(id("volume-control", "default"))
        .w_full()
        .max_w(px(320.0))
        .default_value(0.6)
        .child(VolumeControlMute::new().content(|state| {
            Icon::new(match state.level {
                VolumeLevel::Muted => IconName::SpeakerX,
                VolumeLevel::Low => IconName::SpeakerNone,
                VolumeLevel::Medium => IconName::SpeakerLow,
                VolumeLevel::High => IconName::SpeakerHigh,
            })
            .into_any_element()
        }))
        .child(VolumeControlSlider::new())
        .child(VolumeControlValue::new())
        .into_any_element()
}

/// `volume-control-popover.tsx`: a speaker button that opens a vertical slider.
fn popover(window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state("volume-control/popover/state", cx, |_, _| (0.8_f32, false));
    let (volume, muted) = *state.read(cx);
    let speaker = |silent: bool| {
        if silent {
            IconName::SpeakerX
        } else {
            IconName::SpeakerHigh
        }
    };
    let (on_volume, on_muted) = (state.clone(), state);
    Popover::new(id("volume-control", "popover"))
        .trigger(
            Button::new("volume-control/popover/trigger")
                .aria_label("Volume")
                .size(ButtonSize::Icon)
                .variant(ButtonVariant::Outline)
                .icon(speaker(muted || volume == 0.0)),
        )
        .content(
            PopoverContent::new().w_auto().side(Side::Top).child(
                VolumeControl::new("volume-control/popover/control")
                    .muted(muted)
                    .on_muted_change(move |m, _, cx| {
                        on_muted.update(cx, |s, cx| {
                            s.1 = m;
                            cx.notify();
                        })
                    })
                    .on_value_change(move |v, _, cx| {
                        on_volume.update(cx, |s, cx| {
                            s.0 = v;
                            cx.notify();
                        })
                    })
                    .orientation(Orientation::Vertical)
                    .value(volume)
                    .child(VolumeControlMute::new().child(Icon::new(speaker(muted))))
                    .child(VolumeControlSlider::new()),
            ),
        )
        .into_any_element()
}
