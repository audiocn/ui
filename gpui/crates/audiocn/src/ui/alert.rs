//! Port of shadcn `components/ui/alert.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::icons::Icon;
use super::styles::{TextSize, parent_styled, provide, radius_2xl, use_context};
use crate::theme::ActiveTheme;

/// `variant` of [`Alert`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertVariant {
    #[default]
    Default,
    Destructive,
}

/// A callout: optional leading icon, an [`AlertTitle`] and an
/// [`AlertDescription`], and an optional [`Alert::action`] in the top-right.
///
/// ```ignore
/// Alert::new().icon(IconName::Info)
///     .child(AlertTitle::new("Heads up"))
///     .child(AlertDescription::new("Your microphone is muted."))
/// ```
#[derive(IntoElement, Default)]
pub struct Alert {
    variant: AlertVariant,
    icon: Option<Icon>,
    action: Option<AnyElement>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Alert {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The leading `svg` (`size-4`, `translate-y-0.5`, spans both rows).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// `AlertAction`: absolutely positioned `top-2.5 right-3` (adds `pr-18`).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}
parent_styled!(Alert);

impl RenderOnce for Alert {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let fg = match self.variant {
            AlertVariant::Default => t.card_foreground,
            AlertVariant::Destructive => t.destructive,
        };
        let has_action = self.action.is_some();
        let mut el: Div = div()
            .relative()
            .flex()
            .w_full()
            .gap(px(10.0))
            .rounded(radius_2xl(t))
            .border_1()
            .border_color(t.border)
            .pl(px(16.0))
            .pr(px(if has_action { 72.0 } else { 16.0 }))
            .py(px(12.0))
            .text_left()
            .text_sm_()
            .bg(t.card)
            .text_color(fg)
            .when_some(self.icon, |el, icon| {
                el.child(div().mt(px(2.0)).child(icon.size(px(16.0))))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .children(self.children),
            )
            .when_some(self.action, |el, a| {
                el.child(div().absolute().top(px(10.0)).right(px(12.0)).child(a))
            });
        el.style().refine(&self.style);
        provide(self.variant, el)
    }
}

/// `font-medium`.
#[derive(IntoElement)]
pub struct AlertTitle {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AlertTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(AlertTitle);

impl RenderOnce for AlertTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .font_weight(FontWeight::MEDIUM)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `text-muted-foreground text-sm` (`text-destructive/90` in a destructive alert).
#[derive(IntoElement)]
pub struct AlertDescription {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AlertDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(AlertDescription);

impl RenderOnce for AlertDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let color = match use_context::<AlertVariant>() {
            Some(AlertVariant::Destructive) => t.destructive.opacity(0.9),
            _ => t.muted_foreground,
        };
        let mut el: Div = div().text_sm_().text_color(color).children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `absolute top-2.5 right-3`: pass to [`Alert::action`] or use standalone.
#[derive(IntoElement, Default)]
pub struct AlertAction {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AlertAction {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(AlertAction);

impl RenderOnce for AlertAction {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div().children(self.children);
        el.style().refine(&self.style);
        el
    }
}
