//! Playground page for `SoundPad` (pending: add a control for every prop).

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "sound-pad",
    component: "SoundPad",
    source: SourceUse::None,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    Vec::new()
}

fn render(_props: &Props, _ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .text_sm()
        .child("SoundPad: playground pending")
        .into_any_element()
}
