//! The menu (Base UI `Menu`) shared by `dropdown_menu.rs` and
//! `context_menu.rs`: content, items, checkbox / radio items, labels,
//! separators, shortcuts, keyboard navigation.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Div, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Refineable, RenderOnce, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::icons::{Icon, IconName};
use super::overlay::{Align, OverlayCtl, Placement, Side, popup};
use super::styles::{
    TextSize, by_mode, parent_styled, radius_2xl, radius_xl, ring_shadow, shadow_lg,
};
use crate::theme::ActiveTheme;

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type BoolHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
type ValueHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;

/// `variant` of [`MenuItem`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuItemVariant {
    #[default]
    Default,
    Destructive,
}

/// Which menu flavour renders the content (sizing differs slightly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuKind {
    Dropdown,
    Context,
}

/// A menu entry.
#[allow(clippy::large_enum_variant)]
pub enum MenuNode {
    Item(MenuItem),
    Checkbox(MenuCheckboxItem),
    RadioGroup(MenuRadioGroup),
    Label(MenuLabel),
    Separator(MenuSeparator),
    Group(MenuGroup),
    Custom(AnyElement),
}

macro_rules! node_from {
    ($($ty:ident => $variant:ident),*) => {
        $(impl From<$ty> for MenuNode {
            fn from(v: $ty) -> Self {
                Self::$variant(v)
            }
        })*
    };
}
node_from!(
    MenuItem => Item,
    MenuCheckboxItem => Checkbox,
    MenuRadioGroup => RadioGroup,
    MenuLabel => Label,
    MenuSeparator => Separator,
    MenuGroup => Group
);

impl From<AnyElement> for MenuNode {
    fn from(v: AnyElement) -> Self {
        Self::Custom(v)
    }
}

/// The menu popup (`DropdownMenuContent` / `ContextMenuContent`).
pub struct MenuContent {
    pub(crate) side: Side,
    pub(crate) align: Align,
    pub(crate) side_offset: f32,
    pub(crate) align_offset: f32,
    pub(crate) children: Vec<MenuNode>,
    pub(crate) style: StyleRefinement,
}

impl Default for MenuContent {
    fn default() -> Self {
        Self {
            side: Side::Bottom,
            align: Align::Start,
            side_offset: 4.0,
            align_offset: 0.0,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl MenuContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item, checkbox / radio group, label, separator, group or element.
    pub fn child(mut self, node: impl Into<MenuNode>) -> Self {
        self.children.push(node.into());
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<MenuNode>>) -> Self {
        self.children.extend(nodes.into_iter().map(Into::into));
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn side_offset(mut self, offset: f32) -> Self {
        self.side_offset = offset;
        self
    }

    pub fn align_offset(mut self, offset: f32) -> Self {
        self.align_offset = offset;
        self
    }
}

impl Styled for MenuContent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A plain grouping of entries (`DropdownMenuGroup`).
#[derive(Default)]
pub struct MenuGroup {
    children: Vec<MenuNode>,
}

impl MenuGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, node: impl Into<MenuNode>) -> Self {
        self.children.push(node.into());
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<MenuNode>>) -> Self {
        self.children.extend(nodes.into_iter().map(Into::into));
        self
    }
}

/// An action item. Children are the label (icons via [`MenuItem::icon`],
/// shortcut via [`MenuItem::shortcut`]).
pub struct MenuItem {
    icon: Option<Icon>,
    shortcut: Option<SharedString>,
    inset: bool,
    variant: MenuItemVariant,
    disabled: bool,
    close_on_click: bool,
    on_select: Option<Action>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self::empty().child(label.into())
    }

    /// No label (add children).
    pub fn empty() -> Self {
        Self {
            icon: None,
            shortcut: None,
            inset: false,
            variant: MenuItemVariant::Default,
            disabled: false,
            close_on_click: true,
            on_select: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Trailing `DropdownMenuShortcut` text.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// `inset`: `pl-7`, aligning with checkbox items.
    pub fn inset(mut self, inset: bool) -> Self {
        self.inset = inset;
        self
    }

    pub fn variant(mut self, variant: MenuItemVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `closeOnClick` (default `true`).
    pub fn close_on_click(mut self, close: bool) -> Self {
        self.close_on_click = close;
        self
    }

    /// `onClick`: the item was activated (click, Enter or Space).
    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}
parent_styled!(MenuItem);

/// A checkable item (check mark on the right). Controlled: pass `checked`
/// and update it from `on_checked_change`. Doesn't close the menu by default.
pub struct MenuCheckboxItem {
    checked: bool,
    inset: bool,
    disabled: bool,
    close_on_click: bool,
    on_checked_change: Option<BoolHandler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MenuCheckboxItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            checked: false,
            inset: false,
            disabled: false,
            close_on_click: false,
            on_checked_change: None,
            children: vec![label.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn inset(mut self, inset: bool) -> Self {
        self.inset = inset;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn close_on_click(mut self, close: bool) -> Self {
        self.close_on_click = close;
        self
    }

    pub fn on_checked_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_checked_change = Some(Rc::new(f));
        self
    }
}
parent_styled!(MenuCheckboxItem);

/// A group of [`MenuRadioItem`]s sharing one `value`.
#[derive(Default)]
pub struct MenuRadioGroup {
    value: Option<SharedString>,
    on_value_change: Option<ValueHandler>,
    items: Vec<MenuRadioItem>,
}

impl MenuRadioGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn on_value_change(
        mut self,
        f: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    pub fn child(mut self, item: MenuRadioItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = MenuRadioItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// A radio item (check mark when its value is the group's value).
pub struct MenuRadioItem {
    value: SharedString,
    inset: bool,
    disabled: bool,
    close_on_click: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MenuRadioItem {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            inset: false,
            disabled: false,
            close_on_click: false,
            children: vec![label.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }

    pub fn inset(mut self, inset: bool) -> Self {
        self.inset = inset;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn close_on_click(mut self, close: bool) -> Self {
        self.close_on_click = close;
        self
    }
}
parent_styled!(MenuRadioItem);

/// `text-muted-foreground px-2 py-1 text-xs` heading.
pub struct MenuLabel {
    text: SharedString,
    inset: bool,
}

impl MenuLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            inset: false,
        }
    }

    pub fn inset(mut self, inset: bool) -> Self {
        self.inset = inset;
        self
    }
}

/// `-mx-1 my-1 h-px bg-foreground/5`.
#[derive(Default)]
pub struct MenuSeparator;

impl MenuSeparator {
    pub fn new() -> Self {
        Self
    }
}

/// `ml-auto text-xs tracking-widest text-muted-foreground`; use as an item child.
#[derive(IntoElement)]
pub struct MenuShortcut {
    text: SharedString,
}

impl MenuShortcut {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for MenuShortcut {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .ml_auto()
            .text_xs_()
            .text_color(cx.theme().muted_foreground)
            .child(spaced(&self.text))
    }
}

/// `tracking-widest` (0.1em) approximated with thin spaces.
fn spaced(text: &str) -> SharedString {
    let mut out = String::with_capacity(text.len() * 4);
    for (i, c) in text.chars().enumerate() {
        if i > 0 {
            out.push('\u{200A}');
        }
        out.push(c);
    }
    out.into()
}

// ---------------------------------------------------------------------------
// Rendering

#[allow(clippy::large_enum_variant)]
enum Row {
    Active {
        el: ItemParts,
        action: Action,
        close: bool,
        disabled: bool,
    },
    Label(MenuLabel),
    Separator,
    Custom(AnyElement),
}

struct ItemParts {
    icon: Option<Icon>,
    indicator: Option<bool>,
    shortcut: Option<SharedString>,
    inset: bool,
    destructive: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

fn flatten(nodes: Vec<MenuNode>, out: &mut Vec<Row>) {
    for node in nodes {
        match node {
            MenuNode::Item(i) => {
                let action = i.on_select.unwrap_or_else(|| Rc::new(|_, _| {}));
                out.push(Row::Active {
                    el: ItemParts {
                        icon: i.icon,
                        indicator: None,
                        shortcut: i.shortcut,
                        inset: i.inset,
                        destructive: i.variant == MenuItemVariant::Destructive,
                        children: i.children,
                        style: i.style,
                    },
                    action,
                    close: i.close_on_click,
                    disabled: i.disabled,
                });
            }
            MenuNode::Checkbox(c) => {
                let checked = c.checked;
                let handler = c.on_checked_change;
                out.push(Row::Active {
                    el: ItemParts {
                        icon: None,
                        indicator: Some(checked),
                        shortcut: None,
                        inset: c.inset,
                        destructive: false,
                        children: c.children,
                        style: c.style,
                    },
                    action: Rc::new(move |window, cx| {
                        if let Some(h) = handler.as_ref() {
                            h(!checked, window, cx);
                        }
                    }),
                    close: c.close_on_click,
                    disabled: c.disabled,
                });
            }
            MenuNode::RadioGroup(g) => {
                for item in g.items {
                    let handler = g.on_value_change.clone();
                    let value = item.value.clone();
                    let selected = g.value.as_ref() == Some(&item.value);
                    out.push(Row::Active {
                        el: ItemParts {
                            icon: None,
                            indicator: Some(selected),
                            shortcut: None,
                            inset: item.inset,
                            destructive: false,
                            children: item.children,
                            style: item.style,
                        },
                        action: Rc::new(move |window, cx| {
                            if !selected && let Some(h) = handler.as_ref() {
                                h(value.clone(), window, cx);
                            }
                        }),
                        close: item.close_on_click,
                        disabled: item.disabled,
                    });
                }
            }
            MenuNode::Label(l) => out.push(Row::Label(l)),
            MenuNode::Separator(_) => out.push(Row::Separator),
            MenuNode::Group(g) => flatten(g.children, out),
            MenuNode::Custom(e) => out.push(Row::Custom(e)),
        }
    }
}

/// Renders the open menu popup anchored to `anchor`.
pub(crate) fn render_menu(
    kind: MenuKind,
    ctl: &OverlayCtl,
    content: MenuContent,
    placement: Placement,
    anchor: Bounds<Pixels>,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let t = cx.theme();
    let mut rows = Vec::new();
    flatten(content.children, &mut rows);
    let highlighted = ctl.state.read(cx).highlighted;
    let highlight_bg = t.foreground.opacity(0.1);
    let accent_fg = t.accent_foreground;
    let item_radius = radius_xl(t);

    let mut actions: Vec<(Action, bool, bool)> = Vec::new();
    let mut list = div().flex().flex_col();
    for row in rows {
        match row {
            Row::Separator => {
                list = list.child(
                    div()
                        .mx(px(-4.0))
                        .my(px(4.0))
                        .h(px(1.0))
                        .flex_shrink_0()
                        .bg(t.foreground.opacity(0.05)),
                );
            }
            Row::Custom(e) => list = list.child(e),
            Row::Label(l) => {
                list = list.child(
                    div()
                        .px(px(8.0))
                        .when(l.inset, |el| el.pl(px(28.0)))
                        .py(px(4.0))
                        .text_xs_()
                        .text_color(t.muted_foreground)
                        .child(l.text),
                );
            }
            Row::Active {
                el: parts,
                action,
                close,
                disabled,
            } => {
                let ix = actions.len();
                actions.push((action.clone(), close, disabled));
                let is_hl = highlighted == Some(ix) && !disabled;
                let fg = if is_hl || parts.destructive {
                    accent_fg
                } else {
                    t.popover_foreground
                };
                let has_indicator = parts.indicator.is_some();
                let hover = ctl.state.clone();
                let click_ctl = ctl.clone();
                let mut el = div()
                    .id(ElementId::Integer(ix as u64))
                    .relative()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(8.0))
                    .min_h(px(28.0))
                    .rounded(item_radius)
                    .px(px(8.0))
                    .py(px(6.0))
                    .when(has_indicator, |el| el.pr(px(32.0)))
                    .when(parts.inset, |el| el.pl(px(28.0)))
                    .text_sm_()
                    .text_color(fg)
                    .when(is_hl, |el| el.bg(highlight_bg))
                    .when(disabled, |el| el.opacity(0.5))
                    .role(gpui::Role::MenuItem)
                    .when_some(parts.icon, |el, i| el.child(i.size(px(16.0))))
                    .children(parts.children)
                    .when_some(parts.shortcut, |el, s| {
                        el.child(
                            div()
                                .ml_auto()
                                .text_xs_()
                                .text_color(if is_hl { accent_fg } else { t.muted_foreground })
                                .child(spaced(&s)),
                        )
                    })
                    .when(parts.indicator == Some(true), |el| {
                        el.child(
                            div()
                                .absolute()
                                .right(px(8.0))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(Icon::new(IconName::Check).size(px(16.0))),
                        )
                    });
                el.style().refine(&parts.style);
                if !disabled {
                    el = el
                        .on_mouse_move(move |_, _, cx| {
                            hover.update(cx, |s, cx| {
                                if s.highlighted != Some(ix) {
                                    s.highlighted = Some(ix);
                                    cx.notify();
                                }
                            });
                        })
                        .on_click(move |_, window, cx| {
                            if close {
                                click_ctl.set_open(false, window, cx);
                            }
                            action(window, cx);
                        });
                }
                list = list.child(el);
            }
        }
    }

    let mut shadows = shadow_lg();
    shadows.push(ring_shadow(
        1.0,
        by_mode(t, t.foreground.opacity(0.05), t.foreground.opacity(0.1)),
    ));
    let max_h = window.viewport_size().height - px(16.0);
    let mut surface: Div = div()
        .relative()
        .rounded(radius_2xl(t))
        .p(px(4.0))
        .max_h(max_h)
        .overflow_hidden()
        .bg(t.popover)
        .text_color(t.popover_foreground)
        .font_family(t.font_sans.clone())
        .shadow(shadows)
        .map(|el| match kind {
            MenuKind::Dropdown => el.w(anchor.size.width).min_w(px(128.0)),
            MenuKind::Context => el.min_w(px(144.0)),
        })
        .child(list);
    surface.style().refine(&content.style);

    let nav = ctl.clone();
    let surface = surface.on_key_down(move |e: &KeyDownEvent, window, cx| {
        let n = actions.len();
        if n == 0 {
            return;
        }
        let current = nav.state.read(cx).highlighted;
        let step = |from: Option<usize>, dir: isize| -> Option<usize> {
            let mut i = from.map_or(if dir > 0 { -1 } else { n as isize }, |i| i as isize);
            for _ in 0..n {
                i = (i + dir).rem_euclid(n as isize);
                if !actions[i as usize].2 {
                    return Some(i as usize);
                }
            }
            from
        };
        let next = match e.keystroke.key.as_str() {
            "down" => step(current, 1),
            "up" => step(current, -1),
            "home" => step(None, 1),
            "end" => step(None, -1),
            "enter" | "space" => {
                cx.stop_propagation();
                if let Some(i) = current {
                    let (action, close, _) = actions[i].clone();
                    if close {
                        nav.set_open(false, window, cx);
                    }
                    action(window, cx);
                }
                return;
            }
            "tab" => {
                nav.set_open(false, window, cx);
                return;
            }
            _ => return,
        };
        cx.stop_propagation();
        nav.state.update(cx, |s, cx| {
            s.highlighted = next;
            cx.notify();
        });
    });
    popup(ctl, placement, anchor, surface, window, cx)
}
