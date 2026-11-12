//! Port of shadcn `components/ui/select.tsx` (Base UI `Select`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Pixels, Refineable, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, div, point, prelude::FluentBuilder, px,
};

use super::icons::{Icon, IconName};
use super::overlay::{
    Align, OpenHandler, OverlayCtl, Placement, Side, measure_bounds, popup, trigger, use_overlay,
};
use super::styles::{
    TextSize, by_mode, fit_radius, focus_ring_color, parent_styled, provide, radius_2xl, radius_xl,
    ring_overlay_bordered, ring_shadow, shadow_lg, use_context,
};
use crate::theme::ActiveTheme;

type ValueHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;
type ValueRenderer = Rc<dyn Fn(Option<&SharedString>, Option<&SharedString>) -> AnyElement>;

/// `size` of [`SelectTrigger`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectSize {
    #[default]
    Default,
    Sm,
}

/// What a [`SelectValue`] inside the trigger reads.
#[derive(Clone)]
struct SelectValueCx {
    value: Option<SharedString>,
    label: Option<SharedString>,
}

/// A select: a [`SelectTrigger`] showing the [`SelectValue`], and a
/// [`SelectContent`] list of [`SelectItem`]s (optionally in [`SelectGroup`]s
/// with [`SelectLabel`]s and [`SelectSeparator`]s). Arrow keys, Home / End,
/// Enter / Space and Escape work like Base UI; the selected item shows a check
/// and, by default, the popup opens with the selected item over the trigger.
///
/// ```ignore
/// Select::new("rate").default_value("48000")
///     .trigger(SelectTrigger::new().child(SelectValue::new().placeholder("Sample rate")))
///     .content(SelectContent::new().child(SelectGroup::new()
///         .child(SelectItem::new("44100", "44.1 kHz"))
///         .child(SelectItem::new("48000", "48 kHz"))))
///     .on_value_change(|v, _, _| println!("{v}"))
/// ```
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    value: Option<Option<SharedString>>,
    default_value: Option<SharedString>,
    on_value_change: Option<ValueHandler>,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenHandler>,
    disabled: bool,
    invalid: bool,
    trigger: SelectTrigger,
    content: SelectContent,
}

struct SelectSelection {
    value: Option<SharedString>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: None,
            on_value_change: None,
            open: None,
            default_open: false,
            on_open_change: None,
            disabled: false,
            invalid: false,
            trigger: SelectTrigger::new().child(SelectValue::new()),
            content: SelectContent::new(),
        }
    }

    /// Controlled value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(Some(value.into()));
        self
    }

    /// Controlled value that may be empty (`null`).
    pub fn selected(mut self, value: Option<SharedString>) -> Self {
        self.value = Some(value);
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn on_value_change(
        mut self,
        f: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `aria-invalid` on the trigger.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn trigger(mut self, trigger: SelectTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn content(mut self, content: SelectContent) -> Self {
        self.content = content;
        self
    }
}

/// Flattened list rows.
#[allow(clippy::large_enum_variant)]
enum Row {
    Item(SelectItem),
    Label(SelectLabel),
    Separator(SelectSeparator),
    /// `SelectGroup`'s `p-1` (top or bottom).
    Pad,
    Custom(AnyElement),
}

fn flatten(nodes: Vec<SelectNode>, out: &mut Vec<Row>) {
    for node in nodes {
        match node {
            SelectNode::Item(i) => out.push(Row::Item(i)),
            SelectNode::Label(l) => out.push(Row::Label(l)),
            SelectNode::Separator(s) => out.push(Row::Separator(s)),
            SelectNode::Custom(e) => out.push(Row::Custom(e)),
            SelectNode::Group(g) => {
                out.push(Row::Pad);
                flatten(g.children, out);
                out.push(Row::Pad);
            }
        }
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let selection =
            window.use_keyed_state(ElementId::Name(format!("{id}-value").into()), cx, |_, _| {
                SelectSelection {
                    value: self.default_value.clone(),
                }
            });
        let controlled = self.value.is_some();
        let value = match self.value.clone() {
            Some(v) => v,
            None => selection.read(cx).value.clone(),
        };
        let ctl = use_overlay(
            id.clone(),
            self.open,
            self.default_open,
            self.on_open_change.clone(),
            true,
            window,
            cx,
        );
        let mut rows = Vec::new();
        flatten(self.content.children, &mut rows);
        let item_values: Vec<(SharedString, bool)> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(i) => Some((i.value.clone(), i.disabled)),
                _ => None,
            })
            .collect();
        let label = rows.iter().find_map(|r| match r {
            Row::Item(i) if Some(&i.value) == value.as_ref() => Some(i.label.clone()),
            _ => None,
        });
        let selected_index = item_values
            .iter()
            .position(|(v, _)| Some(v) == value.as_ref());

        let open = ctl.is_open(cx);
        // Highlight the selected item when the popup opens.
        if open && ctl.state.read(cx).highlighted.is_none() {
            // Base UI highlights the selected item; with nothing selected it only
            // highlights the first item when the popup was opened from the keyboard.
            let keyboard = window.last_input_was_keyboard();
            let start = selected_index.or_else(|| {
                keyboard
                    .then(|| item_values.iter().position(|(_, d)| !d))
                    .flatten()
            });
            ctl.state.update(cx, |s, _| s.highlighted = start);
            if let Some(i) = start {
                ctl.state
                    .read(cx)
                    .scroll
                    .scroll_to_item(row_index(&rows, i));
            }
        }

        let commit: ValueHandler = {
            let selection = selection.clone();
            let handler = self.on_value_change.clone();
            let ctl = ctl.clone();
            let current = value.clone();
            Rc::new(move |v: SharedString, window: &mut Window, cx: &mut App| {
                ctl.set_open(false, window, cx);
                if current.as_ref() == Some(&v) {
                    return;
                }
                if !controlled {
                    selection.update(cx, |s, cx| {
                        s.value = Some(v.clone());
                        cx.notify();
                    });
                }
                if let Some(h) = handler.as_ref() {
                    h(v, window, cx);
                }
            })
        };

        // ---- trigger
        let t = cx.theme();
        let disabled = self.disabled;
        let trig = self.trigger;
        let height = match trig.size {
            SelectSize::Default => px(32.0),
            SelectSize::Sm => px(28.0),
        };
        let radius = fit_radius(radius_2xl(t), height);
        let trigger_focus = ctl.state.read(cx).trigger_focus.clone();
        let focus_visible =
            !disabled && trigger_focus.is_focused(window) && window.last_input_was_keyboard();
        let mut border = gpui::transparent_black();
        if focus_visible {
            border = t.ring;
        }
        if self.invalid {
            border = by_mode(t, t.destructive, t.destructive.opacity(0.5));
        }
        let ring = focus_ring_color(t, focus_visible, self.invalid, 0.3);
        let mut trigger_el = div()
            .relative()
            .flex()
            .flex_1()
            .items_center()
            .justify_between()
            .gap(px(6.0))
            .h(height)
            .px(px(12.0))
            .py(px(8.0))
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(t.input.opacity(0.5))
            .text_sm_()
            .whitespace_nowrap()
            .when(disabled, |el| el.opacity(0.5).cursor_not_allowed())
            .when_some(ring, |el, ring| {
                el.child(ring_overlay_bordered(radius, 3.0, 1.0, ring))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    .children(trig.children),
            )
            .child(
                Icon::new(IconName::CaretDown)
                    .size(px(16.0))
                    .color(t.muted_foreground),
            );
        // Sizing / placement props go on the outer trigger wrapper, the rest
        // (colours, padding, ...) on the trigger surface.
        let outer_style = StyleRefinement {
            size: trig.style.size.clone(),
            min_size: trig.style.min_size.clone(),
            max_size: trig.style.max_size.clone(),
            margin: trig.style.margin.clone(),
            flex_grow: trig.style.flex_grow,
            flex_shrink: trig.style.flex_shrink,
            flex_basis: trig.style.flex_basis,
            align_self: trig.style.align_self,
            ..StyleRefinement::default()
        };
        let mut inner_style = trig.style;
        inner_style.margin = Default::default();
        inner_style.size.width = None;
        inner_style.min_size = Default::default();
        inner_style.max_size = Default::default();
        trigger_el.style().refine(&inner_style);
        let trigger_el = provide(
            SelectValueCx {
                value: value.clone(),
                label: label.clone(),
            },
            trigger_el,
        );

        let mut root = if disabled {
            div()
                .id(id.clone())
                .flex()
                .child(trigger_el.into_any_element())
        } else {
            trigger(id.clone(), &ctl, trigger_el.into_any_element(), cx)
                .track_focus(&trigger_focus)
                .role(gpui::Role::ComboBox)
                .aria_expanded(open)
        };
        root.style().refine(&outer_style);

        let anchor = ctl.state.read(cx).trigger_bounds;
        let (true, Some(anchor)) = (open && !disabled, anchor) else {
            return root;
        };

        // ---- popup
        let content_style = self.content.style;
        let align_item = self.content.align_item_with_trigger && selected_index.is_some();
        let mut placement = Placement::new(
            self.content.side,
            self.content.align,
            self.content.side_offset,
            self.content.align_offset,
        );
        let viewport = window.viewport_size();
        let max_h;
        if align_item {
            // Put the selected item over the trigger (Base UI `alignItemWithTrigger`).
            // Its offset inside the popup is measured over the first frames.
            if ctl.state.read(cx).selected_item_offset.is_none() {
                window.request_animation_frame();
            }
            let offset = ctl
                .state
                .read(cx)
                .selected_item_offset
                .unwrap_or_else(|| estimate_offset(&rows, selected_index.unwrap_or(0)));
            let item_h = px(32.0);
            let y = anchor.top() + (anchor.size.height - item_h) * 0.5 - offset;
            placement = Placement::new(Side::Bottom, Align::Start, 0.0, 0.0);
            let shifted = gpui::Bounds::new(
                point(anchor.left(), y),
                gpui::size(anchor.size.width, px(0.0)),
            );
            max_h = viewport.height - px(16.0);
            return root.child(select_popup(
                &ctl,
                placement,
                shifted,
                rows,
                &item_values,
                value.clone(),
                commit,
                anchor.size.width,
                max_h,
                content_style,
                window,
                cx,
            ));
        }
        max_h = match placement.side {
            Side::Bottom => viewport.height - anchor.bottom() - px(placement.side_offset) - px(8.0),
            Side::Top => anchor.top() - px(placement.side_offset) - px(8.0),
            _ => viewport.height - px(16.0),
        }
        .max(px(120.0));
        root.child(select_popup(
            &ctl,
            placement,
            anchor,
            rows,
            &item_values,
            value,
            commit,
            anchor.size.width,
            max_h,
            content_style,
            window,
            cx,
        ))
    }
}

/// Index of the `item`-th item among all rows.
fn row_index(rows: &[Row], item: usize) -> usize {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Row::Item(_)))
        .nth(item)
        .map_or(0, |(i, _)| i)
}

/// Fallback y offset of an item before it has been measured.
fn estimate_offset(rows: &[Row], item: usize) -> Pixels {
    let mut y = px(0.0);
    let mut n = 0;
    for r in rows {
        match r {
            Row::Item(_) => {
                if n == item {
                    return y;
                }
                n += 1;
                y += px(32.0);
            }
            Row::Label(_) => y += px(24.0),
            Row::Separator(_) => y += px(9.0),
            Row::Pad => y += px(4.0),
            Row::Custom(_) => y += px(32.0),
        }
    }
    y
}

#[allow(clippy::too_many_arguments)]
fn select_popup(
    ctl: &OverlayCtl,
    placement: Placement,
    anchor: gpui::Bounds<Pixels>,
    rows: Vec<Row>,
    items: &[(SharedString, bool)],
    value: Option<SharedString>,
    commit: ValueHandler,
    width: Pixels,
    max_h: Pixels,
    style: StyleRefinement,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let t = cx.theme();
    let highlighted = ctl.state.read(cx).highlighted;
    let highlight_bg = t.foreground.opacity(0.1);
    let fg = t.popover_foreground;
    let accent_fg = t.accent_foreground;
    let item_radius = radius_xl(t);
    let scroll = ctl.state.read(cx).scroll.clone();
    let content_top = ctl.state.read(cx).content_bounds.map(|b| b.top());

    let mut list = div()
        .id("select-list")
        .flex()
        .flex_col()
        .max_h(max_h)
        .overflow_y_scroll()
        .track_scroll(&scroll);

    let item_rows: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Row::Item(_)))
        .map(|(i, _)| i)
        .collect();
    let mut item_ix = 0usize;
    for row in rows {
        match row {
            Row::Pad => list = list.child(div().h(px(4.0)).flex_shrink_0()),
            Row::Custom(e) => list = list.child(div().mx(px(4.0)).child(e)),
            Row::Label(l) => {
                list = list.child(
                    div()
                        .mx(px(4.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .text_xs_()
                        .text_color(t.muted_foreground)
                        .children(l.children),
                );
            }
            Row::Separator(_) => {
                list = list.child(
                    div()
                        .my(px(4.0))
                        .h(px(1.0))
                        .flex_shrink_0()
                        .bg(t.foreground.opacity(0.05)),
                );
            }
            Row::Item(item) => {
                let ix = item_ix;
                item_ix += 1;
                let is_highlighted = highlighted == Some(ix);
                let is_selected = value.as_ref() == Some(&item.value);
                let disabled = item.disabled;
                let top_aligned = item.style.align_items == Some(gpui::AlignItems::FlexStart);
                let hover_state = ctl.state.clone();
                let click_commit = commit.clone();
                let v = item.value.clone();
                let measure_state = ctl.state.clone();
                let mut el = div()
                    .id(ElementId::Name(
                        format!("select-item-{}", item.value).into(),
                    ))
                    .relative()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(8.0))
                    .mx(px(4.0))
                    .min_h(px(28.0))
                    .rounded(item_radius)
                    .py(px(6.0))
                    .pr(px(32.0))
                    .pl(px(8.0))
                    .text_sm_()
                    .text_color(if is_highlighted { accent_fg } else { fg })
                    .when(is_highlighted, |el| el.bg(highlight_bg))
                    .when(disabled, |el| el.opacity(0.5))
                    .role(gpui::Role::ListBoxOption)
                    .aria_selected(is_selected)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(8.0))
                            .whitespace_nowrap()
                            .map(|el| {
                                if item.children.is_empty() {
                                    el.child(item.label.clone())
                                } else {
                                    el.children(item.children)
                                }
                            }),
                    )
                    .when(is_selected, |el| {
                        // `absolute right-2 flex size-4 items-center`: an absolute child
                        // follows the row's align-items, so a top-aligned (multi-line)
                        // item gets its check on the first line, at the top padding.
                        el.child(
                            div()
                                .absolute()
                                .right(px(8.0))
                                .when(top_aligned, |el| el.top(px(6.0)).h(px(16.0)))
                                .when(!top_aligned, |el| el.top_0().bottom_0())
                                .flex()
                                .items_center()
                                .child(Icon::new(IconName::Check).size(px(16.0))),
                        )
                        .when_some(content_top, |el, top| {
                            el.child(measure_bounds(move |b, _, cx| {
                                measure_state.update(cx, |s, _| {
                                    s.selected_item_offset = Some(b.top() - top);
                                });
                            }))
                        })
                    });
                el.style().refine(&item.style);
                if !disabled {
                    el = el
                        .on_mouse_move(move |_, _, cx| {
                            hover_state.update(cx, |s, cx| {
                                if s.highlighted != Some(ix) {
                                    s.highlighted = Some(ix);
                                    cx.notify();
                                }
                            });
                        })
                        .on_click(move |_, window, cx| click_commit(v.clone(), window, cx));
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
    let mut surface = div()
        .relative()
        .w(width)
        .min_w(px(144.0))
        .rounded(radius_2xl(t))
        .overflow_hidden()
        .bg(t.popover)
        .text_color(t.popover_foreground)
        .font_family(t.font_sans.clone())
        .shadow(shadows)
        .child(list);
    surface.style().refine(&style);

    // Keyboard navigation (the popup holds focus while open).
    let nav = ctl.clone();
    let enabled: Vec<bool> = items.iter().map(|(_, d)| !d).collect();
    let values: Vec<SharedString> = items.iter().map(|(v, _)| v.clone()).collect();
    let key_commit = commit;
    let surface = surface.on_key_down(move |e: &KeyDownEvent, window, cx| {
        let n = enabled.len();
        if n == 0 {
            return;
        }
        let current = nav.state.read(cx).highlighted;
        let step = |from: Option<usize>, dir: isize| -> Option<usize> {
            let mut i = from.map_or(if dir > 0 { -1 } else { n as isize }, |i| i as isize);
            for _ in 0..n {
                i += dir;
                if i < 0 || i >= n as isize {
                    return from;
                }
                if enabled[i as usize] {
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
                    key_commit(values[i].clone(), window, cx);
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
            if let Some(i) = next {
                s.scroll.scroll_to_item(item_rows[i]);
            }
            cx.notify();
        });
    });
    popup(ctl, placement, anchor, surface, window, cx)
}

/// The trigger button (`SelectTrigger`). Put a [`SelectValue`] (and any icon)
/// in it; the caret is added automatically.
#[derive(Default)]
pub struct SelectTrigger {
    size: SelectSize,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SelectTrigger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(mut self, size: SelectSize) -> Self {
        self.size = size;
        self
    }
}
parent_styled!(SelectTrigger);

/// Shows the selected item's label, or the placeholder (`text-muted-foreground`).
#[derive(IntoElement, Default)]
pub struct SelectValue {
    placeholder: Option<SharedString>,
    render: Option<ValueRenderer>,
    style: StyleRefinement,
}

impl SelectValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Custom rendering from `(value, label)` (the React children function).
    pub fn render(
        mut self,
        f: impl Fn(Option<&SharedString>, Option<&SharedString>) -> AnyElement + 'static,
    ) -> Self {
        self.render = Some(Rc::new(f));
        self
    }
}

impl Styled for SelectValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SelectValue {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let vcx = use_context::<SelectValueCx>().unwrap_or(SelectValueCx {
            value: None,
            label: None,
        });
        let muted = cx.theme().muted_foreground;
        let mut el: Div = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(6.0))
            .text_left()
            .overflow_hidden()
            .text_ellipsis();
        el = match (self.render, vcx.value.as_ref()) {
            (Some(r), _) => el.child(r(vcx.value.as_ref(), vcx.label.as_ref())),
            (None, Some(v)) => el.child(vcx.label.clone().unwrap_or_else(|| v.clone())),
            (None, None) => el
                .text_color(muted)
                .child(self.placeholder.unwrap_or_default()),
        };
        el.style().refine(&self.style);
        el
    }
}

/// A node of [`SelectContent`].
#[allow(clippy::large_enum_variant)]
pub enum SelectNode {
    Item(SelectItem),
    Group(SelectGroup),
    Label(SelectLabel),
    Separator(SelectSeparator),
    Custom(AnyElement),
}

impl From<SelectItem> for SelectNode {
    fn from(v: SelectItem) -> Self {
        Self::Item(v)
    }
}
impl From<SelectGroup> for SelectNode {
    fn from(v: SelectGroup) -> Self {
        Self::Group(v)
    }
}
impl From<SelectLabel> for SelectNode {
    fn from(v: SelectLabel) -> Self {
        Self::Label(v)
    }
}
impl From<SelectSeparator> for SelectNode {
    fn from(v: SelectSeparator) -> Self {
        Self::Separator(v)
    }
}
impl From<AnyElement> for SelectNode {
    fn from(v: AnyElement) -> Self {
        Self::Custom(v)
    }
}

/// The popup list (`SelectContent`).
pub struct SelectContent {
    side: Side,
    align: Align,
    side_offset: f32,
    align_offset: f32,
    align_item_with_trigger: bool,
    children: Vec<SelectNode>,
    style: StyleRefinement,
}

impl Default for SelectContent {
    fn default() -> Self {
        Self {
            side: Side::Bottom,
            align: Align::Center,
            side_offset: 4.0,
            align_offset: 0.0,
            align_item_with_trigger: true,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl SelectContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item, group, label, separator or any element.
    pub fn child(mut self, node: impl Into<SelectNode>) -> Self {
        self.children.push(node.into());
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<SelectNode>>) -> Self {
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

    /// Default `true`: open with the selected item over the trigger.
    pub fn align_item_with_trigger(mut self, align: bool) -> Self {
        self.align_item_with_trigger = align;
        self
    }
}

impl Styled for SelectContent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// `p-1` group of items.
#[derive(Default)]
pub struct SelectGroup {
    children: Vec<SelectNode>,
}

impl SelectGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, node: impl Into<SelectNode>) -> Self {
        self.children.push(node.into());
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<SelectNode>>) -> Self {
        self.children.extend(nodes.into_iter().map(Into::into));
        self
    }
}

/// An option. `label` is shown in the trigger; children (if any) replace it
/// in the list.
pub struct SelectItem {
    value: SharedString,
    label: SharedString,
    disabled: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SelectItem {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
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
parent_styled!(SelectItem);

/// `text-muted-foreground px-2 py-1 text-xs` group heading.
pub struct SelectLabel {
    children: Vec<AnyElement>,
}

impl SelectLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            children: vec![text.into().into_any_element()],
        }
    }
}

/// `-mx-1 my-1 h-px` rule.
#[derive(Default)]
pub struct SelectSeparator;

impl SelectSeparator {
    pub fn new() -> Self {
        Self
    }
}
