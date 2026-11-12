//! Port of `components/ui/tempo-control.tsx`: an editable BPM field (Base UI
//! `NumberField`) with a tap tempo button, keyboard stepping and a rolling
//! average of the last four tap intervals ([`TapTempo`]).
//!
//! `<TempoControl>` provides its state to `<TempoControlInput>` and
//! `<TempoControlTap>` through context; here the same happens through
//! [`crate::ui::styles::provide`], so the parts work as children of a
//! [`TempoControl`] in any layout.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ClipboardItem, Context, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point,
    Refineable, RenderOnce, ShapedLine, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Subscription, TextRun, UTF16Selection, Window, canvas, div, fill, point,
    prelude::FluentBuilder, px, size,
};

use crate::config::{AudioConfig, AudioSize};
use crate::motion::Instant;
use crate::signal::tap_tempo::{TapTempo, TapTempoOptions, TempoChangeDetails, TempoChangeReason};
use crate::theme::ActiveTheme;
use crate::ui::styles::{
    TextSize, fit_radius, provide, radius_lg, ring_overlay, ring_overlay_bordered, use_context,
};

// ---------------------------------------------------------------------------
// Number field logic (Base UI `NumberField` + `TempoControl`), GPUI-free.

/// A change [`TempoField`] reports to the component's callbacks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TempoEvent {
    /// `onValueChange(value, details)`.
    Change(f32, TempoChangeDetails),
    /// `onValueCommitted(value)`.
    Commit(f32),
}

/// A keyboard step of the BPM field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempoStep {
    /// ArrowUp (`step`, or `largeStep` with Shift, `smallStep` with Alt).
    Up { large: bool },
    /// ArrowDown.
    Down { large: bool },
    /// PageUp: `largeStep`.
    PageUp,
    /// PageDown.
    PageDown,
    /// Home: the minimum.
    Home,
    /// End: the maximum.
    End,
}

/// What the BPM field shows and how edits, keys, taps and blur turn into
/// [`TempoEvent`]s: the state behind [`TempoControl`], kept free of GPUI so
/// it can be tested (and reused by custom tempo UIs).
///
/// ```ignore
/// let mut field = TempoField::new(TapTempoOptions::default(), 10.0);
/// field.edit("96.4");          // [Change(96, Input)]
/// field.blur();                // [Commit(96)]
/// field.step(TempoStep::End);  // [Change(600, Keyboard), Commit(600)]
/// ```
#[derive(Clone, Debug)]
pub struct TempoField {
    tempo: TapTempo,
    large_step: f32,
    /// `empty` in React: the input was cleared (Base UI value `null`).
    empty: bool,
    /// Text typed since the last commit (Base UI keeps it until blur).
    draft: Option<String>,
}

fn parse_bpm(text: &str) -> Option<f32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

fn format_bpm(value: f32) -> String {
    // `maximumFractionDigits: 0, useGrouping: false`.
    format!("{}", value.round() as i64)
}

impl TempoField {
    pub fn new(options: TapTempoOptions, large_step: f32) -> Self {
        Self {
            tempo: TapTempo::new(options),
            large_step,
            empty: false,
            draft: None,
        }
    }

    /// Applies the latest props (call on every render).
    pub fn set_options(&mut self, options: TapTempoOptions, large_step: f32) {
        self.tempo.set_options(options);
        self.large_step = large_step;
    }

    /// The current whole BPM.
    pub fn value(&self) -> f32 {
        self.tempo.value()
    }

    pub fn tap_tempo(&self) -> &TapTempo {
        &self.tempo
    }

    fn editable(&self) -> bool {
        let o = self.tempo.options();
        !(o.disabled || o.read_only)
    }

    /// The input's text.
    pub fn display(&self) -> String {
        if let Some(draft) = &self.draft {
            return draft.clone();
        }
        if self.empty {
            return String::new();
        }
        format_bpm(self.value())
    }

    /// `largeStep`, rounded, at least 1.
    pub fn large_step(&self) -> f32 {
        self.large_step.round().max(1.0)
    }

    /// Forgets the tap history (`resetTaps`).
    pub fn reset_taps(&mut self) {
        self.tempo.reset_taps();
    }

    /// The input's text changed (typing, paste, cut). Partial or empty text
    /// keeps the last value; any edit restarts the tap measurement.
    pub fn edit(&mut self, text: &str) -> Vec<TempoEvent> {
        if !self.editable() {
            return Vec::new();
        }
        self.draft = Some(text.to_string());
        self.tempo.reset_taps();
        match parse_bpm(text) {
            None => {
                self.empty = true;
                Vec::new()
            }
            Some(next) => {
                self.empty = false;
                self.tempo
                    .set_value(next)
                    .map(|v| {
                        vec![TempoEvent::Change(
                            v,
                            TempoChangeDetails::new(TempoChangeReason::Input),
                        )]
                    })
                    .unwrap_or_default()
            }
        }
    }

    /// The input lost focus (or Enter): commits a typed value and shows the
    /// formatted BPM again; an empty edit restores the last value.
    pub fn blur(&mut self) -> Vec<TempoEvent> {
        let Some(draft) = self.draft.take() else {
            return Vec::new();
        };
        self.empty = false;
        match parse_bpm(&draft) {
            Some(next) if self.editable() => {
                let committed = next.round().clamp(self.tempo.min(), self.tempo.max());
                vec![TempoEvent::Commit(committed)]
            }
            _ => Vec::new(),
        }
    }

    /// Arrow keys, PageUp/PageDown, Home/End.
    pub fn step(&mut self, step: TempoStep) -> Vec<TempoEvent> {
        if !self.editable() {
            return Vec::new();
        }
        let current = self.value();
        let (min, max) = (self.tempo.min(), self.tempo.max());
        let large = self.large_step();
        let next = match step {
            TempoStep::Up { large: true } | TempoStep::PageUp => current + large,
            TempoStep::Down { large: true } | TempoStep::PageDown => current - large,
            TempoStep::Up { large: false } => current + 1.0,
            TempoStep::Down { large: false } => current - 1.0,
            TempoStep::Home => min,
            TempoStep::End => max,
        }
        .clamp(min, max);
        self.draft = None;
        self.empty = false;
        let mut events = Vec::new();
        if let Some(v) = self.tempo.set_value(next) {
            events.push(TempoEvent::Change(
                v,
                TempoChangeDetails::new(TempoChangeReason::Keyboard),
            ));
        }
        events.push(TempoEvent::Commit(next));
        events
    }

    /// A tap at `now_ms` (monotonic milliseconds).
    pub fn tap(&mut self, now_ms: f64) -> Vec<TempoEvent> {
        let outcome = self.tempo.tap(now_ms);
        let mut events = Vec::new();
        if let Some(v) = outcome.changed {
            self.draft = None;
            events.push(TempoEvent::Change(
                v,
                TempoChangeDetails::new(TempoChangeReason::Tap),
            ));
        }
        if let Some(v) = outcome.committed {
            self.empty = false;
            self.draft = None;
            events.push(TempoEvent::Commit(v));
        }
        events
    }
}

// ---------------------------------------------------------------------------
// Element state

type ChangeHandler = Rc<dyn Fn(f32, TempoChangeDetails, &mut Window, &mut App) + 'static>;
type CommitHandler = Rc<dyn Fn(f32, &mut Window, &mut App) + 'static>;
type TextHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;

#[derive(Clone, Default)]
struct Callbacks {
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    on_input_change: Option<TextHandler>,
}

/// Keyed state of a [`TempoControl`], shared with its input and tap parts.
pub struct TempoControlState {
    field: TempoField,
    input_focus: FocusHandle,
    tap_focus: FocusHandle,
    /// Byte range in the displayed text (ASCII).
    selection: Range<usize>,
    selection_reversed: bool,
    selecting: bool,
    last_line: Option<ShapedLine>,
    /// Where the text was painted (its left edge) and the input's bounds.
    last_origin_x: Pixels,
    last_bounds: Option<Bounds<Pixels>>,
    callbacks: Callbacks,
    _subscriptions: Vec<Subscription>,
}

fn now_ms() -> f64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = *EPOCH.get_or_init(Instant::now);
    Instant::now().duration_since(epoch).as_secs_f64() * 1000.0
}

fn allowed_char(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+')
}

impl TempoControlState {
    fn new(field: TempoField, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_focus = cx.focus_handle().tab_stop(true);
        let tap_focus = cx.focus_handle().tab_stop(true);
        let subs = vec![
            cx.on_blur(&input_focus, window, |this, window, cx| {
                this.selecting = false;
                let events = this.field.blur();
                this.collapse_to_end();
                this.emit_deferred(events, window, cx);
                cx.notify();
            }),
            cx.on_focus(&input_focus, window, |this, _, cx| {
                this.collapse_to_end();
                cx.notify();
            }),
        ];
        Self {
            field,
            input_focus,
            tap_focus,
            selection: 0..0,
            selection_reversed: false,
            selecting: false,
            last_line: None,
            last_origin_x: px(0.0),
            last_bounds: None,
            callbacks: Callbacks::default(),
            _subscriptions: subs,
        }
    }

    /// The tempo logic (value, display text, tap history).
    pub fn field(&self) -> &TempoField {
        &self.field
    }

    fn text_len(&self) -> usize {
        self.field.display().len()
    }

    fn collapse_to_end(&mut self) {
        let len = self.text_len();
        self.selection = len..len;
        self.selection_reversed = false;
    }

    fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn move_to(&mut self, offset: usize) {
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    fn select_to(&mut self, offset: usize) {
        if self.selection_reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.text_len();
        self.selection.start = self.selection.start.min(len);
        self.selection.end = self.selection.end.min(len);
    }

    /// Replaces `range` of the displayed text and reports the edit.
    fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.field.editable() {
            return;
        }
        let inserted: String = text.chars().filter(|c| allowed_char(*c)).collect();
        if inserted.is_empty() && range.is_empty() {
            return;
        }
        let mut next = self.field.display();
        let range = range.start.min(next.len())..range.end.min(next.len());
        next.replace_range(range.clone(), &inserted);
        let caret = range.start + inserted.len();
        let events = self.field.edit(&next);
        self.move_to(caret);
        if let Some(cb) = self.callbacks.on_input_change.clone() {
            let text: SharedString = next.into();
            window.defer(cx, move |window, cx| cb(text, window, cx));
        }
        self.emit_deferred(events, window, cx);
        cx.notify();
    }

    fn emit_deferred(&self, events: Vec<TempoEvent>, window: &mut Window, cx: &mut Context<Self>) {
        if events.is_empty() {
            return;
        }
        let callbacks = self.callbacks.clone();
        window.defer(cx, move |window, cx| emit(&callbacks, events, window, cx));
    }

    fn index_for_x(&self, x: Pixels) -> usize {
        match &self.last_line {
            Some(line) if x > self.last_origin_x => {
                line.closest_index_for_x(x - self.last_origin_x)
            }
            Some(_) => 0,
            None => self.text_len(),
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.input_focus, cx);
        self.selecting = true;
        let index = self.index_for_x(event.position.x);
        if event.click_count >= 2 {
            self.selection = 0..self.text_len();
            self.selection_reversed = false;
        } else if event.modifiers.shift {
            self.select_to(index);
        } else {
            self.move_to(index);
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.select_to(self.index_for_x(event.position.x));
            cx.notify();
        }
    }

    /// Editing keys of the BPM input. Returns whether the key was handled.
    fn on_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let ks = &event.keystroke;
        let m = ks.modifiers;
        let secondary = m.secondary();
        let step = match ks.key.as_str() {
            "up" => Some(TempoStep::Up { large: m.shift }),
            "down" => Some(TempoStep::Down { large: m.shift }),
            "pageup" => Some(TempoStep::PageUp),
            "pagedown" => Some(TempoStep::PageDown),
            "home" if !m.shift => Some(TempoStep::Home),
            "end" if !m.shift => Some(TempoStep::End),
            _ => None,
        };
        if let Some(step) = step {
            let events = self.field.step(step);
            self.collapse_to_end();
            self.emit_deferred(events, window, cx);
            cx.notify();
            return true;
        }
        self.clamp_selection();
        let len = self.text_len();
        match ks.key.as_str() {
            "left" => {
                let to = if m.alt || secondary {
                    0
                } else if self.selection.is_empty() || m.shift {
                    self.cursor().saturating_sub(1)
                } else {
                    self.selection.start
                };
                if m.shift {
                    self.select_to(to);
                } else {
                    self.move_to(to);
                }
            }
            "right" => {
                let to = if m.alt || secondary {
                    len
                } else if self.selection.is_empty() || m.shift {
                    (self.cursor() + 1).min(len)
                } else {
                    self.selection.end
                };
                if m.shift {
                    self.select_to(to);
                } else {
                    self.move_to(to);
                }
            }
            "home" => self.select_to(0),
            "end" => self.select_to(len),
            "backspace" => {
                let range = if self.selection.is_empty() {
                    let start = if m.alt || secondary {
                        0
                    } else {
                        self.cursor().saturating_sub(1)
                    };
                    start..self.cursor()
                } else {
                    self.selection.clone()
                };
                self.replace(range, "", window, cx);
            }
            "delete" => {
                let range = if self.selection.is_empty() {
                    self.cursor()..(self.cursor() + 1).min(len)
                } else {
                    self.selection.clone()
                };
                self.replace(range, "", window, cx);
            }
            "enter" => {
                let events = self.field.blur();
                self.collapse_to_end();
                self.emit_deferred(events, window, cx);
            }
            "a" if secondary => {
                self.selection = 0..len;
                self.selection_reversed = false;
            }
            "c" | "x" if secondary => {
                if !self.selection.is_empty() {
                    let text = self.field.display()[self.selection.clone()].to_string();
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    if ks.key == "x" {
                        self.replace(self.selection.clone(), "", window, cx);
                    }
                }
            }
            "v" if secondary => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.replace(self.selection.clone(), &text, window, cx);
                }
            }
            _ => return false,
        }
        cx.notify();
        true
    }
}

fn emit(callbacks: &Callbacks, events: Vec<TempoEvent>, window: &mut Window, cx: &mut App) {
    for event in events {
        match event {
            TempoEvent::Change(v, details) => {
                if let Some(cb) = &callbacks.on_value_change {
                    cb(v, details, window, cx);
                }
            }
            TempoEvent::Commit(v) => {
                if let Some(cb) = &callbacks.on_value_committed {
                    cb(v, window, cx);
                }
            }
        }
    }
}

impl EntityInputHandler for TempoControlState {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.field.display();
        let range = range.start.min(text.len())..range.end.min(text.len());
        actual_range.replace(range.clone());
        Some(text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.clamp_selection();
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clamp_selection();
        let range = range.unwrap_or(self.selection.clone());
        self.replace(range, text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_line.as_ref()?;
        let x = self.last_origin_x;
        Some(Bounds::from_corners(
            point(x + line.x_for_index(range.start), bounds.top()),
            point(x + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?;
        Some(self.index_for_x(point.x))
    }
}

/// What [`TempoControl`] hands its parts (React's `TempoContext`).
#[derive(Clone)]
struct TempoContext {
    state: Entity<TempoControlState>,
    disabled: bool,
    read_only: bool,
    height: Pixels,
}

fn use_tempo_control() -> TempoContext {
    use_context::<TempoContext>().expect("useTempoControl must be used inside TempoControl.")
}

/// `--tempo-height` of a size: `sm` 28px, `default` 32px, `lg` 36px.
pub fn tempo_control_height(size: AudioSize) -> Pixels {
    px(match size {
        AudioSize::Sm => 28.0,
        AudioSize::Default => 32.0,
        AudioSize::Lg => 36.0,
    })
}

// ---------------------------------------------------------------------------
// TempoControl

/// An editable BPM input with tap tempo, keyboard stepping and a rolling
/// average (`<TempoControl>`). Without children it renders a
/// [`TempoControlInput`] and a [`TempoControlTap`]; with children, place
/// those parts yourself.
///
/// Keys in the input: ArrowUp/Down ±1 (Shift: `large_step`, Alt: 1),
/// PageUp/PageDown ±`large_step`, Home/End min/max; typing commits on
/// blur or Enter. Taps average the last four intervals and restart after
/// `tap_timeout` ms idle.
///
/// ```ignore
/// TempoControl::new("tempo")
///     .value(bpm)
///     .on_value_change(|bpm, details, _, cx| set_bpm(bpm, details.reason, cx))
/// ```
#[derive(IntoElement)]
pub struct TempoControl {
    id: ElementId,
    value: Option<f32>,
    default_value: f32,
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    min: f32,
    max: f32,
    large_step: f32,
    tap_timeout: f64,
    size: Option<AudioSize>,
    disabled: Option<bool>,
    read_only: bool,
    name: Option<SharedString>,
    aria_label: SharedString,
    config: AudioConfig,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TempoControl {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: 120.0,
            on_value_change: None,
            on_value_committed: None,
            min: 30.0,
            max: 600.0,
            large_step: 10.0,
            tap_timeout: 2000.0,
            size: None,
            disabled: None,
            read_only: false,
            name: None,
            aria_label: "Tempo".into(),
            config: AudioConfig::default(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Controlled BPM.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Initial BPM when uncontrolled. Default 120.
    pub fn default_value(mut self, value: f32) -> Self {
        self.default_value = value;
        self
    }

    /// Every distinct whole-BPM change (typing, keys, taps).
    pub fn on_value_change(
        mut self,
        f: impl Fn(f32, TempoChangeDetails, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    /// Committed BPM: on blur / Enter after typing, each key step, each
    /// measured tap.
    pub fn on_value_committed(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_committed = Some(Rc::new(f));
        self
    }

    /// Lowest BPM (rounded up). Default 30. Must be positive.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Highest BPM (rounded down). Default 600.
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Shift+arrow / PageUp/PageDown increment. Default 10.
    pub fn large_step(mut self, step: f32) -> Self {
        self.large_step = step;
        self
    }

    /// Idle milliseconds before the next tap starts a new sequence. Default 2000.
    pub fn tap_timeout(mut self, ms: f64) -> Self {
        self.tap_timeout = ms;
        self
    }

    /// `sm` / `default` / `lg`; unset, it follows the [`AudioConfig`].
    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Unset, it follows [`AudioConfig::disabled`].
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Focusable input, no edits, tap disabled. Default false.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Name of the numeric form field (kept for parity; GPUI has no forms).
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Accessible name of the group. Default "Tempo".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }

    /// Settings from a container (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }
}

impl ParentElement for TempoControl {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for TempoControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TempoControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let size = self.size.or(self.config.size).unwrap_or_default();
        let options = TapTempoOptions {
            value: self.value,
            default_value: self.default_value,
            min: self.min,
            max: self.max,
            tap_timeout: self.tap_timeout,
            disabled,
            read_only: self.read_only,
        };
        let large_step = self.large_step;
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            TempoControlState::new(TempoField::new(options, large_step), window, cx)
        });
        let callbacks = Callbacks {
            on_value_change: self.on_value_change.clone(),
            on_value_committed: self.on_value_committed.clone(),
            on_input_change: None,
        };
        state.update(cx, |s, _| {
            s.field.set_options(options, large_step);
            s.callbacks = callbacks;
        });

        let context = TempoContext {
            state,
            disabled,
            read_only: self.read_only,
            height: tempo_control_height(size),
        };
        let children = if self.children.is_empty() {
            vec![
                TempoControlInput::new().into_any_element(),
                TempoControlTap::new().into_any_element(),
            ]
        } else {
            self.children
        };
        let mut root = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(disabled, |el| el.opacity(0.5))
            .role(gpui::Role::Group)
            .aria_label(self.aria_label)
            .children(children);
        root.style().refine(&self.style);
        provide(context, root)
    }
}

// ---------------------------------------------------------------------------
// TempoControlInput

/// The BPM text field with its "BPM" unit (`<TempoControlInput>`). Must be
/// rendered inside a [`TempoControl`]. Styles apply to the text box
/// (`w-11 px-2 text-end font-mono text-sm`).
///
/// ```ignore
/// TempoControl::new("delay").child(TempoControlInput::new().aria_label("Delay BPM"))
/// ```
#[derive(IntoElement)]
pub struct TempoControlInput {
    aria_label: Option<SharedString>,
    on_change: Option<TextHandler>,
    style: StyleRefinement,
}

impl Default for TempoControlInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TempoControlInput {
    pub fn new() -> Self {
        Self {
            aria_label: Some("Tempo in beats per minute".into()),
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// Default "Tempo in beats per minute".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Called with the raw text after every edit (`onChange`), after the
    /// tap history resets.
    pub fn on_change(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl Styled for TempoControlInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TempoControlInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_tempo_control();
        let state = ctx.state.clone();
        let on_change = self.on_change.clone();
        state.update(cx, |s, _| s.callbacks.on_input_change = on_change);
        let focus = state.read(cx).input_focus.clone();
        let focused = !ctx.disabled && focus.is_focused(window);

        let t = cx.theme();
        let radius = fit_radius(radius_lg(t), ctx.height);
        let ring = focused.then(|| t.ring.opacity(0.3));
        let caret_color = t.foreground;
        let selection_color = t.ring.opacity(0.35);
        let mono = t.font_mono.clone();
        let unit_color = t.muted_foreground;

        let paint_state = state.clone();
        let text_box = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let s = paint_state.read(cx);
                let text = s.field.display();
                let selection = s.selection.clone();
                let cursor = s.cursor().min(text.len());
                let style = window.text_style();
                let font_size = style.font_size.to_pixels(window.rem_size());
                let run = TextRun {
                    len: text.len(),
                    font: style.font(),
                    color: style.color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line =
                    window
                        .text_system()
                        .shape_line(text.clone().into(), font_size, &[run], None);
                // `px-2 text-end`.
                let right = bounds.right() - px(8.0);
                let origin_x = right - line.width();
                let line_height = window.line_height();
                let top = bounds.top() + (bounds.size.height - line_height) / 2.0;
                window.handle_input(
                    &s.input_focus.clone(),
                    ElementInputHandler::new(bounds, paint_state.clone()),
                    cx,
                );
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    if focused && !selection.is_empty() {
                        let a = selection.start.min(text.len());
                        let b = selection.end.min(text.len());
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(origin_x + line.x_for_index(a), top),
                                point(origin_x + line.x_for_index(b), top + line_height),
                            ),
                            selection_color,
                        ));
                    }
                    let _ = line.paint(
                        point(origin_x, top),
                        line_height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                    if focused && selection.is_empty() {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(origin_x + line.x_for_index(cursor), top),
                                size(px(1.0), line_height),
                            ),
                            caret_color,
                        ));
                    }
                });
                paint_state.update(cx, |s, _| {
                    s.last_line = Some(line);
                    s.last_origin_x = origin_x;
                    s.last_bounds = Some(bounds);
                });
            },
        )
        .size_full();

        let mut input = div()
            .id("tempo-control-input")
            .h_full()
            .w(px(44.0))
            .font_family(mono)
            .text_sm_()
            .when_some(self.aria_label, |el, l| el.aria_label(l))
            .role(gpui::Role::SpinButton)
            .child(text_box);
        input.style().refine(&self.style);
        let input = if ctx.disabled {
            input
        } else {
            let (s1, s2, s3, s4) = (state.clone(), state.clone(), state.clone(), state.clone());
            let s5 = state.clone();
            input
                .track_focus(&focus)
                .cursor(gpui::CursorStyle::IBeam)
                .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                    s1.update(cx, |s, cx| s.on_mouse_down(e, window, cx));
                })
                .on_mouse_move(move |e, window, cx| {
                    s2.update(cx, |s, cx| s.on_mouse_move(e, window, cx));
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    s3.update(cx, |s, _| s.selecting = false);
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    s5.update(cx, |s, _| s.selecting = false);
                })
                .on_key_down(move |e, window, cx| {
                    if s4.update(cx, |s, cx| s.on_key(e, window, cx)) {
                        cx.stop_propagation();
                    }
                })
        };

        div()
            .relative()
            .flex()
            .items_center()
            .h(ctx.height)
            .rounded(radius)
            .bg(t.input.opacity(0.5))
            .when_some(ring, |el, ring| el.child(ring_overlay(radius, 3.0, ring)))
            .child(input)
            .child(
                div()
                    .pr(px(8.0))
                    .text_xs_()
                    .text_color(unit_color)
                    .child("BPM"),
            )
    }
}

// ---------------------------------------------------------------------------
// TempoControlTap

/// The click passed to [`TempoControlTap::on_click`]; call
/// [`TapClick::prevent_default`] to cancel the tap.
pub struct TapClick<'a> {
    pub event: &'a ClickEvent,
    prevented: Cell<bool>,
}

impl TapClick<'_> {
    /// `event.preventDefault()`: this click does not tap.
    pub fn prevent_default(&self) {
        self.prevented.set(true);
    }

    pub fn default_prevented(&self) -> bool {
        self.prevented.get()
    }
}

type TapClickHandler = Rc<dyn Fn(&TapClick, &mut Window, &mut App) + 'static>;

/// Registers one tap ([`TempoControlTapProps::tap`]).
pub type TapHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// What [`TempoControlTap::render`] receives to build a custom trigger
/// (Base UI's `render` prop): call `tap` on press.
pub struct TempoControlTapProps {
    /// Registers a tap (use as the trigger's click / press handler).
    pub tap: TapHandler,
    /// `disabled || readOnly || prop`.
    pub disabled: bool,
    pub aria_label: SharedString,
    /// The tap's children (default "Tap").
    pub children: Vec<AnyElement>,
}

type TapRender = Box<dyn FnOnce(TempoControlTapProps, &mut Window, &mut App) -> AnyElement>;

/// The tap tempo button (`<TempoControlTap>`). Must be rendered inside a
/// [`TempoControl`]. Disabled when the control is disabled or read-only.
/// Space / Enter tap once per press (held keys don't repeat).
///
/// ```ignore
/// TempoControl::new("tempo").child(TempoControlInput::new()).child(TempoControlTap::new().child("Tap tempo"))
/// ```
#[derive(IntoElement)]
pub struct TempoControlTap {
    disabled: bool,
    aria_label: SharedString,
    on_click: Option<TapClickHandler>,
    render: Option<TapRender>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Default for TempoControlTap {
    fn default() -> Self {
        Self::new()
    }
}

impl TempoControlTap {
    pub fn new() -> Self {
        Self {
            disabled: false,
            aria_label: "Tap tempo".into(),
            on_click: None,
            render: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Default "Tap tempo".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }

    /// Runs before the tap; [`TapClick::prevent_default`] cancels it.
    pub fn on_click(mut self, f: impl Fn(&TapClick, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    /// Renders a custom trigger instead of the button (`render` prop), e.g.
    /// a sound pad.
    pub fn render(
        mut self,
        f: impl FnOnce(TempoControlTapProps, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.render = Some(Box::new(f));
        self
    }
}

impl ParentElement for TempoControlTap {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for TempoControlTap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TempoControlTap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_tempo_control();
        let disabled = ctx.disabled || ctx.read_only || self.disabled;
        let state = ctx.state.clone();
        let tap: TapHandler = Rc::new(move |window, cx| {
            let (events, callbacks) = state.update(cx, |s, cx| {
                let events = s.field.tap(now_ms());
                if !events.is_empty() {
                    s.collapse_to_end();
                    cx.notify();
                }
                (events, s.callbacks.clone())
            });
            emit(&callbacks, events, window, cx);
        });
        let children = if self.children.is_empty() {
            vec!["Tap".into_any_element()]
        } else {
            self.children
        };

        if let Some(render) = self.render {
            let on_click = self.on_click.clone();
            let inner = tap.clone();
            let tap: TapHandler = match on_click {
                Some(on_click) => Rc::new(move |window, cx| {
                    let click = TapClick {
                        event: &ClickEvent::default(),
                        prevented: Cell::new(false),
                    };
                    on_click(&click, window, cx);
                    if !click.default_prevented() {
                        inner(window, cx);
                    }
                }),
                None => inner,
            };
            return render(
                TempoControlTapProps {
                    tap,
                    disabled,
                    aria_label: self.aria_label,
                    children,
                },
                window,
                cx,
            );
        }

        let focus = ctx.state.read(cx).tap_focus.clone();
        let focus_visible =
            !disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let t = cx.theme();
        let radius = fit_radius(radius_lg(t), ctx.height);
        let muted = t.muted;
        let ring = focus_visible.then(|| t.ring.opacity(0.3));

        let mut el = div()
            .id("tempo-control-tap")
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .h(ctx.height)
            .px(px(12.0))
            .rounded(radius)
            .border_1()
            .border_color(t.border)
            .bg(t.background)
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .role(gpui::Role::Button)
            .aria_label(self.aria_label)
            .when(disabled, |el| el.opacity(0.5))
            .when(!disabled, |el| {
                el.hover(move |s| s.bg(muted)).active(move |s| s.bg(muted))
            })
            .when_some(ring, |el, ring| {
                el.child(ring_overlay_bordered(radius, 3.0, 1.0, ring))
            })
            .children(children);
        el.style().refine(&self.style);
        if disabled {
            return el.into_any_element();
        }
        let on_click = self.on_click;
        el.track_focus(&focus)
            .on_click(move |event, window, cx| {
                if let Some(on_click) = &on_click {
                    let click = TapClick {
                        event,
                        prevented: Cell::new(false),
                    };
                    on_click(&click, window, cx);
                    if click.default_prevented() {
                        return;
                    }
                }
                tap(window, cx);
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(options: TapTempoOptions) -> TempoField {
        TempoField::new(options, 10.0)
    }

    fn taps(f: &mut TempoField, times: &[f64]) -> Vec<TempoEvent> {
        times.iter().flat_map(|&t| f.tap(t)).collect()
    }

    fn change(v: f32, reason: TempoChangeReason) -> TempoEvent {
        TempoEvent::Change(v, TempoChangeDetails::new(reason))
    }

    #[test]
    fn tempo_rounds_fractional_defaults_typed_values_and_commits() {
        let mut f = field(TapTempoOptions {
            default_value: 123.6,
            ..Default::default()
        });
        assert_eq!(f.display(), "124");
        assert_eq!(f.edit("96.4"), vec![change(96.0, TempoChangeReason::Input)]);
        assert_eq!(f.blur(), vec![TempoEvent::Commit(96.0)]);
        assert_eq!(f.display(), "96");
        // Alt+ArrowUp: smallStep 1.
        f.step(TempoStep::Up { large: false });
        assert_eq!(f.display(), "97");
    }

    #[test]
    fn tempo_rounds_controlled_bpm_and_fractional_large_steps() {
        let mut f = TempoField::new(
            TapTempoOptions {
                value: Some(90.6),
                ..Default::default()
            },
            2.5,
        );
        assert_eq!(f.display(), "91");
        let events = f.step(TempoStep::Up { large: true });
        assert_eq!(events[0], change(94.0, TempoChangeReason::Keyboard));
        assert_eq!(f.display(), "91");
    }

    #[test]
    fn tempo_waits_for_two_taps() {
        let mut f = field(TapTempoOptions::default());
        assert_eq!(f.display(), "120");
        assert!(f.tap(0.0).is_empty());
        assert_eq!(
            f.tap(600.0),
            vec![
                change(100.0, TempoChangeReason::Tap),
                TempoEvent::Commit(100.0)
            ]
        );
        assert_eq!(f.display(), "100");
    }

    #[test]
    fn tempo_averages_intervals_and_keeps_four() {
        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 400.0, 1000.0]);
        assert_eq!(f.display(), "120");
        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0]);
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_restarts_after_idle_pause_and_custom_timeout() {
        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 600.0, 4000.0]);
        assert_eq!(f.display(), "100");
        f.tap(4500.0);
        assert_eq!(f.display(), "120");

        let mut f = field(TapTempoOptions {
            tap_timeout: 3000.0,
            ..Default::default()
        });
        taps(&mut f, &[0.0, 600.0, 3700.0]);
        assert_eq!(f.display(), "100");
        f.tap(4200.0);
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_accepts_gaps_up_to_two_seconds() {
        let mut f = field(TapTempoOptions {
            min: 1.0,
            ..Default::default()
        });
        taps(&mut f, &[0.0, 1999.0]);
        assert_eq!(f.display(), "30");
        f.tap(4000.0);
        assert_eq!(f.display(), "30");
        f.tap(4500.0);
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_reacquires_slower_tempo_and_smooths() {
        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 250.0, 500.0, 750.0, 1000.0]);
        assert_eq!(f.display(), "240");
        f.tap(3200.0);
        assert_eq!(f.display(), "240");
        taps(&mut f, &[4200.0, 5200.0]);
        assert_eq!(f.display(), "60");

        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 2000.0]);
        assert_eq!(f.display(), "30");

        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 500.0, 1050.0, 1500.0, 2020.0]);
        assert_eq!(f.display(), "119");

        let mut f = field(TapTempoOptions::default());
        taps(&mut f, &[0.0, 500.0, 1000.0, 2000.0, 2500.0]);
        assert_eq!(f.display(), "96");
    }

    #[test]
    fn tempo_ignores_rapid_taps_clamps_and_rounds() {
        let mut f = field(TapTempoOptions::default());
        assert!(taps(&mut f, &[0.0, 0.0, 40.0]).is_empty());
        assert_eq!(f.tap(600.0).last(), Some(&TempoEvent::Commit(100.0)));

        let mut f = field(TapTempoOptions {
            min: 60.0,
            max: 180.0,
            tap_timeout: 3000.0,
            ..Default::default()
        });
        taps(&mut f, &[0.0, 100.0]);
        assert_eq!(f.display(), "180");
        taps(&mut f, &[4000.0, 6000.0]);
        assert_eq!(f.display(), "60");

        let mut f = field(TapTempoOptions::default());
        assert_eq!(
            taps(&mut f, &[0.0, 487.0]).last(),
            Some(&TempoEvent::Commit(123.0))
        );
    }

    #[test]
    fn tempo_reports_taps_without_changing_controlled_value() {
        let mut f = field(TapTempoOptions {
            value: Some(90.0),
            ..Default::default()
        });
        let events = taps(&mut f, &[0.0, 500.0]);
        assert_eq!(events[0], change(120.0, TempoChangeReason::Tap));
        assert_eq!(f.display(), "90");
        f.set_options(
            TapTempoOptions {
                value: Some(120.0),
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_commits_unchanged_measured_tempo() {
        let mut f = field(TapTempoOptions::default());
        assert_eq!(taps(&mut f, &[0.0, 500.0]), vec![TempoEvent::Commit(120.0)]);
    }

    #[test]
    fn tempo_steps_clamps_and_commits_with_keys() {
        let mut f = field(TapTempoOptions::default());
        f.step(TempoStep::Up { large: false });
        assert_eq!(f.display(), "121");
        f.step(TempoStep::Down { large: true });
        assert_eq!(f.display(), "111");
        f.step(TempoStep::Home);
        assert_eq!(f.display(), "30");
        let events = f.step(TempoStep::End);
        assert_eq!(f.display(), "600");
        assert_eq!(events.last(), Some(&TempoEvent::Commit(600.0)));
        f.step(TempoStep::PageDown);
        assert_eq!(f.display(), "590");
    }

    #[test]
    fn tempo_typed_bpm_resets_tap_history() {
        let mut f = field(TapTempoOptions::default());
        f.tap(0.0);
        f.edit("");
        f.edit("9");
        f.edit("95");
        f.blur();
        assert_eq!(f.display(), "95");
        f.tap(500.0);
        assert_eq!(f.display(), "95");
        f.tap(1000.0);
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_partial_input_resets_taps_without_reporting() {
        let mut f = field(TapTempoOptions::default());
        f.tap(0.0);
        assert!(f.edit(".").is_empty());
        assert_eq!(f.display(), ".");
        assert!(f.blur().is_empty());
        assert!(f.tap(600.0).is_empty());
        assert_eq!(
            f.tap(1200.0),
            vec![
                change(100.0, TempoChangeReason::Tap),
                TempoEvent::Commit(100.0)
            ]
        );
        assert_eq!(f.display(), "100");
    }

    #[test]
    fn tempo_restores_last_value_after_empty_blur() {
        let mut f = field(TapTempoOptions {
            default_value: 96.0,
            ..Default::default()
        });
        assert!(f.edit("").is_empty());
        assert_eq!(f.display(), "");
        assert!(f.blur().is_empty());
        assert_eq!(f.display(), "96");
    }

    #[test]
    fn tempo_disabled_resets_taps_and_read_only_ignores_keys() {
        let mut f = field(TapTempoOptions::default());
        f.tap(0.0);
        f.set_options(
            TapTempoOptions {
                disabled: true,
                ..Default::default()
            },
            10.0,
        );
        f.set_options(TapTempoOptions::default(), 10.0);
        f.tap(600.0);
        assert_eq!(f.display(), "120");
        f.tap(1200.0);
        assert_eq!(f.display(), "100");

        let mut f = field(TapTempoOptions {
            read_only: true,
            ..Default::default()
        });
        assert!(f.step(TempoStep::Up { large: false }).is_empty());
        assert!(f.edit("90").is_empty());
        assert_eq!(f.display(), "120");
    }

    #[test]
    fn tempo_control_defaults() {
        let c = TempoControl::new("t");
        assert_eq!(c.default_value, 120.0);
        assert_eq!(
            (c.min, c.max, c.large_step, c.tap_timeout),
            (30.0, 600.0, 10.0, 2000.0)
        );
        assert_eq!(c.aria_label.as_ref(), "Tempo");
        assert_eq!(tempo_control_height(AudioSize::Default), px(32.0));
        assert_eq!(tempo_control_height(AudioSize::Sm), px(28.0));
        assert_eq!(tempo_control_height(AudioSize::Lg), px(36.0));
        assert_eq!(TempoControlTap::new().aria_label.as_ref(), "Tap tempo");
        assert_eq!(
            TempoControlInput::new().aria_label.as_deref(),
            Some("Tempo in beats per minute")
        );
    }
}
