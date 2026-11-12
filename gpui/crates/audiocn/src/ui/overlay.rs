//! The floating-UI core shared by Popover, Select, DropdownMenu, ContextMenu
//! and Tooltip: open state (controlled / uncontrolled), trigger measurement,
//! Base UI's `side` / `align` / offsets positioning (with flipping), and
//! dismissal (click outside, Escape).

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, Bounds, Context, Div, ElementId, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point, ScrollHandle,
    Size, Stateful, StatefulInteractiveElement, Styled, Window, anchored, canvas, deferred, div,
    point, px,
};

use super::styles::provide;
use crate::motion::Instant;

/// Which side of the trigger a popup sits on (`side`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

/// Alignment along the trigger edge (`align`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

/// `side`, `align`, `sideOffset`, `alignOffset` of a Base UI positioner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub side: Side,
    pub align: Align,
    pub side_offset: f32,
    pub align_offset: f32,
}

impl Placement {
    pub fn new(side: Side, align: Align, side_offset: f32, align_offset: f32) -> Self {
        Self {
            side,
            align,
            side_offset,
            align_offset,
        }
    }

    /// Flips to the opposite side when the popup (of `size`, from the last
    /// frame) would overflow the viewport and fits better on the other side.
    pub fn flipped(
        mut self,
        trigger: Bounds<Pixels>,
        size: Option<Size<Pixels>>,
        viewport: Size<Pixels>,
    ) -> Self {
        let Some(size) = size else { return self };
        let margin = px(8.0);
        let off = px(self.side_offset);
        match self.side {
            Side::Bottom => {
                let below = viewport.height - trigger.bottom() - off - margin;
                let above = trigger.top() - off - margin;
                if size.height > below && above > below {
                    self.side = Side::Top;
                }
            }
            Side::Top => {
                let above = trigger.top() - off - margin;
                let below = viewport.height - trigger.bottom() - off - margin;
                if size.height > above && below > above {
                    self.side = Side::Bottom;
                }
            }
            Side::Right => {
                let right = viewport.width - trigger.right() - off - margin;
                let left = trigger.left() - off - margin;
                if size.width > right && left > right {
                    self.side = Side::Left;
                }
            }
            Side::Left => {
                let left = trigger.left() - off - margin;
                let right = viewport.width - trigger.right() - off - margin;
                if size.width > left && right > left {
                    self.side = Side::Right;
                }
            }
        }
        self
    }

    /// Window position + which corner of the popup sits there.
    pub fn anchor(&self, trigger: Bounds<Pixels>) -> (Point<Pixels>, Anchor) {
        let so = px(self.side_offset);
        let ao = px(self.align_offset);
        match self.side {
            Side::Bottom | Side::Top => {
                let y = if self.side == Side::Bottom {
                    trigger.bottom() + so
                } else {
                    trigger.top() - so
                };
                let (x, a_top, a_bottom) = match self.align {
                    Align::Start => (trigger.left() + ao, Anchor::TopLeft, Anchor::BottomLeft),
                    Align::Center => (
                        trigger.center().x + ao,
                        Anchor::TopCenter,
                        Anchor::BottomCenter,
                    ),
                    Align::End => (trigger.right() - ao, Anchor::TopRight, Anchor::BottomRight),
                };
                (
                    point(x, y),
                    if self.side == Side::Bottom {
                        a_top
                    } else {
                        a_bottom
                    },
                )
            }
            Side::Right | Side::Left => {
                let x = if self.side == Side::Right {
                    trigger.right() + so
                } else {
                    trigger.left() - so
                };
                let right = self.side == Side::Right;
                let (y, anchor) = match self.align {
                    Align::Start => (
                        trigger.top() + ao,
                        if right {
                            Anchor::TopLeft
                        } else {
                            Anchor::TopRight
                        },
                    ),
                    Align::Center => (
                        trigger.center().y + ao,
                        if right {
                            Anchor::LeftCenter
                        } else {
                            Anchor::RightCenter
                        },
                    ),
                    Align::End => (
                        trigger.bottom() - ao,
                        if right {
                            Anchor::BottomLeft
                        } else {
                            Anchor::BottomRight
                        },
                    ),
                };
                (point(x, y), anchor)
            }
        }
    }
}

pub(crate) type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

/// Per-instance overlay state (keyed element state).
pub(crate) struct OverlayState {
    pub open: bool,
    /// Open state seen by the last render (to detect controlled changes).
    pub last_open: bool,
    pub trigger_bounds: Option<Bounds<Pixels>>,
    pub content_bounds: Option<Bounds<Pixels>>,
    pub scroll: ScrollHandle,
    pub content_focus: FocusHandle,
    pub trigger_focus: FocusHandle,
    pub opened_at: Instant,
    /// Keyboard / pointer highlighted item (menus, select).
    pub highlighted: Option<usize>,
    /// Context menus: where the pointer opened it.
    pub anchor_point: Option<Point<Pixels>>,
    /// Select `alignItemWithTrigger`: y of the selected item inside the popup.
    pub selected_item_offset: Option<Pixels>,
    /// Hover-intent timer generation (tooltips).
    pub hover_generation: usize,
}

/// Handle to an overlay's state plus its props.
#[derive(Clone)]
pub(crate) struct OverlayCtl {
    pub state: Entity<OverlayState>,
    pub controlled: Option<bool>,
    pub on_open_change: Option<OpenHandler>,
}

/// Context provided to the trigger subtree (read by [`super::Button`]).
#[derive(Clone)]
pub(crate) struct OverlayTrigger {
    pub expanded: bool,
    pub focus: FocusHandle,
}

impl OverlayCtl {
    pub fn is_open(&self, cx: &App) -> bool {
        self.controlled.unwrap_or(self.state.read(cx).open)
    }

    pub fn set_open(&self, open: bool, window: &mut Window, cx: &mut App) {
        if self.is_open(cx) == open {
            return;
        }
        if self.controlled.is_none() {
            self.state.update(cx, |s, cx| {
                s.open = open;
                cx.notify();
            });
        }
        if !open {
            let (content, trigger) = {
                let s = self.state.read(cx);
                (s.content_focus.clone(), s.trigger_focus.clone())
            };
            if content.contains_focused(window, cx) || content.is_focused(window) {
                window.focus(&trigger, cx);
            }
        }
        if let Some(cb) = self.on_open_change.clone() {
            cb(open, window, cx);
        }
    }

    pub fn toggle(&self, window: &mut Window, cx: &mut App) {
        let open = self.is_open(cx);
        self.set_open(!open, window, cx);
    }
}

/// Reads (or creates) the overlay state for `id` and syncs it with props.
/// On every closed → open transition the popup's focus handle is focused.
pub(crate) fn use_overlay(
    id: impl Into<ElementId>,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenHandler>,
    focus_content: bool,
    window: &mut Window,
    cx: &mut App,
) -> OverlayCtl {
    let state = window.use_keyed_state(id, cx, |_, cx: &mut Context<OverlayState>| OverlayState {
        open: default_open,
        last_open: false,
        trigger_bounds: None,
        content_bounds: None,
        scroll: ScrollHandle::new(),
        content_focus: cx.focus_handle(),
        trigger_focus: cx.focus_handle().tab_stop(true),
        opened_at: Instant::now(),
        highlighted: None,
        anchor_point: None,
        selected_item_offset: None,
        hover_generation: 0,
    });
    let ctl = OverlayCtl {
        state,
        controlled: open,
        on_open_change,
    };
    let is_open = ctl.is_open(cx);
    let was_open = ctl.state.read(cx).last_open;
    if is_open != was_open {
        ctl.state.update(cx, |s, _| {
            s.last_open = is_open;
            if is_open {
                s.opened_at = Instant::now();
                s.content_bounds = None;
                s.selected_item_offset = None;
            } else {
                s.highlighted = None;
            }
        });
        if is_open && focus_content {
            let focus = ctl.state.read(cx).content_focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
    }
    ctl
}

/// Wraps `child` as the overlay trigger: measures it, toggles on click and
/// Enter / Space, and tells a [`super::Button`] child it controls a popup.
pub(crate) fn trigger(
    id: impl Into<ElementId>,
    ctl: &OverlayCtl,
    child: AnyElement,
    cx: &App,
) -> Stateful<Div> {
    let (expanded, focus) = (ctl.is_open(cx), ctl.state.read(cx).trigger_focus.clone());
    let measure = ctl.state.clone();
    let click = ctl.clone();
    let key = ctl.clone();
    div()
        .id(id)
        .relative()
        .flex()
        .child(provide(OverlayTrigger { expanded, focus }, child))
        .child(measure_trigger(measure))
        .on_click(move |e, window, cx| {
            if e.is_keyboard() {
                return;
            }
            click.toggle(window, cx);
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            if e.is_held {
                return;
            }
            let k = e.keystroke.key.as_str();
            let open = key.is_open(cx);
            if matches!(k, "enter" | "space") || (!open && matches!(k, "down" | "up")) {
                cx.stop_propagation();
                key.toggle(window, cx);
            }
        })
}

/// An absolutely positioned, full-size, non-interactive child that reports
/// its parent's window bounds during prepaint.
pub(crate) fn measure_bounds(
    f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| f(bounds, window, cx),
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Redraws the window on the next frame. The popup is positioned from bounds
/// measured during prepaint, so when they change (the first frame of a
/// `default_open` popup, a trigger that moved, a popup that needs flipping)
/// nothing else would draw the popup in its place until some unrelated
/// change redraws the window.
pub(crate) fn redraw_next_frame(window: &mut Window) {
    window.on_next_frame(|window, _| window.refresh());
}

/// [`measure_bounds`] for the trigger: stores its bounds in `state` and, while
/// the popup is open, redraws when they changed.
pub(crate) fn measure_trigger(state: Entity<OverlayState>) -> impl IntoElement {
    measure_bounds(move |bounds, window, cx| {
        let redraw = state.update(cx, |s, _| {
            let changed = s.trigger_bounds != Some(bounds);
            s.trigger_bounds = Some(bounds);
            changed && s.last_open
        });
        if redraw {
            redraw_next_frame(window);
        }
    })
}

/// [`measure_bounds`] for the popup surface: stores its bounds in `state` and
/// redraws when its size changed (placement flips on the size).
pub(crate) fn measure_content(state: Entity<OverlayState>) -> impl IntoElement {
    measure_bounds(move |bounds, window, cx| {
        let redraw = state.update(cx, |s, _| {
            let changed = s.content_bounds.map(|b| b.size) != Some(bounds.size);
            s.content_bounds = Some(bounds);
            changed
        });
        if redraw {
            redraw_next_frame(window);
        }
    })
}

/// Renders `content` in a deferred, anchored layer next to `anchor`, with
/// click-outside (ignoring `ignore` bounds, e.g. the trigger) and Escape
/// dismissal. `content` should be the styled popup surface.
pub(crate) fn popup(
    ctl: &OverlayCtl,
    placement: Placement,
    anchor: Bounds<Pixels>,
    content: Div,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let s = ctl.state.read(cx);
    let placement = placement.flipped(
        anchor,
        s.content_bounds.map(|b| b.size),
        window.viewport_size(),
    );
    let (position, corner) = placement.anchor(anchor);
    let focus = s.content_focus.clone();
    let outside = ctl.clone();
    let escape = ctl.clone();
    let size_state = ctl.state.clone();
    let ignore = s.trigger_bounds;
    deferred(
        anchored()
            .position(position)
            .anchor(corner)
            .snap_to_window_with_margin(px(8.0))
            .child(
                content
                    .id("popup")
                    .occlude()
                    .track_focus(&focus)
                    .child(measure_content(size_state))
                    .on_mouse_down_out(move |e, window, cx| {
                        if ignore.is_some_and(|b| b.contains(&e.position)) {
                            return;
                        }
                        outside.set_open(false, window, cx);
                    })
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key == "escape" {
                            cx.stop_propagation();
                            escape.set_open(false, window, cx);
                        }
                    }),
            ),
    )
    .with_priority(1)
}

/// Opacity and slide offset of Tailwind's `animate-in fade-in-0
/// slide-in-from-*-2` at `duration` ms (`zoom-in-95` is not reproduced).
pub(crate) fn enter_animation(
    opened_at: Instant,
    duration_ms: f32,
    side: Side,
    window: &mut Window,
) -> (f32, Point<Pixels>) {
    let t = (Instant::now().duration_since(opened_at).as_secs_f32() * 1000.0 / duration_ms)
        .clamp(0.0, 1.0);
    if t < 1.0 {
        window.request_animation_frame();
    }
    // tw-animate-css uses the default `ease` timing function.
    let e = crate::motion::Easing::EASE.apply(t);
    let d = px(8.0 * (1.0 - e));
    let offset = match side {
        Side::Bottom => point(px(0.0), -d),
        Side::Top => point(px(0.0), d),
        Side::Right => point(-d, px(0.0)),
        Side::Left => point(d, px(0.0)),
    };
    (e, offset)
}
