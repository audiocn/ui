//! Port of shadcn `components/ui/tooltip.tsx` (Base UI `Tooltip`).

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Div, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement,
    PathBuilder, Refineable, RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, anchored, canvas, deferred, div, point, prelude::FluentBuilder, px,
};

use super::kbd::InTooltip;
use super::overlay::{
    Align, OpenHandler, OverlayCtl, Placement, Side, enter_animation, measure_content,
    measure_trigger, use_overlay,
};
use super::styles::{parent_styled, provide, radius_xl};
use crate::theme::ActiveTheme;

/// A hover / focus tooltip around a trigger.
///
/// Opens after [`Tooltip::delay`] (0 by default, like shadcn's
/// `TooltipProvider`) on hover; closes when the pointer leaves or presses.
///
/// ```ignore
/// Tooltip::new("mute-tip")
///     .trigger(Button::new("mute").size(ButtonSize::Icon).icon(IconName::SpeakerX))
///     .content(TooltipContent::new().child("Mute").child(Kbd::new("M")))
/// ```
#[derive(IntoElement)]
pub struct Tooltip {
    id: ElementId,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
    delay: Duration,
    disabled: bool,
    trigger: Option<AnyElement>,
    content: Option<TooltipContent>,
    style: StyleRefinement,
}

impl Tooltip {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: None,
            on_open_change: None,
            delay: Duration::ZERO,
            disabled: false,
            trigger: None,
            content: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn on_open_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(f));
        self
    }

    /// Hover delay before opening (`delay`, ms). Default 0.
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay = Duration::from_millis(ms);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `TooltipTrigger`.
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn content(mut self, content: TooltipContent) -> Self {
        self.content = Some(content);
        self
    }
}

/// Styles the trigger wrapper.
impl Styled for Tooltip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn schedule_open(ctl: &OverlayCtl, delay: Duration, window: &mut Window, cx: &mut App) {
    let generation = ctl.state.update(cx, |s, _| {
        s.hover_generation += 1;
        s.hover_generation
    });
    if delay.is_zero() {
        ctl.set_open(true, window, cx);
        return;
    }
    let ctl = ctl.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(delay).await;
            let _ = cx.update(|window, cx| {
                if ctl.state.read(cx).hover_generation == generation {
                    ctl.set_open(true, window, cx);
                }
            });
        })
        .detach();
}

fn cancel(ctl: &OverlayCtl, window: &mut Window, cx: &mut App) {
    ctl.state.update(cx, |s, _| s.hover_generation += 1);
    ctl.set_open(false, window, cx);
}

impl RenderOnce for Tooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctl = use_overlay(
            self.id.clone(),
            self.open,
            false,
            self.on_open_change,
            false,
            window,
            cx,
        );
        let hover = ctl.clone();
        let down = ctl.clone();
        let measure = ctl.state.clone();
        let delay = self.delay;
        let disabled = self.disabled;
        let mut root = div()
            .id(self.id)
            .relative()
            .flex()
            .child(self.trigger.unwrap_or_else(|| div().into_any_element()))
            .child(measure_trigger(measure))
            .when(!disabled, |el| {
                el.on_hover(move |hovered, window, cx| {
                    if *hovered {
                        schedule_open(&hover, delay, window, cx);
                    } else {
                        cancel(&hover, window, cx);
                    }
                })
                .on_any_mouse_down(move |_, window, cx| cancel(&down, window, cx))
            });
        root.style().refine(&self.style);

        let anchor = ctl.state.read(cx).trigger_bounds;
        let (true, Some(anchor), Some(content)) =
            (ctl.is_open(cx) && !disabled, anchor, self.content)
        else {
            return root;
        };
        let t = cx.theme();
        let placement = Placement::new(
            content.side,
            content.align,
            content.side_offset,
            content.align_offset,
        )
        .flipped(
            anchor,
            ctl.state.read(cx).content_bounds.map(|b| b.size),
            window.viewport_size(),
        );
        let (position, corner) = placement.anchor(anchor);
        let opened_at = ctl.state.read(cx).opened_at;
        let (opacity, offset) = enter_animation(opened_at, 150.0, placement.side, window);
        let bg = t.foreground;
        let side = placement.side;
        let size_state = ctl.state.clone();
        let mut surface: Div = div()
            .relative()
            .top(offset.y)
            .left(offset.x)
            .opacity(opacity)
            .flex()
            .items_center()
            .gap(px(6.0))
            .max_w(px(320.0))
            .rounded(radius_xl(t))
            .px(px(12.0))
            .py(px(6.0))
            .text_size(px(12.0))
            .line_height(px(16.0))
            .bg(bg)
            .text_color(t.background)
            .font_family(t.font_sans.clone())
            .children(content.children)
            .child(measure_content(size_state))
            .child(arrow(side, bg));
        surface.style().refine(&content.style);
        root.child(
            deferred(
                anchored()
                    .position(position)
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.0))
                    .child(provide(InTooltip, surface)),
            )
            .with_priority(2),
        )
    }
}

/// `TooltipPrimitive.Arrow`: a 10px square rotated 45°, half tucked under the
/// popup, i.e. a ~5px triangle pointing at the trigger.
fn arrow(side: Side, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let h = 5.07;
            let c = bounds.center();
            let mut b = PathBuilder::fill();
            let (p0, p1, tip) = match side {
                Side::Top => (
                    point(c.x - px(h), bounds.bottom()),
                    point(c.x + px(h), bounds.bottom()),
                    point(c.x, bounds.bottom() + px(h)),
                ),
                Side::Bottom => (
                    point(c.x - px(h), bounds.top()),
                    point(c.x + px(h), bounds.top()),
                    point(c.x, bounds.top() - px(h)),
                ),
                Side::Left => (
                    point(bounds.right(), c.y - px(h)),
                    point(bounds.right(), c.y + px(h)),
                    point(bounds.right() + px(h), c.y),
                ),
                Side::Right => (
                    point(bounds.left(), c.y - px(h)),
                    point(bounds.left(), c.y + px(h)),
                    point(bounds.left() - px(h), c.y),
                ),
            };
            b.move_to(p0);
            b.line_to(tip);
            b.line_to(p1);
            b.close();
            if let Ok(path) = b.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The tooltip bubble: `bg-foreground text-background rounded-xl px-3 py-1.5
/// text-xs`, with an arrow.
pub struct TooltipContent {
    side: Side,
    align: Align,
    side_offset: f32,
    align_offset: f32,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Default for TooltipContent {
    fn default() -> Self {
        Self {
            side: Side::Top,
            align: Align::Center,
            side_offset: 4.0,
            align_offset: 0.0,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl TooltipContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default [`Side::Top`].
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
}
parent_styled!(TooltipContent);
