//! Port of shadcn `components/ui/tabs.tsx` (Base UI `Tabs`).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::styles::{
    TextSize, by_mode, parent_styled, provide, radius_2xl, ring_overlay, use_context,
};
use crate::Orientation;
use crate::theme::ActiveTheme;

type ValueHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;

struct TabsState {
    value: Option<SharedString>,
}

/// Shared with the parts through context.
#[derive(Clone)]
struct TabsCx {
    state: Entity<TabsState>,
    value: Option<SharedString>,
    controlled: bool,
    orientation: Orientation,
    on_value_change: Option<ValueHandler>,
    /// Trigger values + focus handles in render order (for arrow keys).
    triggers: Rc<RefCell<Vec<(SharedString, FocusHandle)>>>,
}

impl TabsCx {
    fn select(&self, value: SharedString, window: &mut Window, cx: &mut App) {
        if self.value.as_ref() == Some(&value) {
            return;
        }
        if !self.controlled {
            self.state.update(cx, |s, cx| {
                s.value = Some(value.clone());
                cx.notify();
            });
        }
        if let Some(h) = self.on_value_change.as_ref() {
            h(value, window, cx);
        }
    }
}

/// Tabs root: `flex gap-2` (`flex-col` when horizontal). Controlled with
/// [`Tabs::value`], uncontrolled with [`Tabs::default_value`].
///
/// ```ignore
/// Tabs::new("view").default_value("preview")
///     .child(TabsList::new().variant(TabsListVariant::Line)
///         .child(TabsTrigger::new("preview").child("Preview"))
///         .child(TabsTrigger::new("code").child("Code")))
///     .child(TabsContent::new("preview").child(...))
/// ```
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    orientation: Orientation,
    on_value_change: Option<ValueHandler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: None,
            orientation: Orientation::Horizontal,
            on_value_change: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Controlled active tab.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn on_value_change(
        mut self,
        f: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }
}
parent_styled!(Tabs);

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default = self.default_value.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _: &mut Context<TabsState>| {
            TabsState { value: default }
        });
        let controlled = self.value.is_some();
        let value = if controlled {
            self.value.clone()
        } else {
            state.read(cx).value.clone()
        };
        let tcx = TabsCx {
            state,
            value,
            controlled,
            orientation: self.orientation,
            on_value_change: self.on_value_change,
            triggers: Rc::default(),
        };
        let mut el: Div = div()
            .flex()
            .gap(px(8.0))
            .when(self.orientation == Orientation::Horizontal, |el| {
                el.flex_col()
            })
            .children(self.children);
        el.style().refine(&self.style);
        provide(tcx, el)
    }
}

/// `variant` of [`TabsList`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabsListVariant {
    /// `bg-muted` pill track.
    #[default]
    Default,
    /// Transparent, with an underline under the active tab.
    Line,
}

/// The row (or column) of [`TabsTrigger`]s.
#[derive(IntoElement, Default)]
pub struct TabsList {
    variant: TabsListVariant,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TabsList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn variant(mut self, variant: TabsListVariant) -> Self {
        self.variant = variant;
        self
    }
}
parent_styled!(TabsList);

impl RenderOnce for TabsList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let vertical =
            use_context::<TabsCx>().is_some_and(|c| c.orientation == Orientation::Vertical);
        let line = self.variant == TabsListVariant::Line;
        let mut el: Div = div()
            .flex()
            .items_center()
            .justify_center()
            .text_color(t.muted_foreground)
            .when(!line, |el| el.bg(t.muted).rounded(radius_2xl(t)))
            .when(line, |el| el.gap(px(4.0)))
            .map(|el| {
                if vertical {
                    el.flex_col().p(px(4.0))
                } else {
                    el.h(px(32.0)).p(px(3.0))
                }
            })
            .children(self.children);
        el.style().refine(&self.style);
        provide(self.variant, el)
    }
}

struct TriggerState {
    focus: FocusHandle,
}

/// A tab button; `value` matches a [`TabsContent`].
#[derive(IntoElement)]
pub struct TabsTrigger {
    value: SharedString,
    disabled: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TabsTrigger {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            disabled: false,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
parent_styled!(TabsTrigger);

impl RenderOnce for TabsTrigger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tcx = use_context::<TabsCx>();
        let variant = use_context::<TabsListVariant>().unwrap_or_default();
        let id = ElementId::Name(format!("tabs-trigger-{}", self.value).into());
        let focus = window
            .use_keyed_state(id.clone(), cx, |_, cx: &mut Context<TriggerState>| {
                TriggerState {
                    focus: cx.focus_handle().tab_stop(true),
                }
            })
            .read(cx)
            .focus
            .clone();
        if let Some(tcx) = tcx.as_ref() {
            tcx.triggers
                .borrow_mut()
                .push((self.value.clone(), focus.clone()));
        }
        let active = tcx
            .as_ref()
            .is_some_and(|c| c.value.as_ref() == Some(&self.value));
        let vertical = tcx
            .as_ref()
            .is_some_and(|c| c.orientation == Orientation::Vertical);
        let focus_visible =
            !self.disabled && focus.is_focused(window) && window.last_input_was_keyboard();

        let t = cx.theme();
        let radius = radius_2xl(t).min(px(12.5));
        let line = variant == TabsListVariant::Line;
        let base_fg = by_mode(t, t.foreground.opacity(0.6), t.muted_foreground);
        let active_bg = (!line).then(|| by_mode(t, t.background, t.input.opacity(0.3)));
        let fg = t.foreground;
        let ring = t.ring;
        let disabled = self.disabled;

        let mut el = div()
            .id(id)
            .relative()
            .flex()
            .flex_1()
            .items_center()
            .gap(px(6.0))
            .rounded(radius)
            .border_1()
            .border_color(if focus_visible {
                ring
            } else {
                gpui::transparent_black()
            })
            .py(px(2.0))
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .text_color(if active { fg } else { base_fg })
            .map(|el| {
                if vertical {
                    el.w_full().justify_start().px(px(12.0))
                } else {
                    el.h(px(25.0)).justify_center().px(px(6.0))
                }
            })
            .when(!disabled, |el| el.hover(move |s| s.text_color(fg)))
            .when(disabled, |el| el.opacity(0.5))
            .when(active, |el| el.when_some(active_bg, |el, bg| el.bg(bg)))
            .role(gpui::Role::Tab)
            .aria_selected(active)
            .when(focus_visible, |el| {
                el.child(ring_overlay(radius, 3.0, ring.opacity(0.5)))
            })
            .when(line && active, |el| {
                let fg = t.foreground;
                el.child(if vertical {
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(-4.0))
                        .w(px(2.0))
                        .bg(fg)
                } else {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(-5.0))
                        .h(px(2.0))
                        .bg(fg)
                })
            })
            .children(self.children);
        el.style().refine(&self.style);
        if disabled {
            return el;
        }
        let Some(tcx) = tcx else {
            return el.track_focus(&focus);
        };
        let value = self.value.clone();
        let click_cx = tcx.clone();
        el.track_focus(&focus)
            .on_click(move |_, window, cx| click_cx.select(value.clone(), window, cx))
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let key = e.keystroke.key.as_str();
                let (prev, next) = if tcx.orientation == Orientation::Vertical {
                    ("up", "down")
                } else {
                    ("left", "right")
                };
                let triggers = tcx.triggers.borrow().clone();
                if triggers.is_empty() {
                    return;
                }
                let Some(i) = triggers.iter().position(|(_, f)| f.is_focused(window)) else {
                    return;
                };
                let n = triggers.len();
                let target = match key {
                    k if k == prev => (i + n - 1) % n,
                    k if k == next => (i + 1) % n,
                    "home" => 0,
                    "end" => n - 1,
                    _ => return,
                };
                cx.stop_propagation();
                window.focus(&triggers[target].1, cx);
            })
    }
}

/// The panel shown while its `value` is active (`flex-1 text-sm`).
#[derive(IntoElement)]
pub struct TabsContent {
    value: SharedString,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TabsContent {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}
parent_styled!(TabsContent);

impl RenderOnce for TabsContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let active = use_context::<TabsCx>().is_some_and(|c| c.value.as_ref() == Some(&self.value));
        let mut el: Div = div()
            .flex_1()
            .text_sm_()
            .when(!active, |el| el.hidden())
            .when(active, |el| el.children(self.children));
        el.style().refine(&self.style);
        el
    }
}
