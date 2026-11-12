//! Port of shadcn `components/ui/dropdown-menu.tsx` (Base UI `Menu`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce,
    StyleRefinement, Styled, Window, div,
};

pub use super::menu::{
    MenuCheckboxItem as DropdownMenuCheckboxItem, MenuContent as DropdownMenuContent,
    MenuGroup as DropdownMenuGroup, MenuItem as DropdownMenuItem,
    MenuItemVariant as DropdownMenuItemVariant, MenuLabel as DropdownMenuLabel,
    MenuRadioGroup as DropdownMenuRadioGroup, MenuRadioItem as DropdownMenuRadioItem,
    MenuSeparator as DropdownMenuSeparator, MenuShortcut as DropdownMenuShortcut,
};
use super::menu::{MenuKind, render_menu};
use super::overlay::{OpenHandler, Placement, trigger, use_overlay};

/// A menu opened from a trigger (usually a [`super::Button`]).
///
/// ```ignore
/// DropdownMenu::new("more")
///     .trigger(Button::new("btn").variant(ButtonVariant::Ghost).size(ButtonSize::Icon)
///         .icon(IconName::DotsThree))
///     .content(DropdownMenuContent::new()
///         .child(DropdownMenuLabel::new("Track"))
///         .child(DropdownMenuItem::new("Rename").shortcut("⌘R").on_select(|_, _| {}))
///         .child(DropdownMenuSeparator::new())
///         .child(DropdownMenuItem::new("Delete").variant(DropdownMenuItemVariant::Destructive)))
/// ```
#[derive(IntoElement)]
pub struct DropdownMenu {
    id: ElementId,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenHandler>,
    trigger: Option<AnyElement>,
    content: Option<DropdownMenuContent>,
    style: StyleRefinement,
}

impl DropdownMenu {
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

    /// `DropdownMenuTrigger`.
    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn content(mut self, content: DropdownMenuContent) -> Self {
        self.content = Some(content);
        self
    }
}

/// Styles the trigger wrapper.
impl Styled for DropdownMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DropdownMenu {
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
        let mut t = trigger(
            self.id,
            &ctl,
            self.trigger.unwrap_or_else(|| div().into_any_element()),
            cx,
        );
        t.style().refine(&self.style);
        let root = t;
        let anchor = ctl.state.read(cx).trigger_bounds;
        match (ctl.is_open(cx), anchor, self.content) {
            (true, Some(anchor), Some(content)) => {
                let placement = Placement::new(
                    content.side,
                    content.align,
                    content.side_offset,
                    content.align_offset,
                );
                let menu = render_menu(
                    MenuKind::Dropdown,
                    &ctl,
                    content,
                    placement,
                    anchor,
                    window,
                    cx,
                );
                root.child(menu)
            }
            _ => root,
        }
    }
}
