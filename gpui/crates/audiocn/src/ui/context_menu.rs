//! Port of shadcn `components/ui/context-menu.tsx` (Base UI `ContextMenu`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Refineable, RenderOnce, StyleRefinement, Styled, Window, div, px, size,
};

pub use super::menu::{
    MenuCheckboxItem as ContextMenuCheckboxItem, MenuContent as ContextMenuContent,
    MenuGroup as ContextMenuGroup, MenuItem as ContextMenuItem,
    MenuItemVariant as ContextMenuItemVariant, MenuLabel as ContextMenuLabel,
    MenuRadioGroup as ContextMenuRadioGroup, MenuRadioItem as ContextMenuRadioItem,
    MenuSeparator as ContextMenuSeparator, MenuShortcut as ContextMenuShortcut,
};
use super::menu::{MenuKind, render_menu};
use super::overlay::{Align, OpenHandler, Placement, Side, use_overlay};

/// A menu opened by right-clicking its trigger area, at the pointer.
///
/// ```ignore
/// ContextMenu::new("pad")
///     .trigger(div().size_full().child("Right click here"))
///     .content(ContextMenuContent::new().child(ContextMenuItem::new("Clear pad")))
/// ```
#[derive(IntoElement)]
pub struct ContextMenu {
    id: ElementId,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
    disabled: bool,
    trigger: Option<AnyElement>,
    content: Option<ContextMenuContent>,
    style: StyleRefinement,
}

impl ContextMenu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: None,
            on_open_change: None,
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `ContextMenuTrigger`: the area that opens the menu on right click.
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    /// The menu. Its `side` / `align` are ignored (it opens at the pointer,
    /// `alignOffset` 4 like shadcn's defaults).
    pub fn content(mut self, content: ContextMenuContent) -> Self {
        self.content = Some(content);
        self
    }
}

/// Styles the trigger area.
impl Styled for ContextMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctl = use_overlay(
            self.id.clone(),
            self.open,
            false,
            self.on_open_change,
            true,
            window,
            cx,
        );
        let open_ctl = ctl.clone();
        let mut area = div()
            .id(self.id)
            .child(self.trigger.unwrap_or_else(|| div().into_any_element()));
        if !self.disabled {
            area = area.on_mouse_down(MouseButton::Right, move |e, window, cx| {
                cx.stop_propagation();
                let position = e.position;
                open_ctl.state.update(cx, |s, cx| {
                    s.anchor_point = Some(position);
                    s.highlighted = None;
                    cx.notify();
                });
                if !open_ctl.is_open(cx) {
                    open_ctl.set_open(true, window, cx);
                }
            });
        }
        area.style().refine(&self.style);
        let point = ctl.state.read(cx).anchor_point;
        match (ctl.is_open(cx), point, self.content) {
            (true, Some(p), Some(content)) => {
                let placement = Placement::new(Side::Right, Align::Start, 0.0, 4.0);
                let anchor = Bounds::new(p, size(px(0.0), px(0.0)));
                let menu = render_menu(
                    MenuKind::Context,
                    &ctl,
                    content,
                    placement,
                    anchor,
                    window,
                    cx,
                );
                area.child(menu)
            }
            _ => area,
        }
    }
}
