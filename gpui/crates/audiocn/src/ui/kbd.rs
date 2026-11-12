//! Port of shadcn `components/ui/kbd.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::styles::{TextSize, by_mode, radius_lg, use_context};
use crate::theme::ActiveTheme;

/// Marker context set by `TooltipContent` (`in-data-[slot=tooltip-content]`).
#[derive(Clone, Copy)]
pub(crate) struct InTooltip;

/// A keyboard key cap.
///
/// ```ignore
/// KbdGroup::new().child(Kbd::new("⌘")).child(Kbd::new("K"))
/// ```
#[derive(IntoElement)]
pub struct Kbd {
    children: Vec<AnyElement>,
    style: StyleRefinement,
    in_tooltip: bool,
}

impl Kbd {
    /// A key with a text label (add more children with `.child`).
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            children: vec![gpui::IntoElement::into_any_element(label.into())],
            style: StyleRefinement::default(),
            in_tooltip: false,
        }
    }

    /// An empty key (e.g. for an icon child).
    pub fn empty() -> Self {
        Self {
            children: Vec::new(),
            style: StyleRefinement::default(),
            in_tooltip: false,
        }
    }

    /// Style for use inside a tooltip (detected automatically inside `TooltipContent`).
    pub fn in_tooltip(mut self, in_tooltip: bool) -> Self {
        self.in_tooltip = in_tooltip;
        self
    }
}

impl ParentElement for Kbd {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Kbd {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let in_tooltip = self.in_tooltip || use_context::<InTooltip>().is_some();
        let (bg, fg) = if in_tooltip {
            (
                by_mode(t, t.background.opacity(0.2), t.background.opacity(0.1)),
                t.background,
            )
        } else {
            (t.muted, t.muted_foreground)
        };
        let mut el: Div = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .h(px(20.0))
            .min_w(px(20.0))
            .gap(px(4.0))
            .px(px(4.0))
            .rounded(radius_lg(t))
            .bg(bg)
            .text_color(fg)
            .font_family(t.font_sans.clone())
            .text_xs_()
            .font_weight(FontWeight::MEDIUM)
            .when(in_tooltip, |el| el.relative())
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// A row of [`Kbd`]s (`inline-flex items-center gap-1`).
#[derive(IntoElement, Default)]
pub struct KbdGroup {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl KbdGroup {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ParentElement for KbdGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for KbdGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for KbdGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
