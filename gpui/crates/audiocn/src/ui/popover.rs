//! Port of shadcn `components/ui/popover.tsx` (Base UI `Popover`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, IntoElement, ParentElement, Refineable,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, div, px,
};

use super::overlay::{
    Align, OpenHandler, Placement, Side, enter_animation, popup, trigger, use_overlay,
};
use super::styles::{TextSize, by_mode, parent_styled, radius_3xl, ring_shadow, shadow_lg};
use crate::theme::ActiveTheme;

/// A popover: a [`PopoverTrigger`]-like element that toggles a floating
/// [`PopoverContent`]. Closes on click outside and Escape. Controlled with
/// [`Popover::open`], uncontrolled with [`Popover::default_open`].
///
/// ```ignore
/// Popover::new("volume")
///     .trigger(Button::new("btn").variant(ButtonVariant::Outline).child("Volume"))
///     .content(PopoverContent::new().align(Align::Start).child(...))
/// ```
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenHandler>,
    trigger: Option<AnyElement>,
    content: Option<PopoverContent>,
    style: StyleRefinement,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: None,
            default_open: false,
            on_open_change: None,
            trigger: None,
            content: None,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    pub fn on_open_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(f));
        self
    }

    /// `PopoverTrigger`: usually a [`super::Button`] (it gets
    /// `aria-expanded` styling and Enter / Space toggling).
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn content(mut self, content: PopoverContent) -> Self {
        self.content = Some(content);
        self
    }
}

/// `className` applies to the trigger wrapper.
impl Styled for Popover {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctl = use_overlay(
            self.id.clone(),
            self.open,
            self.default_open,
            self.on_open_change,
            true,
            window,
            cx,
        );
        let mut root = trigger(
            self.id,
            &ctl,
            self.trigger.unwrap_or_else(|| div().into_any_element()),
            cx,
        );
        root.style().refine(&self.style);
        let open = ctl.is_open(cx);
        let anchor = ctl.state.read(cx).trigger_bounds;
        match (open, anchor, self.content) {
            (true, Some(anchor), Some(content)) => {
                let placement = Placement::new(
                    content.side,
                    content.align,
                    content.side_offset,
                    content.align_offset,
                );
                let opened_at = ctl.state.read(cx).opened_at;
                let (opacity, offset) = enter_animation(opened_at, 100.0, content.side, window);
                let surface = content
                    .surface(cx)
                    .opacity(opacity)
                    .relative()
                    .top(offset.y)
                    .left(offset.x);
                let layer = popup(&ctl, placement, anchor, surface, window, cx);
                root.child(layer)
            }
            _ => root,
        }
    }
}

/// The floating panel: `bg-popover w-72 rounded-3xl p-4 text-sm shadow-lg
/// ring-1 ring-foreground/5 flex flex-col gap-4`.
pub struct PopoverContent {
    side: Side,
    align: Align,
    side_offset: f32,
    align_offset: f32,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Default for PopoverContent {
    fn default() -> Self {
        Self {
            side: Side::Bottom,
            align: Align::Center,
            side_offset: 4.0,
            align_offset: 0.0,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl PopoverContent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Default 4.
    pub fn side_offset(mut self, offset: f32) -> Self {
        self.side_offset = offset;
        self
    }

    pub fn align_offset(mut self, offset: f32) -> Self {
        self.align_offset = offset;
        self
    }

    fn surface(self, cx: &App) -> Div {
        let t = cx.theme();
        let mut shadows = shadow_lg();
        shadows.push(ring_shadow(
            1.0,
            by_mode(t, t.foreground.opacity(0.05), t.foreground.opacity(0.1)),
        ));
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .w(px(288.0))
            .rounded(radius_3xl(t))
            .p(px(16.0))
            .text_sm_()
            .bg(t.popover)
            .text_color(t.popover_foreground)
            .font_family(t.font_sans.clone())
            .shadow(shadows)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
parent_styled!(PopoverContent);

/// `flex flex-col gap-1 text-sm`.
#[derive(IntoElement, Default)]
pub struct PopoverHeader {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl PopoverHeader {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(PopoverHeader);

impl RenderOnce for PopoverHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .text_sm_()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `text-base font-medium`.
#[derive(IntoElement)]
pub struct PopoverTitle {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl PopoverTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(PopoverTitle);

impl RenderOnce for PopoverTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_base_()
            .font_weight(FontWeight::MEDIUM)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `text-muted-foreground`.
#[derive(IntoElement)]
pub struct PopoverDescription {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl PopoverDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(PopoverDescription);

impl RenderOnce for PopoverDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_color(cx.theme().muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
