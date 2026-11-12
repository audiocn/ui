//! Port of shadcn `components/ui/badge.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Refineable,
    RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::icons::Icon;
use super::styles::{TextSize, by_mode, radius_2xl};
use crate::theme::ActiveTheme;

/// `variant` of [`Badge`] (`badgeVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Destructive,
    Outline,
    Ghost,
    Link,
}

/// A small pill label.
///
/// ```ignore
/// Badge::new().variant(BadgeVariant::Secondary).child("Live")
/// ```
#[derive(IntoElement, Default)]
pub struct Badge {
    variant: BadgeVariant,
    icon_start: Option<Icon>,
    icon_end: Option<Icon>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Badge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Leading icon (`data-icon="inline-start"`, `size-3`).
    pub fn icon_start(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_start = Some(icon.into());
        self
    }

    /// Trailing icon (`data-icon="inline-end"`, `size-3`).
    pub fn icon_end(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_end = Some(icon.into());
        self
    }
}

impl ParentElement for Badge {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Badge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let (bg, fg, border) = match self.variant {
            BadgeVariant::Default => (Some(t.primary), Some(t.primary_foreground), None),
            BadgeVariant::Secondary => (Some(t.secondary), Some(t.secondary_foreground), None),
            BadgeVariant::Destructive => (
                Some(by_mode(
                    t,
                    t.destructive.opacity(0.1),
                    t.destructive.opacity(0.2),
                )),
                Some(t.destructive),
                None,
            ),
            BadgeVariant::Outline => (None, Some(t.foreground), Some(t.border)),
            BadgeVariant::Ghost | BadgeVariant::Link => (None, None, None),
        };
        let ghost_hover = (self.variant == BadgeVariant::Ghost).then(|| {
            (
                by_mode(t, t.muted, t.muted.opacity(0.5)),
                t.muted_foreground,
            )
        });
        let link = self.variant == BadgeVariant::Link;
        let link_fg = t.primary;

        let mut el: Div = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .h(px(20.0))
            .gap(px(4.0))
            .overflow_hidden()
            .rounded(radius_2xl(t).min(px(10.0)))
            .border_1()
            .border_color(border.unwrap_or(gpui::transparent_black()))
            .pl(px(if self.icon_start.is_some() { 6.0 } else { 8.0 }))
            .pr(px(if self.icon_end.is_some() { 6.0 } else { 8.0 }))
            .py(px(2.0))
            .text_xs_()
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(fg, |el, fg| el.text_color(fg))
            .when(link, |el| el.text_color(link_fg).hover(|s| s.underline()))
            .when_some(ghost_hover, |el, (bg, fg)| {
                el.hover(move |s| s.bg(bg).text_color(fg))
            })
            .when_some(self.icon_start, |el, i| el.child(i.size(px(12.0))))
            .children(self.children)
            .when_some(self.icon_end, |el, i| el.child(i.size(px(12.0))));
        el.style().refine(&self.style);
        el
    }
}
