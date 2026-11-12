//! Port of shadcn `components/ui/toggle.tsx` (Base UI `Toggle`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, Refineable, RenderOnce, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, div, prelude::FluentBuilder, px,
};

use super::icons::Icon;
use super::styles::{TextSize, fit_radius, focus_ring_color, radius_2xl, ring_overlay_bordered};
use crate::theme::ActiveTheme;

/// `variant` of [`Toggle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToggleVariant {
    #[default]
    Default,
    Outline,
}

/// `size` of [`Toggle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToggleSize {
    #[default]
    Default,
    Sm,
    Lg,
}

impl ToggleSize {
    pub fn height(self) -> Pixels {
        px(match self {
            Self::Sm => 28.0,
            Self::Default => 32.0,
            Self::Lg => 36.0,
        })
    }
}

type PressedHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

struct ToggleState {
    pressed: bool,
    focus: FocusHandle,
}

/// A two-state button. Controlled with [`Toggle::pressed`], uncontrolled with
/// [`Toggle::default_pressed`]; changes go to [`Toggle::on_pressed_change`].
///
/// ```ignore
/// Toggle::new("bold").variant(ToggleVariant::Outline).icon(IconName::TextT)
/// ```
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    pressed: Option<bool>,
    default_pressed: bool,
    variant: ToggleVariant,
    size: ToggleSize,
    disabled: bool,
    invalid: bool,
    icon_start: Option<Icon>,
    icon_end: Option<Icon>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_pressed_change: Option<PressedHandler>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            pressed: None,
            default_pressed: false,
            variant: ToggleVariant::Default,
            size: ToggleSize::Default,
            disabled: false,
            invalid: false,
            icon_start: None,
            icon_end: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
            on_pressed_change: None,
        }
    }

    /// Controlled pressed state.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    pub fn default_pressed(mut self, pressed: bool) -> Self {
        self.default_pressed = pressed;
        self
    }

    pub fn variant(mut self, variant: ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ToggleSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Leading icon (`data-icon="inline-start"`, `size-4`).
    pub fn icon_start(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_start = Some(icon.into());
        self
    }

    /// Trailing icon (`data-icon="inline-end"`).
    pub fn icon_end(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_end = Some(icon.into());
        self
    }

    /// Icon-only toggle (same as [`Toggle::icon_start`]).
    pub fn icon(self, icon: impl Into<Icon>) -> Self {
        self.icon_start(icon)
    }

    pub fn on_pressed_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_pressed_change = Some(Rc::new(f));
        self
    }
}

impl ParentElement for Toggle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Toggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self.pressed.unwrap_or(self.default_pressed);
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<ToggleState>| {
                ToggleState {
                    pressed: initial,
                    focus: cx.focus_handle().tab_stop(true),
                }
            });
        if let Some(p) = self.pressed {
            state.update(cx, |s, _| s.pressed = p);
        }
        let (pressed, focus) = {
            let s = state.read(cx);
            (s.pressed, s.focus.clone())
        };
        let focus_visible =
            !self.disabled && focus.is_focused(window) && window.last_input_was_keyboard();

        let t = cx.theme();
        let height = self.size.height();
        let radius = fit_radius(radius_2xl(t), height);
        let (pad_icon, pad) = match self.size {
            ToggleSize::Sm => (6.0, 10.0),
            _ => (8.0, 10.0),
        };
        let outline = self.variant == ToggleVariant::Outline;
        let mut border = if outline {
            t.input
        } else {
            gpui::transparent_black()
        };
        if focus_visible {
            border = t.ring;
        }
        if self.invalid {
            border = t.destructive;
        }
        let ring = focus_ring_color(t, focus_visible, self.invalid, 0.3);
        let muted = t.muted;
        let fg = t.foreground;
        let disabled = self.disabled;

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(height)
            .min_w(height)
            .pl(px(if self.icon_start.is_some() {
                pad_icon
            } else {
                pad
            }))
            .pr(px(if self.icon_end.is_some() {
                pad_icon
            } else {
                pad
            }))
            .rounded(radius)
            .when(outline || focus_visible || self.invalid, |el| {
                el.border_1().border_color(border)
            })
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .when(pressed, |el| el.bg(muted))
            .when(!disabled, |el| {
                el.hover(move |s| s.bg(muted).text_color(fg))
            })
            .when(disabled, |el| el.opacity(0.5))
            .role(gpui::Role::Button)
            .aria_toggled(if pressed {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .when_some(ring, |el, ring| {
                el.child(ring_overlay_bordered(radius, 3.0, 1.0, ring))
            })
            .when_some(self.icon_start, |el, i| el.child(i.size(px(16.0))))
            .children(self.children)
            .when_some(self.icon_end, |el, i| el.child(i.size(px(16.0))));
        el.style().refine(&self.style);
        if disabled {
            return el;
        }
        let controlled = self.pressed.is_some();
        let handler = self.on_pressed_change;
        el.track_focus(&focus).on_click(move |_, window, cx| {
            let next = !state.read(cx).pressed;
            if !controlled {
                state.update(cx, |s, cx| {
                    s.pressed = next;
                    cx.notify();
                });
            }
            if let Some(h) = handler.as_ref() {
                h(next, window, cx);
            }
        })
    }
}
