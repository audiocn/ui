//! Port of shadcn `components/ui/card.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::styles::{
    TextSize, by_mode, parent_styled, provide, radius_4xl, ring_shadow, shadow_sm, use_context,
};
use crate::theme::ActiveTheme;

/// `size` of [`Card`]: sets `--card-spacing` (20px / 16px).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardSize {
    #[default]
    Default,
    Sm,
}

#[derive(Clone, Copy)]
struct CardSpacing(f32);

fn card_spacing() -> f32 {
    use_context::<CardSpacing>().map_or(20.0, |s| s.0)
}

fn card_radius(cx: &App) -> gpui::Pixels {
    radius_4xl(cx.theme()).min(px(24.0))
}

/// A card: `bg-card rounded-[min(var(--radius-4xl),24px)] py-(--card-spacing)
/// shadow-sm ring-1 ring-foreground/5`, a flex column with `--card-spacing` gaps.
///
/// ```ignore
/// Card::new()
///     .child(CardHeader::new().child(CardTitle::new("Mixer")).action(Button::new("x")))
///     .child(CardContent::new().child(...))
/// ```
#[derive(IntoElement, Default)]
pub struct Card {
    size: CardSize,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Card {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(mut self, size: CardSize) -> Self {
        self.size = size;
        self
    }
}
parent_styled!(Card);

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let spacing = match self.size {
            CardSize::Default => 20.0,
            CardSize::Sm => 16.0,
        };
        let mut shadows = shadow_sm();
        shadows.insert(
            0,
            ring_shadow(
                1.0,
                by_mode(t, t.foreground.opacity(0.05), t.foreground.opacity(0.1)),
            ),
        );
        let mut el: Div = div()
            .flex()
            .flex_col()
            .gap(px(spacing))
            .overflow_hidden()
            .rounded(card_radius(cx))
            .py(px(spacing))
            .text_sm_()
            .bg(t.card)
            .text_color(t.card_foreground)
            .shadow(shadows)
            .children(self.children);
        el.style().refine(&self.style);
        provide(CardSpacing(spacing), el)
    }
}

/// Card header: title + description on the left, an optional [`CardAction`]
/// on the right (`grid-cols-[1fr_auto]`).
#[derive(IntoElement, Default)]
pub struct CardHeader {
    action: Option<AnyElement>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    border_bottom: bool,
}

impl CardHeader {
    pub fn new() -> Self {
        Self::default()
    }

    /// The `CardAction` slot (top right, spans both rows).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// `border-b` (adds `pb-(--card-spacing)`).
    pub fn border_b(mut self, border: bool) -> Self {
        self.border_bottom = border;
        self
    }
}
parent_styled!(CardHeader);

impl RenderOnce for CardHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = card_spacing();
        let border = cx.theme().border;
        let column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(6.0))
            .children(self.children);
        let mut el: Div = div()
            .flex()
            .items_start()
            .gap(px(6.0))
            .px(px(s))
            .when(self.border_bottom, |el| {
                el.border_b_1().border_color(border).pb(px(s))
            })
            .child(column)
            .when_some(self.action, |el, a| el.child(a));
        el.style().refine(&self.style);
        el
    }
}

/// `font-heading text-base font-medium`.
#[derive(IntoElement)]
pub struct CardTitle {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl CardTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(CardTitle);

impl RenderOnce for CardTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_base_()
            .font_weight(FontWeight::MEDIUM)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `text-muted-foreground text-sm`.
#[derive(IntoElement)]
pub struct CardDescription {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl CardDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(CardDescription);

impl RenderOnce for CardDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_sm_()
            .text_color(cx.theme().muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// Top-right slot of a [`CardHeader`] (`self-start justify-self-end`). Pass it
/// to [`CardHeader::action`].
#[derive(IntoElement, Default)]
pub struct CardAction {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl CardAction {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(CardAction);

impl RenderOnce for CardAction {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div().flex_shrink_0().self_start().children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `px-(--card-spacing)`.
#[derive(IntoElement, Default)]
pub struct CardContent {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl CardContent {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(CardContent);

impl RenderOnce for CardContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div().px(px(card_spacing())).children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `flex items-center px-(--card-spacing)`.
#[derive(IntoElement, Default)]
pub struct CardFooter {
    children: Vec<AnyElement>,
    style: StyleRefinement,
    border_top: bool,
}

impl CardFooter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `border-t` (adds `pt-(--card-spacing)`).
    pub fn border_t(mut self, border: bool) -> Self {
        self.border_top = border;
        self
    }
}
parent_styled!(CardFooter);

impl RenderOnce for CardFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = card_spacing();
        let border = cx.theme().border;
        let mut el: Div = div()
            .flex()
            .items_center()
            .px(px(s))
            .when(self.border_top, |el| {
                el.border_t_1().border_color(border).pt(px(s))
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
