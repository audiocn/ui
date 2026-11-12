//! Port of shadcn `components/ui/input.tsx` (Base UI `Input`): a single-line
//! text field with caret, selection, IME, clipboard and the usual editing keys.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, GlobalElementId, Hsla, InspectorElementId,
    InteractiveElement, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Pixels, Point, Refineable, RenderOnce, ShapedLine, SharedString,
    Style, StyleRefinement, Styled, Subscription, TextRun, UTF16Selection, UnderlineStyle, Window,
    actions, div, fill, point, prelude::FluentBuilder, px, relative, size,
};

use super::styles::{by_mode, fit_radius, focus_ring_color, radius_2xl, ring_overlay_bordered};
use crate::theme::ActiveTheme;

actions!(
    audiocn_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectToHome,
        SelectToEnd,
        Paste,
        Cut,
        Copy,
        Submit,
        Cancel,
    ]
);

const CONTEXT: &str = "AudiocnInput";

pub(crate) fn init(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("ctrl-left", WordLeft, c),
        KeyBinding::new("ctrl-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("cmd-left", Home, c),
        KeyBinding::new("cmd-right", End, c),
        KeyBinding::new("shift-home", SelectToHome, c),
        KeyBinding::new("shift-end", SelectToEnd, c),
        KeyBinding::new("cmd-shift-left", SelectToHome, c),
        KeyBinding::new("cmd-shift-right", SelectToEnd, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("enter", Submit, c),
        KeyBinding::new("escape", Cancel, c),
    ]);
}

type TextHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;
type CancelHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Default, Clone)]
struct Callbacks {
    on_change: Option<TextHandler>,
    on_submit: Option<TextHandler>,
    on_blur: Option<TextHandler>,
    on_cancel: Option<CancelHandler>,
    on_focus: Option<CancelHandler>,
    select_on_focus: bool,
    filter: Option<Rc<dyn Fn(char) -> bool>>,
    max_length: Option<usize>,
}

/// Editing state of an [`Input`] (kept in keyed element state).
pub struct InputState {
    focus: FocusHandle,
    content: String,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    is_selecting: bool,
    last_prop: Option<SharedString>,
    callbacks: Callbacks,
    did_auto_focus: bool,
    _subscriptions: Vec<Subscription>,
}

impl InputState {
    fn new(initial: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        let subs = vec![
            cx.on_blur(&focus, window, |this, window, cx| {
                this.is_selecting = false;
                let len = this.content.len();
                this.selected_range = len..len;
                if let Some(cb) = this.callbacks.on_blur.clone() {
                    let text: SharedString = this.content.clone().into();
                    window.defer(cx, move |window, cx| cb(text, window, cx));
                }
                cx.notify();
            }),
            cx.on_focus(&focus, window, |this, window, cx| {
                if this.callbacks.select_on_focus {
                    this.selected_range = 0..this.content.len();
                    this.selection_reversed = false;
                }
                if let Some(cb) = this.callbacks.on_focus.clone() {
                    window.defer(cx, move |window, cx| cb(window, cx));
                }
                cx.notify();
            }),
        ];
        let len = initial.len();
        Self {
            focus,
            content: initial,
            selected_range: len..len,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.0),
            is_selecting: false,
            last_prop: None,
            callbacks: Callbacks::default(),
            did_auto_focus: false,
            _subscriptions: subs,
        }
    }

    /// Current text.
    pub fn text(&self) -> &str {
        &self.content
    }

    fn emit_change(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cb) = self.callbacks.on_change.clone() {
            let text: SharedString = self.content.clone().into();
            window.defer(cx, move |window, cx| cb(text, window, cx));
        }
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content[offset..]
            .chars()
            .next()
            .map_or(self.content.len(), |c| offset + c.len_utf8())
    }

    fn previous_word(&self, offset: usize) -> usize {
        let before = &self.content[..offset];
        let trimmed = before.trim_end_matches(|c: char| !c.is_alphanumeric());
        trimmed
            .char_indices()
            .rev()
            .find(|(_, c)| !c.is_alphanumeric())
            .map_or(0, |(i, c)| i + c.len_utf8())
    }

    fn next_word(&self, offset: usize) -> usize {
        let after = &self.content[offset..];
        let skip = after.len()
            - after
                .trim_start_matches(|c: char| !c.is_alphanumeric())
                .len();
        let rest = &after[skip..];
        let word = rest
            .char_indices()
            .find(|(_, c)| !c.is_alphanumeric())
            .map_or(rest.len(), |(i, _)| i);
        offset + skip + word
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor_offset()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word(self.cursor_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn select_to_home(&mut self, _: &SelectToHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if prev == self.cursor_offset() {
                return;
            }
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if next == self.cursor_offset() {
                return;
            }
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        match self.callbacks.on_submit.clone() {
            Some(cb) => {
                let text: SharedString = self.content.clone().into();
                window.defer(cx, move |window, cx| cb(text, window, cx));
            }
            None => cx.propagate(),
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        match self.callbacks.on_cancel.clone() {
            Some(cb) => window.defer(cx, move |window, cx| cb(window, cx)),
            None => cx.propagate(),
        }
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if self.content.is_empty() {
            return 0;
        }
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_x)
            .min(self.content.len())
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.is_selecting = true;
        let index = self.index_for_position(event.position);
        if event.click_count >= 2 {
            self.selected_range = 0..self.content.len();
            self.selection_reversed = false;
            cx.notify();
        } else if event.modifiers.shift {
            self.select_to(index, cx);
        } else {
            self.move_to(index, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_position(event.position), cx);
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn filtered(&self, text: &str, replaced: usize) -> String {
        let mut out: String = match self.callbacks.filter.as_ref() {
            Some(f) => text.chars().filter(|c| f(*c)).collect(),
            None => text.to_string(),
        };
        if let Some(max) = self.callbacks.max_length {
            let room = max.saturating_sub(self.content.chars().count() - replaced);
            out = out.chars().take(room).collect();
        }
        out
    }
}

impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let replaced = self.content[range.clone()].chars().count();
        let new_text = self.filtered(new_text, replaced);
        if new_text.is_empty() && range.is_empty() {
            return;
        }
        self.content.replace_range(range.clone(), &new_text);
        let caret = range.start + new_text.len();
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.marked_range = None;
        self.emit_change(window, cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content.replace_range(range.clone(), new_text);
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + range.start..r.end + range.start)
            .unwrap_or_else(|| {
                let caret = range.start + new_text.len();
                caret..caret
            });
        self.emit_change(window, cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let left = bounds.left() - self.scroll_x;
        Some(Bounds::from_corners(
            point(left + line.x_for_index(range.start), bounds.top()),
            point(left + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line = self.last_layout.as_ref()?;
        let index = line.index_for_x(point.x - bounds.left() + self.scroll_x)?;
        Some(self.offset_to_utf16(index))
    }
}

/// `type` of [`Input`] that matters for display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputType {
    #[default]
    Text,
    /// Masks the text with `•`.
    Password,
}

/// A single-line text input: `bg-input/50 h-8 rounded-2xl px-2.5 py-1
/// text-sm`, `focus-visible:ring-3 ring-ring/30`.
///
/// Controlled with [`Input::value`] (the text follows the prop whenever the
/// prop changes), uncontrolled with [`Input::default_value`]. Edits go to
/// [`Input::on_change`]; Enter to [`Input::on_submit`], Escape to
/// [`Input::on_cancel`], focus loss to [`Input::on_blur`].
///
/// ```ignore
/// Input::new("gain-edit").value(format!("{db:.1}")).auto_focus(true)
///     .select_on_focus(true)
///     .on_submit(|text, _, cx| commit(text))
///     .on_cancel(|_, cx| stop_editing())
/// ```
#[derive(IntoElement)]
pub struct Input {
    id: ElementId,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    placeholder: SharedString,
    input_type: InputType,
    disabled: bool,
    invalid: bool,
    auto_focus: bool,
    callbacks: Callbacks,
    style: StyleRefinement,
}

impl Input {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: None,
            placeholder: SharedString::default(),
            input_type: InputType::Text,
            disabled: false,
            invalid: false,
            auto_focus: false,
            callbacks: Callbacks::default(),
            style: StyleRefinement::default(),
        }
    }

    /// Controlled text: replaces the content whenever it differs from the
    /// previous render's value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.input_type = input_type;
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

    /// Focus the input the first time it renders (`autoFocus`).
    pub fn auto_focus(mut self, auto_focus: bool) -> Self {
        self.auto_focus = auto_focus;
        self
    }

    /// Select all text whenever the input gains focus.
    pub fn select_on_focus(mut self, select: bool) -> Self {
        self.callbacks.select_on_focus = select;
        self
    }

    /// Only accept characters for which `allow` returns true (typed or pasted).
    pub fn filter(mut self, allow: impl Fn(char) -> bool + 'static) -> Self {
        self.callbacks.filter = Some(Rc::new(allow));
        self
    }

    /// `maxLength` in characters.
    pub fn max_length(mut self, max: usize) -> Self {
        self.callbacks.max_length = Some(max);
        self
    }

    /// Called with the new text after every edit.
    pub fn on_change(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.callbacks.on_change = Some(Rc::new(f));
        self
    }

    /// Enter.
    pub fn on_submit(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.callbacks.on_submit = Some(Rc::new(f));
        self
    }

    /// Focus lost.
    pub fn on_blur(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.callbacks.on_blur = Some(Rc::new(f));
        self
    }

    /// Focus gained.
    pub fn on_focus(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.callbacks.on_focus = Some(Rc::new(f));
        self
    }

    /// Escape.
    pub fn on_cancel(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.callbacks.on_cancel = Some(Rc::new(f));
        self
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The focus handle of the [`Input`] with `id` (rendered in the current
/// element scope), e.g. to focus it programmatically.
pub fn input_focus_handle(
    id: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> FocusHandle {
    window
        .use_keyed_state(id, cx, |window, cx| {
            InputState::new(String::new(), window, cx)
        })
        .read(cx)
        .focus
        .clone()
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self
            .value
            .clone()
            .or(self.default_value.clone())
            .unwrap_or_default()
            .to_string();
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            InputState::new(initial, window, cx)
        });
        let callbacks = self.callbacks.clone();
        let value = self.value.clone();
        state.update(cx, |s, _| {
            s.callbacks = callbacks;
            if let Some(v) = value
                && s.last_prop.as_ref() != Some(&v)
            {
                if s.content != v.as_ref() {
                    s.content = v.to_string();
                    let len = s.content.len();
                    s.selected_range = if s.callbacks.select_on_focus && s.focus.is_focused(window)
                    {
                        0..len
                    } else {
                        len..len
                    };
                    s.marked_range = None;
                }
                s.last_prop = Some(v);
            }
        });
        let focus = state.read(cx).focus.clone();
        if self.auto_focus && !state.read(cx).did_auto_focus {
            state.update(cx, |s, _| s.did_auto_focus = true);
            let f = focus.clone();
            let auto = state.clone();
            window.defer(cx, move |window, cx| {
                window.focus(&f, cx);
                // The focus subscription may not be active yet on first render.
                auto.update(cx, |s, cx| {
                    if s.callbacks.select_on_focus {
                        s.selected_range = 0..s.content.len();
                        s.selection_reversed = false;
                        cx.notify();
                    }
                });
            });
        }
        let focused = focus.is_focused(window);

        let t = cx.theme();
        let height = px(32.0);
        let radius = fit_radius(radius_2xl(t), height);
        let mut border = gpui::transparent_black();
        if focused {
            border = t.ring;
        }
        if self.invalid {
            border = by_mode(t, t.destructive, t.destructive.opacity(0.5));
        }
        let ring = focus_ring_color(t, focused && !self.disabled, self.invalid, 0.3);
        let text_el = TextElement {
            state: state.clone(),
            placeholder: self.placeholder.clone(),
            password: self.input_type == InputType::Password,
            placeholder_color: t.muted_foreground,
            caret_color: t.foreground,
            selection_color: t.ring.opacity(0.35),
        };

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .items_center()
            .w_full()
            .min_w_0()
            .h(height)
            .px(px(10.0))
            .py(px(4.0))
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(t.input.opacity(0.5))
            .text_size(px(14.0))
            .line_height(px(20.0))
            .when_some(ring, |el, ring| {
                el.child(ring_overlay_bordered(radius, 3.0, 1.0, ring))
            })
            .child(div().flex_1().min_w_0().overflow_hidden().child(text_el));
        el.style().refine(&self.style);
        if self.disabled {
            return el.opacity(0.5).cursor_not_allowed();
        }
        el.key_context(CONTEXT)
            .track_focus(&focus)
            .cursor(CursorStyle::IBeam)
            .on_action(window.listener_for(&state, InputState::backspace))
            .on_action(window.listener_for(&state, InputState::delete))
            .on_action(window.listener_for(&state, InputState::delete_word_left))
            .on_action(window.listener_for(&state, InputState::left))
            .on_action(window.listener_for(&state, InputState::right))
            .on_action(window.listener_for(&state, InputState::word_left))
            .on_action(window.listener_for(&state, InputState::word_right))
            .on_action(window.listener_for(&state, InputState::select_left))
            .on_action(window.listener_for(&state, InputState::select_right))
            .on_action(window.listener_for(&state, InputState::select_word_left))
            .on_action(window.listener_for(&state, InputState::select_word_right))
            .on_action(window.listener_for(&state, InputState::select_all))
            .on_action(window.listener_for(&state, InputState::home))
            .on_action(window.listener_for(&state, InputState::end))
            .on_action(window.listener_for(&state, InputState::select_to_home))
            .on_action(window.listener_for(&state, InputState::select_to_end))
            .on_action(window.listener_for(&state, InputState::paste))
            .on_action(window.listener_for(&state, InputState::copy))
            .on_action(window.listener_for(&state, InputState::cut))
            .on_action(window.listener_for(&state, InputState::submit))
            .on_action(window.listener_for(&state, InputState::cancel))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, InputState::on_mouse_down),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, InputState::on_mouse_up),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, InputState::on_mouse_up),
            )
            .on_mouse_move(window.listener_for(&state, InputState::on_mouse_move))
    }
}

struct TextElement {
    state: Entity<InputState>,
    placeholder: SharedString,
    password: bool,
    placeholder_color: Hsla,
    caret_color: Hsla,
    selection_color: Hsla,
}

struct TextPrepaint {
    line: ShapedLine,
    scroll_x: Pixels,
    caret: Option<Bounds<Pixels>>,
    selection: Option<Bounds<Pixels>>,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = TextPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> TextPrepaint {
        let s = self.state.read(cx);
        let style = window.text_style();
        let focused = s.focus.is_focused(window);
        let masked: String;
        let (text, color, map): (SharedString, Hsla, bool) = if s.content.is_empty() {
            (self.placeholder.clone(), self.placeholder_color, false)
        } else if self.password {
            masked = s.content.chars().map(|_| '•').collect();
            (masked.into(), style.color, true)
        } else {
            (s.content.clone().into(), style.color, false)
        };
        // Byte offsets in `content` → byte offsets in the displayed text.
        let to_display = |offset: usize| -> usize {
            if map {
                s.content[..offset].chars().count() * '•'.len_utf8()
            } else if s.content.is_empty() {
                0
            } else {
                offset
            }
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match s
            .marked_range
            .as_ref()
            .filter(|_| !map && !s.content.is_empty())
        {
            Some(marked) => [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect::<Vec<_>>(),
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        let cursor_x = line.x_for_index(to_display(s.cursor_offset()));
        // Keep the caret in view.
        let mut scroll_x = s.scroll_x;
        let width = bounds.size.width;
        if cursor_x - scroll_x > width - px(2.0) {
            scroll_x = cursor_x - width + px(2.0);
        }
        if cursor_x < scroll_x {
            scroll_x = cursor_x;
        }
        scroll_x = scroll_x.max(px(0.0));
        let left = bounds.left() - scroll_x;
        let sel = &s.selected_range;
        let (caret, selection) = if !focused {
            (None, None)
        } else if sel.is_empty() {
            (
                Some(Bounds::new(
                    point(left + cursor_x, bounds.top()),
                    size(px(1.0), bounds.size.height),
                )),
                None,
            )
        } else {
            (
                None,
                Some(Bounds::from_corners(
                    point(left + line.x_for_index(to_display(sel.start)), bounds.top()),
                    point(
                        left + line.x_for_index(to_display(sel.end)),
                        bounds.bottom(),
                    ),
                )),
            )
        };
        TextPrepaint {
            line,
            scroll_x,
            caret,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut TextPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.state.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(fill(selection, self.selection_color));
            }
            let origin = point(bounds.left() - prepaint.scroll_x, bounds.top());
            let _ = prepaint.line.paint(
                origin,
                window.line_height(),
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(caret) = prepaint.caret.take() {
                window.paint_quad(fill(caret, self.caret_color));
            }
        });
        let line = prepaint.line.clone();
        let scroll_x = prepaint.scroll_x;
        let has_content = !self.state.read(cx).content.is_empty();
        self.state.update(cx, |s, _| {
            s.last_layout = has_content.then_some(line);
            s.last_bounds = Some(bounds);
            s.scroll_x = scroll_x;
        });
    }
}
