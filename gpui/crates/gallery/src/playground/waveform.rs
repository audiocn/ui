//! Playground page for `Waveform` (pending: add a control for every prop).

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "waveform",
    component: "Waveform",
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
        .child("Waveform: playground pending")
        .into_any_element()
}
