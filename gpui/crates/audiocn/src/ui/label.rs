//! Port of shadcn `components/ui/label.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

/// A form label: `flex items-center gap-2 text-sm leading-none font-medium`.
///
/// ```ignore
/// Label::new("Input device")
/// ```
#[derive(IntoElement)]
pub struct Label {
    children: Vec<AnyElement>,
    style: StyleRefinement,
    disabled: bool,
}

impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self::empty().child(text.into())
    }

    /// A label without text (add children with `.child`).
    pub fn empty() -> Self {
        Self {
            children: Vec::new(),
            style: StyleRefinement::default(),
            disabled: false,
        }
    }

    /// `peer-disabled` / `group-data-[disabled=true]`: 50% opacity.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl ParentElement for Label {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Label {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(14.0))
            .line_height(px(14.0))
            .font_weight(FontWeight::MEDIUM)
            .when(self.disabled, |el| el.opacity(0.5))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
