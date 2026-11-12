//! Port of shadcn `components/ui/switch.tsx` (Base UI `Switch`).

use std::rc::Rc;

use gpui::{
    App, Context, ElementId, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Refineable, RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use super::styles::{by_mode, focus_ring_color, ring_overlay_bordered, shadow_sm};
use crate::motion::{Duration, Easing, Instant, Transition};
use crate::theme::ActiveTheme;

/// `size` of [`Switch`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwitchSize {
    #[default]
    Default,
    Sm,
}

type CheckedHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

struct SwitchState {
    checked: bool,
    thumb: Transition,
    focus: FocusHandle,
}

/// An on/off switch. Controlled with [`Switch::checked`], uncontrolled with
/// [`Switch::default_checked`]; changes are reported by
/// [`Switch::on_checked_change`]. Space / Enter toggle it when focused.
///
/// ```ignore
/// Switch::new("monitor").checked(on).on_checked_change(|on, _, cx| ...)
/// ```
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: Option<bool>,
    default_checked: bool,
    size: SwitchSize,
    disabled: bool,
    invalid: bool,
    on_checked_change: Option<CheckedHandler>,
    style: StyleRefinement,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: None,
            default_checked: false,
            size: SwitchSize::Default,
            disabled: false,
            invalid: false,
            on_checked_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled checked state.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Initial state when uncontrolled.
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.default_checked = checked;
        self
    }

    pub fn size(mut self, size: SwitchSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `aria-invalid`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn on_checked_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_checked_change = Some(Rc::new(f));
        self
    }
}

impl Styled for Switch {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self.checked.unwrap_or(self.default_checked);
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<SwitchState>| {
                SwitchState {
                    checked: initial,
                    thumb: Transition::new(if initial { 1.0 } else { 0.0 }),
                    focus: cx.focus_handle().tab_stop(true),
                }
            });
        let now = Instant::now();
        // Controlled: follow the prop.
        if let Some(checked) = self.checked {
            state.update(cx, |s, _| {
                if s.checked != checked {
                    s.checked = checked;
                    s.thumb.animate_to(
                        if checked { 1.0 } else { 0.0 },
                        Duration::from_millis(150),
                        Easing::STANDARD,
                    );
                }
            });
        }
        let (checked, thumb_t, animating, focus) = {
            let s = state.read(cx);
            (
                s.checked,
                s.thumb.value(now),
                s.thumb.is_animating(now),
                s.focus.clone(),
            )
        };
        if animating {
            window.request_animation_frame();
        }
        let focus_visible =
            !self.disabled && focus.is_focused(window) && window.last_input_was_keyboard();

        let t = cx.theme();
        let (w, h, thumb) = match self.size {
            SwitchSize::Default => (32.0, 20.0, 16.0),
            SwitchSize::Sm => (24.0, 16.0, 12.0),
        };
        let travel = w - 4.0 - thumb;
        let radius = px(h / 2.0);
        let mut border = if checked {
            t.primary
        } else {
            gpui::transparent_black()
        };
        if focus_visible {
            border = t.ring;
        }
        if self.invalid {
            border = by_mode(t, t.destructive, t.destructive.opacity(0.5));
        }
        let bg = if checked {
            t.primary
        } else {
            t.input.opacity(0.9)
        };
        let thumb_bg = if t.is_dark {
            if checked {
                t.primary_foreground
            } else {
                t.foreground
            }
        } else {
            t.background
        };
        let ring = focus_ring_color(t, focus_visible, self.invalid, 0.3);

        let controlled = self.checked.is_some();
        let handler = self.on_checked_change.clone();
        let toggle = {
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let next = !state.read(cx).checked;
                if !controlled {
                    state.update(cx, |s, cx| {
                        s.checked = next;
                        s.thumb.animate_to(
                            if next { 1.0 } else { 0.0 },
                            Duration::from_millis(150),
                            Easing::STANDARD,
                        );
                        cx.notify();
                    });
                }
                if let Some(h) = handler.as_ref() {
                    h(next, window, cx);
                }
            }
        };
        let toggle = Rc::new(toggle);
        let disabled = self.disabled;

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .w(px(w))
            .h(px(h))
            .rounded(radius)
            .border_2()
            .border_color(border)
            .bg(bg)
            .role(gpui::Role::Switch)
            .aria_toggled(if checked {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .when(disabled, |el| el.opacity(0.5).cursor_not_allowed())
            .when_some(ring, |el, ring| {
                el.child(ring_overlay_bordered(radius, 3.0, 2.0, ring))
            })
            // `after:absolute after:-inset-x-3 after:-inset-y-2`: larger hit area.
            .when(!disabled, |el| {
                let toggle = toggle.clone();
                let focus = focus.clone();
                el.child(
                    div()
                        .id("hit")
                        .absolute()
                        .left(px(-12.0))
                        .right(px(-12.0))
                        .top(px(-8.0))
                        .bottom(px(-8.0))
                        .on_click(move |_, window, cx| {
                            window.focus(&focus, cx);
                            toggle(window, cx);
                        }),
                )
            })
            .child(
                div()
                    .size(px(thumb))
                    .rounded(px(thumb / 2.0))
                    .bg(thumb_bg)
                    .shadow(shadow_sm())
                    .ml(px(travel * thumb_t)),
            );
        el.style().refine(&self.style);
        if disabled {
            return el;
        }
        el.track_focus(&focus)
            // The larger hit-area child handles pointer clicks; this one only
            // sees keyboard activation (Space / Enter).
            .on_click(move |e, window, cx| {
                if e.is_keyboard() {
                    toggle(window, cx);
                }
            })
    }
}
