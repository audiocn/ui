//! Playground page for `Spectrum` (pending: add a control for every prop).

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "spectrum",
    component: "Spectrum",
    source: SourceUse::Visual,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    Vec::new()
}

fn render(_props: &Props, _ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .text_sm()
        .child("Spectrum: playground pending")
        .into_any_element()
}
