//! Port of `components/ui/fader.tsx`.
//!
//! A volume fader in decibels, with tapers, detents, a scale, reset and an
//! editable value. Compound like the React version: [`Fader`] provides its
//! state to the parts inside it ([`FaderLabel`], [`FaderTrack`],
//! [`FaderRange`], [`FaderThumb`], [`FaderScale`], [`FaderValue`],
//! [`FaderReset`]); without children it renders a track, range and thumb.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, BoxShadow, ClipboardItem, DispatchPhase, Div, ElementId, Entity,
    FocusHandle, FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, Keystroke,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    Refineable, RenderOnce, ScrollWheelEvent, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Subscription, Window, canvas, div, font, px, relative,
};

use crate::audio::decibels::{FormatDbOptions, SILENCE_DB, clamp, format_db};
use crate::audio::taper::{AnyTaper, Taper, TaperInput, resolve_taper};
use crate::audio::types::Orientation;
use crate::components::{
    DbScale, DbScaleFormat, DbScaleSide, DbScaleTick, use_channel_strip_fader_slot,
};
use crate::config::{AudioConfig, AudioSize};
use crate::motion::{Duration, Easing, Instant, Transition};
use crate::theme::{ActiveTheme, Theme, mix};
use crate::ui::styles::{TextSize, css_shadow, provide, radius_md, radius_sm, use_context};

const DEFAULT_MIN_DB: f32 = -60.0;
const DEFAULT_MAX_DB: f32 = 6.0;
const POSITION_STEP: f32 = 0.0005;
const DETENT_SNAP: f32 = 0.012;
const PRECISION: f64 = 1e6;
/// Levels whose text is as wide as any a fader shows: two digits either side of 0.
const WIDTH_SAMPLES_DB: [f32; 4] = [-88.8, -8.8, 8.8, 88.8];
/// `before:-inset-y-1.5`: the control's pointer area reaches 6px past the thumb.
const HIT_SLOP: f32 = 6.0;
/// Tailwind's default `transition` duration.
const RING_TRANSITION: Duration = Duration::from_millis(150);

// ---------------------------------------------------------------------------
// Public types

/// Why a fader's value changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FaderChangeReason {
    Drag,
    TrackPress,
    Keyboard,
    Wheel,
    Reset,
    Input,
}

/// Passed to `on_value_change_with_details`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaderChangeDetails {
    pub reason: FaderChangeReason,
}

/// `variant`: `Console` has a wide cap thumb.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FaderVariant {
    #[default]
    Default,
    Console,
}

type ChangeHandler = Rc<dyn Fn(f32, FaderChangeDetails, &mut Window, &mut App)>;
type CommitHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type FormatFn = Rc<dyn Fn(f32) -> String>;
type DraftHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Pure logic (unit-tested)

/// `Math.round(value * 1e6) / 1e6`.
pub(crate) fn round_value(value: f64) -> f64 {
    js_round(value * PRECISION) / PRECISION
}

/// The `f64` a JavaScript number literal for this `f32` would hold
/// (0.1f32 → 0.1, not 0.10000000149), so step maths matches React's.
pub(crate) fn decimal(value: f32) -> f64 {
    if !value.is_finite() {
        return value as f64;
    }
    format!("{value}").parse().unwrap_or(value as f64)
}

/// JavaScript `Math.round`: halves round toward +∞.
pub(crate) fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// The fader's default `format`: "Silent" at −∞, otherwise `formatDb`.
pub fn default_fader_format(db: f32) -> String {
    if db == SILENCE_DB {
        "Silent".into()
    } else {
        format_db(db, FormatDbOptions::default())
    }
}

/// Parses typed dB text: "−6", "-6 dB", "3.5db", "-inf", "∞". `None` if empty
/// or not a number.
pub(crate) fn parse_db(text: &str) -> Option<f32> {
    let replaced = text.replace('\u{2212}', "-");
    let lower = replaced.to_lowercase();
    // `replace(/db/iu, "")`: the first "db" only.
    let normalized = match lower.find("db") {
        Some(at) => format!("{}{}", &replaced[..at], &replaced[at + 2..]),
        None => replaced,
    };
    let normalized = normalized.trim();
    let unsigned = normalized.strip_prefix('-').unwrap_or(normalized);
    let lower = unsigned.to_lowercase();
    if lower == "inf" || lower == "infinity" || lower == "\u{221e}" {
        return Some(SILENCE_DB);
    }
    if normalized.is_empty() {
        return None;
    }
    js_number(normalized).map(|n| n as f32)
}

/// JavaScript `Number(text)` for decimal text (NaN → `None`).
pub(crate) fn js_number(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0.0);
    }
    let lower = text.to_ascii_lowercase();
    let digits = lower.trim_start_matches(['+', '-']);
    if digits.starts_with("inf") || digits.starts_with("nan") {
        return None;
    }
    text.parse::<f64>().ok()
}

/// JavaScript `String(number)` for the values a fader holds.
pub(crate) fn js_string(value: f32) -> String {
    if value == 0.0 {
        return "0".into();
    }
    format!("{value}")
}

/// Characters in the widest value this fader can show.
fn widest_value(format: &dyn Fn(f32) -> String, min: f32, max: f32) -> usize {
    let mut samples = vec![SILENCE_DB, min, min + 0.1, max, max - 0.1];
    samples.extend(WIDTH_SAMPLES_DB.iter().filter(|&&db| db > min && db < max));
    samples
        .into_iter()
        .map(|db| format(db).encode_utf16().count())
        .max()
        .unwrap_or(0)
}

fn increment_for(alt: bool, shift: bool, step: f32, fine_step: f32, large_step: f32) -> f32 {
    if alt {
        fine_step
    } else if shift {
        large_step
    } else {
        step
    }
}

/// Maps positions to dB and steps the value (`useFaderStepping`).
#[derive(Clone, Debug)]
pub(crate) struct FaderLaw {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub large_step: f32,
    pub fine_step: f32,
    pub silence_at_min: bool,
    pub detents: Vec<f32>,
    pub taper: AnyTaper,
}

impl FaderLaw {
    pub fn to_position(&self, db: f32) -> f32 {
        if db == SILENCE_DB {
            0.0
        } else {
            self.taper.to_position(db)
        }
    }

    pub fn quantize(&self, db: f32, increment: f32) -> f32 {
        if db == SILENCE_DB {
            return if self.silence_at_min {
                SILENCE_DB
            } else {
                self.min
            };
        }
        let (min, increment) = (decimal(self.min), decimal(increment));
        let stepped = min + js_round((decimal(db) - min) / increment) * increment;
        round_value(clamp(stepped as f32, self.min, self.max) as f64) as f32
    }

    pub fn value_at(&self, position: f32, fine: bool) -> f32 {
        if self.silence_at_min && position <= 0.0 {
            return SILENCE_DB;
        }
        for &detent in &self.detents {
            if (position - self.to_position(detent)).abs() < DETENT_SNAP {
                return detent;
            }
        }
        let increment = if fine { self.fine_step } else { self.step };
        self.quantize(self.taper.to_value(position), increment)
    }

    pub fn nudge(&self, current: f32, direction: f32, increment: f32) -> f32 {
        if current == SILENCE_DB {
            return if direction > 0.0 {
                self.min
            } else {
                SILENCE_DB
            };
        }
        if self.silence_at_min && direction < 0.0 && current <= self.min {
            return SILENCE_DB;
        }
        self.quantize(
            (decimal(current) + direction as f64 * decimal(increment)) as f32,
            increment.min(self.step),
        )
    }

    /// The value a key press moves to, or `None` if the key isn't handled.
    /// `key` uses GPUI names ("up", "pageup", "home", ...).
    pub fn key_target(&self, key: &str, shift: bool, alt: bool, current: f32) -> Option<f32> {
        let increment = increment_for(alt, shift, self.step, self.fine_step, self.large_step);
        Some(match key {
            "down" | "left" => self.nudge(current, -1.0, increment),
            "up" | "right" => self.nudge(current, 1.0, increment),
            "end" => self.max,
            "home" => {
                if self.silence_at_min {
                    SILENCE_DB
                } else {
                    self.min
                }
            }
            "pagedown" => self.nudge(current, -1.0, self.large_step),
            "pageup" => self.nudge(current, 1.0, self.large_step),
            _ => return None,
        })
    }
}

/// Rounds a 0..1 slider position to the slider's `step` (Base UI's
/// `roundValueToStep`).
pub(crate) fn snap_position(position: f32, step: f32) -> f32 {
    let p = position.clamp(0.0, 1.0) as f64;
    let step = step as f64;
    (round_value(js_round(p / step) * step) as f32).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// A tiny single-line text editor (the editable value / number inputs)

/// Text being typed into a value field, with a caret and selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TextDraft {
    pub text: String,
    /// Caret, a byte index.
    pub cursor: usize,
    /// The other end of the selection (== cursor when nothing is selected).
    pub anchor: usize,
}

impl TextDraft {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            cursor: end,
            anchor: 0,
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn prev(&self, at: usize) -> usize {
        self.text[..at]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self, at: usize) -> usize {
        self.text[at..]
            .chars()
            .next()
            .map_or(at, |c| at + c.len_utf8())
    }

    fn insert(&mut self, s: &str) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, s);
        self.cursor = start + s.len();
        self.anchor = self.cursor;
    }

    fn move_to(&mut self, at: usize, extend: bool) {
        self.cursor = at;
        if !extend {
            self.anchor = at;
        }
    }

    /// Applies an editing key. Returns true if the key was used.
    pub fn handle_key(&mut self, keystroke: &Keystroke, cx: &mut App) -> bool {
        let m = &keystroke.modifiers;
        let shift = m.shift;
        let command = m.control || m.platform;
        let (start, end) = self.selection();
        match keystroke.key.as_str() {
            "backspace" => {
                if start == end && start > 0 {
                    self.anchor = self.prev(start);
                }
                self.insert("");
            }
            "delete" => {
                if start == end {
                    self.anchor = self.next(end);
                }
                self.insert("");
            }
            "left" => {
                let at = if start != end && !shift {
                    start
                } else {
                    self.prev(self.cursor)
                };
                self.move_to(at, shift);
            }
            "right" => {
                let at = if start != end && !shift {
                    end
                } else {
                    self.next(self.cursor)
                };
                self.move_to(at, shift);
            }
            "home" => self.move_to(0, shift),
            "end" => self.move_to(self.text.len(), shift),
            "a" if command => self.select_all(),
            "c" | "x" if command => {
                let selected = self.text[start..end].to_string();
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
                if keystroke.key == "x" {
                    self.insert("");
                }
            }
            "v" if command => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.insert(text.lines().next().unwrap_or(""));
                }
            }
            _ => {
                if command || m.function {
                    return false;
                }
                let typed = keystroke.key_char.clone().or_else(|| {
                    (keystroke.key.chars().count() == 1).then(|| keystroke.key.clone())
                });
                match typed {
                    Some(text) if !text.chars().any(char::is_control) => self.insert(&text),
                    _ => return false,
                }
            }
        }
        true
    }

    /// The text with its selection highlight and caret, right-aligned, in the
    /// inherited text style.
    pub fn render(&self, focused: bool, color: Hsla) -> Div {
        let (start, end) = self.selection();
        let before = self.text[..start].to_string();
        let selected = self.text[start..end].to_string();
        let after = self.text[end..].to_string();
        let caret_after_selection = self.cursor == end;
        let caret = || div().flex_none().w(px(1.0)).h(px(14.0)).bg(color);
        let highlight = gpui::hsla(212.0 / 360.0, 1.0, 0.6, 0.35);
        div()
            .flex()
            .items_center()
            .justify_end()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(div().flex_none().child(before))
            .when(focused && start == end, |el| el.child(caret()))
            .when(focused && start != end && !caret_after_selection, |el| {
                el.child(caret())
            })
            .when(!selected.is_empty(), |el| {
                el.child(
                    div()
                        .flex_none()
                        .when(focused, |el| el.bg(highlight))
                        .child(selected),
                )
            })
            .when(focused && start != end && caret_after_selection, |el| {
                el.child(caret())
            })
            .child(div().flex_none().child(after))
    }
}

/// Width of "0" (the CSS `ch` unit) in a font at a size.
pub(crate) fn ch_width(family: &SharedString, size: Pixels, window: &Window) -> Pixels {
    let text_system = window.text_system();
    let id = text_system.resolve_font(&font(family.clone()));
    text_system.ch_advance(id, size).unwrap_or(size * 0.6)
}

// ---------------------------------------------------------------------------
// Shared thumb look

/// The ring of a slider thumb: `ring-1 ring-foreground/15`, growing to
/// `ring-4 ring-ring/30` on hover / drag (`ring-ring/40` when focus-visible),
/// with `shadow-sm` underneath. `t` is the 0..1 transition progress.
pub(crate) fn thumb_shadows(theme: &Theme, t: f32, focus_visible: bool) -> Vec<BoxShadow> {
    let rest = theme.foreground.opacity(0.15);
    let active = theme.ring.opacity(if focus_visible { 0.4 } else { 0.3 });
    let width = 1.0 + 3.0 * t;
    let black = |a: f32| gpui::black().opacity(a);
    vec![
        css_shadow(0.0, 0.0, 0.0, width, mix(rest, active, t)),
        css_shadow(0.0, 1.0, 3.0, 0.0, black(0.1)),
        css_shadow(0.0, 1.0, 2.0, -1.0, black(0.1)),
    ]
}

/// `data-disabled:opacity-50` composites the whole fader as one layer, so
/// the opaque thumb hides the range and the range hides the track. GPUI
/// fades each shape on its own instead, which would let them show through
/// each other. The range and thumb therefore undo the root's opacity
/// ([`DISABLED_UNFADE`]) and take these colours, flattened onto the page
/// background as the layer would be.
pub(crate) fn disabled_flatten(color: Hsla, background: Hsla) -> Hsla {
    let (c, b) = (gpui::Rgba::from(color), gpui::Rgba::from(background));
    let a = color.a * 0.5;
    let lerp = |x: f32, y: f32| y + (x - y) * a;
    Hsla::from(gpui::Rgba {
        r: lerp(c.r, b.r),
        g: lerp(c.g, b.g),
        b: lerp(c.b, b.b),
        a: 1.0,
    })
}

/// Undoes the root's `opacity(0.5)` for a part drawn with
/// [`disabled_flatten`] colours (GPUI multiplies nested opacities unclamped).
pub(crate) const DISABLED_UNFADE: f32 = 2.0;

/// Animated ring progress for a slider thumb.
#[derive(Clone, Debug)]
pub(crate) struct ThumbRing {
    transition: Transition,
}

impl Default for ThumbRing {
    fn default() -> Self {
        Self {
            transition: Transition::new(0.0),
        }
    }
}

impl ThumbRing {
    /// Moves toward `active` and returns (progress, still animating).
    pub fn update(&mut self, active: bool) -> (f32, bool) {
        let now = Instant::now();
        self.transition.animate_to(
            if active { 1.0 } else { 0.0 },
            RING_TRANSITION,
            Easing::STANDARD,
        );
        (
            self.transition.value(now).clamp(0.0, 1.0),
            self.transition.is_animating(now),
        )
    }
}

// ---------------------------------------------------------------------------
// State and context

struct FaderState {
    uncontrolled: f32,
    /// `latestRef`: the last value reported or rendered.
    latest: f32,
    thumb_focus: FocusHandle,
    value_focus: FocusHandle,
    reset_focus: FocusHandle,
    editor_focus: FocusHandle,
    dragging: bool,
    /// Pointer offset from the thumb centre along the travel when the drag
    /// started on the thumb (Base UI keeps the thumb under the finger).
    drag_offset: f32,
    thumb_hovered: bool,
    ring: ThumbRing,
    /// The inner track (the travel the thumb centre covers).
    track: Rc<Cell<Option<Bounds<Pixels>>>>,
    editing: bool,
    draft: TextDraft,
    /// Applies the draft on blur; refreshed every render.
    apply_draft: Option<DraftHandler>,
    _editor_blur: Option<Subscription>,
}

#[derive(Clone)]
struct FaderCtx {
    state: Entity<FaderState>,
    controlled: bool,
    aria_label: Option<SharedString>,
    value: f32,
    position: f32,
    origin_position: f32,
    min: f32,
    max: f32,
    reset_value: f32,
    orientation: Orientation,
    variant: FaderVariant,
    disabled: bool,
    law: Rc<FaderLaw>,
    taper: AnyTaper,
    format: FormatFn,
    thumb_size: f32,
    track_size: f32,
    /// Inside a `ChannelStripFader`: the control and scale lose their
    /// half-thumb padding.
    in_strip: bool,
    /// The root has one child (`[data-slot=fader-control]:only-child`).
    single_child: bool,
    on_change: Option<ChangeHandler>,
    on_commit: Option<CommitHandler>,
}

impl FaderCtx {
    fn horizontal(&self) -> bool {
        self.orientation == Orientation::Horizontal
    }

    fn latest(&self, cx: &App) -> f32 {
        self.state.read(cx).latest
    }

    /// `change`: skips repeats, updates uncontrolled state, reports.
    fn change(&self, db: f32, reason: FaderChangeReason, window: &mut Window, cx: &mut App) {
        let controlled = self.controlled;
        let changed = self.state.update(cx, |s, cx| {
            if db == s.latest {
                return false;
            }
            s.latest = db;
            if !controlled {
                s.uncontrolled = db;
            }
            cx.notify();
            true
        });
        if changed && let Some(handler) = &self.on_change {
            handler(db, FaderChangeDetails { reason }, window, cx);
        }
    }

    fn commit(&self, db: f32, window: &mut Window, cx: &mut App) {
        if let Some(handler) = &self.on_commit {
            handler(db, window, cx);
        }
    }

    fn reset(&self, window: &mut Window, cx: &mut App) {
        self.change(self.reset_value, FaderChangeReason::Reset, window, cx);
        self.commit(self.reset_value, window, cx);
    }

    fn handle_key(&self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
        let m = &event.keystroke.modifiers;
        let current = self.latest(cx);
        let Some(next) = self
            .law
            .key_target(&event.keystroke.key, m.shift, m.alt, current)
        else {
            return false;
        };
        self.change(next, FaderChangeReason::Keyboard, window, cx);
        self.commit(next, window, cx);
        true
    }

    /// The pointer's position along the travel (0 bottom/left .. 1).
    fn pointer_position(&self, pointer: Point<Pixels>, offset: f32, cx: &App) -> Option<f32> {
        let track = self.state.read(cx).track.get()?;
        let position = if self.horizontal() {
            let width = track.size.width.as_f32();
            if width <= 0.0 {
                return None;
            }
            (pointer.x.as_f32() - offset - track.origin.x.as_f32()) / width
        } else {
            let height = track.size.height.as_f32();
            if height <= 0.0 {
                return None;
            }
            (track.bottom().as_f32() - (pointer.y.as_f32() - offset)) / height
        };
        Some(snap_position(position, POSITION_STEP))
    }

    fn drag_to(
        &self,
        pointer: Point<Pixels>,
        alt: bool,
        reason: FaderChangeReason,
        window: &mut Window,
        cx: &mut App,
    ) {
        let offset = self.state.read(cx).drag_offset;
        if let Some(position) = self.pointer_position(pointer, offset, cx) {
            let db = self.law.value_at(position, alt);
            self.change(db, reason, window, cx);
        }
    }

    /// The thumb centre along the travel, in window coordinates.
    fn thumb_center(&self, cx: &App) -> Option<f32> {
        let track = self.state.read(cx).track.get()?;
        Some(if self.horizontal() {
            track.origin.x.as_f32() + self.position * track.size.width.as_f32()
        } else {
            track.bottom().as_f32() - self.position * track.size.height.as_f32()
        })
    }

    fn axis(&self, pointer: Point<Pixels>) -> f32 {
        if self.horizontal() {
            pointer.x.as_f32()
        } else {
            pointer.y.as_f32()
        }
    }

    fn start_drag(&self, offset: f32, window: &mut Window, cx: &mut App) {
        let focus = self.state.read(cx).thumb_focus.clone();
        window.focus(&focus, cx);
        // Like a focusable element: ancestors must not take focus back.
        window.prevent_default();
        self.state.update(cx, |s, cx| {
            s.dragging = true;
            s.drag_offset = offset;
            cx.notify();
        });
    }

    fn end_drag(&self, window: &mut Window, cx: &mut App) {
        let was = self.state.update(cx, |s, cx| {
            let was = s.dragging;
            s.dragging = false;
            cx.notify();
            was
        });
        if was {
            let latest = self.latest(cx);
            self.commit(latest, window, cx);
        }
    }

    fn focus_within(&self, window: &Window, cx: &App) -> bool {
        let s = self.state.read(cx);
        s.thumb_focus.is_focused(window)
            || s.editor_focus.is_focused(window)
            || s.value_focus.is_focused(window)
            || s.reset_focus.is_focused(window)
    }

    /// `finish(apply)` of `FaderValue`.
    fn apply_draft(&self, draft: &str, window: &mut Window, cx: &mut App) {
        let Some(parsed) = parse_db(draft) else {
            return;
        };
        let next = if parsed == SILENCE_DB {
            self.min
        } else {
            clamp(parsed, self.min, self.max)
        };
        self.change(next, FaderChangeReason::Input, window, cx);
        self.commit(next, window, cx);
    }
}

fn fader_ctx(part: &str) -> FaderCtx {
    use_context::<FaderCtx>().unwrap_or_else(|| panic!("{part} must be used inside Fader."))
}

fn sizes(size: AudioSize) -> (f32, f32) {
    match size {
        AudioSize::Sm => (12.0, 3.0),
        AudioSize::Default => (16.0, 4.0),
        AudioSize::Lg => (20.0, 6.0),
    }
}

// ---------------------------------------------------------------------------
// Fader

/// A volume fader in decibels, with tapers, detents, a scale, reset and an
/// editable value. Drag, click the track, use the arrow keys (Shift for
/// `large_step`, Alt for `fine_step`), Page Up/Down, Home/End, the wheel
/// (`allow_wheel`), or double-click the thumb to reset.
///
/// ```ignore
/// Fader::new("mic")
///     .value(gain_db)
///     .on_value_change(|db, _, cx| set_gain(db, cx))
///     .child(div().flex().items_center().gap_2()
///         .child(FaderLabel::new("Microphone").mr_auto())
///         .child(FaderReset::new())
///         .child(FaderValue::new()))
///     .child(FaderTrack::new().child(FaderRange::new()).child(FaderThumb::new()))
///     .child(FaderScale::new())
/// ```
#[derive(IntoElement)]
pub struct Fader {
    id: ElementId,
    aria_label: Option<SharedString>,
    value: Option<f32>,
    default_value: Option<f32>,
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    min: f32,
    max: f32,
    step: f32,
    large_step: f32,
    fine_step: f32,
    reset_value: f32,
    taper: TaperInput,
    origin: Option<f32>,
    detents: Vec<f32>,
    silence_at_min: bool,
    allow_wheel: bool,
    orientation: Option<Orientation>,
    variant: FaderVariant,
    size: Option<AudioSize>,
    format: FormatFn,
    disabled: Option<bool>,
    config: AudioConfig,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl Fader {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            aria_label: None,
            value: None,
            default_value: None,
            on_value_change: None,
            on_value_committed: None,
            min: DEFAULT_MIN_DB,
            max: DEFAULT_MAX_DB,
            step: 0.5,
            large_step: 6.0,
            fine_step: 0.1,
            reset_value: 0.0,
            taper: TaperInput::Linear,
            origin: None,
            detents: vec![0.0],
            silence_at_min: false,
            allow_wheel: false,
            orientation: None,
            variant: FaderVariant::Default,
            size: None,
            format: Rc::new(default_fader_format),
            disabled: None,
            config: AudioConfig::default(),
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// `aria-label` of the thumb.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The value in dB (controlled). `f32::NEG_INFINITY` is silence.
    pub fn value(mut self, db: f32) -> Self {
        self.value = Some(db);
        self
    }

    /// Initial value when uncontrolled. Default: `reset_value` clamped to the range.
    pub fn default_value(mut self, db: f32) -> Self {
        self.default_value = Some(db);
        self
    }

    pub fn on_value_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_change = Some(Rc::new(move |db, _, window, cx| f(db, window, cx)));
        self
    }

    /// `onValueChange(value, details)`.
    pub fn on_value_change_with_details(
        mut self,
        f: impl Fn(f32, FaderChangeDetails, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    /// Fires when a drag ends, after keyboard input, and on reset.
    pub fn on_value_committed(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_committed = Some(Rc::new(f));
        self
    }

    /// Bottom of the range in dB. Default −60.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Top of the range in dB. Default +6.
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Arrow keys and drag resolution in dB. Default 0.5.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Shift+arrow and Page Up/Down, in dB. Default 6.
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Alt+arrow and Alt+drag resolution, in dB. Default 0.1.
    pub fn fine_step(mut self, fine_step: f32) -> Self {
        self.fine_step = fine_step;
        self
    }

    /// Value restored by double-clicking the thumb. Default 0.
    pub fn reset_value(mut self, db: f32) -> Self {
        self.reset_value = db;
        self
    }

    /// Position law. Default `Linear`.
    pub fn taper(mut self, taper: impl Into<TaperInput>) -> Self {
        self.taper = taper.into();
        self
    }

    /// Where the range fill starts. Set 0 for a bipolar gain. Default `min`.
    pub fn origin(mut self, db: f32) -> Self {
        self.origin = Some(db);
        self
    }

    /// Values the thumb snaps to while dragging. Default `[0]`.
    pub fn detents(mut self, detents: impl Into<Vec<f32>>) -> Self {
        self.detents = detents.into();
        self
    }

    /// The bottom position reports `-Infinity`. Default false.
    pub fn silence_at_min(mut self, silence_at_min: bool) -> Self {
        self.silence_at_min = silence_at_min;
        self
    }

    /// The mouse wheel adjusts the value while the fader is focused. Default false.
    pub fn allow_wheel(mut self, allow_wheel: bool) -> Self {
        self.allow_wheel = allow_wheel;
        self
    }

    /// Default: the config's, else horizontal.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// `Console` has a wide cap thumb. Default `Default`.
    pub fn variant(mut self, variant: FaderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Default: the config's, else `Default`.
    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Formats the value for `FaderValue` and assistive technology.
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Settings from a surrounding strip or mixer (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }
}

impl ParentElement for Fader {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Fader {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Fader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let orientation = self
            .orientation
            .or(self.config.orientation)
            .unwrap_or_default();
        let size = self.size.or(self.config.size).unwrap_or_default();
        let (min, max) = (self.min, self.max);
        let initial = self
            .default_value
            .unwrap_or_else(|| clamp(self.reset_value, min, max));

        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let editor_focus = cx.focus_handle();
            let blur = cx.on_blur(&editor_focus, window, |s: &mut FaderState, window, cx| {
                if !s.editing {
                    return;
                }
                s.editing = false;
                let draft = std::mem::take(&mut s.draft.text);
                if let Some(apply) = s.apply_draft.clone() {
                    window.defer(cx, move |window, cx| apply(draft, window, cx));
                }
                cx.notify();
            });
            FaderState {
                uncontrolled: initial,
                latest: initial,
                thumb_focus: cx.focus_handle().tab_stop(true),
                value_focus: cx.focus_handle().tab_stop(true),
                reset_focus: cx.focus_handle().tab_stop(true),
                editor_focus,
                dragging: false,
                drag_offset: 0.0,
                thumb_hovered: false,
                ring: ThumbRing::default(),
                track: Rc::new(Cell::new(None)),
                editing: false,
                draft: TextDraft::default(),
                apply_draft: None,
                _editor_blur: Some(blur),
            }
        });

        let controlled = self.value.is_some();
        let value = state.update(cx, |s, _| {
            let value = self.value.unwrap_or(s.uncontrolled);
            s.latest = value;
            if disabled {
                s.dragging = false;
            }
            value
        });

        let taper = resolve_taper(&self.taper, min, max);
        let law = Rc::new(FaderLaw {
            min,
            max,
            step: self.step,
            large_step: self.large_step,
            fine_step: self.fine_step,
            silence_at_min: self.silence_at_min,
            detents: self.detents.clone(),
            taper: taper.clone(),
        });
        let position = law.to_position(value);
        let origin_position = law.to_position(clamp(self.origin.unwrap_or(min), min, max));
        let (thumb_size, track_size) = sizes(size);

        let ctx = FaderCtx {
            state: state.clone(),
            controlled,
            aria_label: self.aria_label.clone(),
            value,
            position,
            origin_position,
            min,
            max,
            reset_value: self.reset_value,
            orientation,
            variant: self.variant,
            disabled,
            law,
            taper,
            format: self.format.clone(),
            thumb_size,
            track_size,
            in_strip: use_channel_strip_fader_slot().is_some(),
            single_child: self.children.len() <= 1,
            on_change: self.on_value_change.clone(),
            on_commit: self.on_value_committed.clone(),
        };

        // Blur applies the typed value with the latest props.
        let apply_ctx = ctx.clone();
        state.update(cx, |s, _| {
            s.apply_draft = Some(Rc::new(move |draft: String, window, cx| {
                apply_ctx.apply_draft(&draft, window, cx);
            }));
        });

        let horizontal = orientation == Orientation::Horizontal;
        let children = if self.children.is_empty() {
            vec![
                FaderTrack::new()
                    .child(FaderRange::new())
                    .child(FaderThumb::new())
                    .into_any_element(),
            ]
        } else {
            self.children
        };

        let wheel_ctx = ctx.clone();
        let allow_wheel = self.allow_wheel;
        let mut root = div()
            .id(self.id)
            .relative()
            .flex()
            .gap_2()
            .map(|el| {
                if horizontal {
                    el.w_full().flex_col()
                } else {
                    el.h_full().min_h(px(128.0)).flex_row().justify_center()
                }
            })
            .when(disabled, |el| el.opacity(0.5))
            .when(allow_wheel, |el| {
                el.on_scroll_wheel(move |e: &ScrollWheelEvent, window, cx| {
                    let ctx = &wheel_ctx;
                    let delta = e.delta.pixel_delta(px(16.0)).y.as_f32();
                    if ctx.disabled || !ctx.focus_within(window, cx) || delta == 0.0 {
                        return;
                    }
                    cx.stop_propagation();
                    let increment = if e.modifiers.alt {
                        ctx.law.fine_step
                    } else {
                        ctx.law.step
                    };
                    // GPUI's wheel delta is the DOM's negated: up is positive.
                    let direction = if delta > 0.0 { 1.0 } else { -1.0 };
                    let next = ctx.law.nudge(ctx.latest(cx), direction, increment);
                    ctx.change(next, FaderChangeReason::Wheel, window, cx);
                    ctx.commit(next, window, cx);
                })
            })
            .children(children);
        root.style().refine(&self.style);
        provide(ctx, root)
    }
}

// ---------------------------------------------------------------------------
// Parts

/// The fader's label: `text-sm font-medium`. Clicking it focuses the thumb.
///
/// ```ignore
/// FaderLabel::new("Microphone").mr_auto()
/// ```
#[derive(IntoElement)]
pub struct FaderLabel {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl FaderLabel {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            style: StyleRefinement::default(),
            children: vec![label.into().into_any_element()],
        }
    }

    /// A label with no text; add children.
    pub fn empty() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for FaderLabel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for FaderLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderLabel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderLabel");
        let focus = ctx.state.read(cx).thumb_focus.clone();
        let disabled = ctx.disabled;
        let mut el = div()
            .id("fader-label")
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().foreground)
            .when(!disabled, |el| {
                el.on_click(move |_, window, cx| window.focus(&focus, cx))
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The control and its track. Styles (`Styled`) apply to the track, like the
/// React `className`; children (range, thumb, a meter) sit inside the track.
#[derive(IntoElement)]
pub struct FaderTrack {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl FaderTrack {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for FaderTrack {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for FaderTrack {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for FaderTrack {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderTrack {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderTrack");
        let theme = cx.theme();
        let horizontal = ctx.horizontal();
        let thumb = px(ctx.thumb_size);
        let half = px(ctx.thumb_size / 2.0);
        let (track_cell, dragging) = {
            let s = ctx.state.read(cx);
            (s.track.clone(), s.dragging)
        };

        // Records the track's bounds, and follows the pointer anywhere in the
        // window while dragging (pointer capture).
        let drag_ctx = ctx.clone();
        let tracker = canvas(
            move |bounds, _, _| track_cell.set(Some(bounds)),
            move |_, _, window, _| {
                if !dragging {
                    return;
                }
                let moved = drag_ctx.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    if e.pressed_button != Some(MouseButton::Left) {
                        moved.end_drag(window, cx);
                        return;
                    }
                    moved.drag_to(
                        e.position,
                        e.modifiers.alt,
                        FaderChangeReason::Drag,
                        window,
                        cx,
                    );
                });
                let released = drag_ctx.clone();
                window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                        released.end_drag(window, cx);
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        // `before:` pointer area: the control plus 6px across the travel.
        let press_ctx = ctx.clone();
        let slop = px(HIT_SLOP);
        let hit = div()
            .id("fader-hit")
            .absolute()
            .map(|el| {
                if horizontal {
                    el.left_0().right_0().top(-slop).bottom(-slop)
                } else {
                    el.top_0().bottom_0().left(-slop).right(-slop)
                }
            })
            .when(!ctx.disabled, |el| {
                el.on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    let ctx = &press_ctx;
                    ctx.start_drag(0.0, window, cx);
                    ctx.drag_to(
                        e.position,
                        e.modifiers.alt,
                        FaderChangeReason::TrackPress,
                        window,
                        cx,
                    );
                })
            });

        let mut track = div()
            .relative()
            .flex_grow(1.0)
            .rounded_full()
            .bg(theme.input.opacity(0.9))
            .map(|el| {
                if horizontal {
                    el.h(px(ctx.track_size)).w_full()
                } else {
                    el.w(px(ctx.track_size)).h_full()
                }
            })
            .child(tracker)
            .children(self.children);
        track.style().refine(&self.style);

        div()
            .relative()
            .flex()
            .items_center()
            .min_w_0()
            .min_h_0()
            .map(|el| {
                if horizontal {
                    el.h(thumb).w_full().px(half)
                } else {
                    el.w(thumb).h_full().flex_col().py(half)
                }
            })
            // `[data-slot=channel-strip-fader]`: no padding, and an only-child
            // control overlaps by (track - thumb) / 2 across the travel.
            .when(ctx.in_strip, |el| el.p_0())
            .when(ctx.in_strip && ctx.single_child, |el| {
                let margin = px((ctx.track_size - ctx.thumb_size) / 2.0);
                if horizontal {
                    el.my(margin)
                } else {
                    el.mx(margin)
                }
            })
            .child(hit)
            .child(track)
    }
}

/// The fill between the origin and the value: `bg-primary rounded-full`.
#[derive(IntoElement)]
pub struct FaderRange {
    style: StyleRefinement,
}

impl FaderRange {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Default for FaderRange {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for FaderRange {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderRange {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderRange");
        let start = ctx.origin_position.min(ctx.position);
        let end = ctx.origin_position.max(ctx.position);
        let theme = cx.theme();
        let color = if ctx.disabled {
            disabled_flatten(theme.primary, theme.background)
        } else {
            theme.primary
        };
        let mut el = div()
            .absolute()
            .rounded_full()
            .bg(color)
            .when(ctx.disabled, |el| el.opacity(DISABLED_UNFADE))
            .map(|el| {
                if ctx.horizontal() {
                    el.top_0()
                        .bottom_0()
                        .left(relative(start))
                        .w(relative(end - start))
                } else {
                    el.left_0()
                        .right_0()
                        .bottom(relative(start))
                        .h(relative(end - start))
                }
            });
        el.style().refine(&self.style);
        el
    }
}

/// The draggable, focusable thumb. Double-click resets to `reset_value`.
#[derive(IntoElement)]
pub struct FaderThumb {
    style: StyleRefinement,
}

impl FaderThumb {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Default for FaderThumb {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for FaderThumb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderThumb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderThumb");
        let theme = cx.theme().clone();
        let horizontal = ctx.horizontal();
        let size = ctx.thumb_size;
        let console = ctx.variant == FaderVariant::Console;
        let (width, height) = match (console, horizontal) {
            (false, _) => (size, size),
            (true, true) => (size * 0.7, size * 1.6),
            (true, false) => (size * 1.8, size * 0.7),
        };

        let focus = ctx.state.read(cx).thumb_focus.clone();
        let focus_visible =
            !ctx.disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let (t, animating) = ctx.state.update(cx, |s, _| {
            let active = !ctx.disabled && (s.thumb_hovered || s.dragging || focus_visible);
            s.ring.update(active)
        });
        if animating {
            window.request_animation_frame();
        }
        let radius = if console {
            radius_sm(&theme)
        } else {
            px(height / 2.0)
        };

        let disabled = ctx.disabled;
        let flat = |color: Hsla| {
            if disabled {
                disabled_flatten(color, theme.background)
            } else {
                color
            }
        };
        let line = div().absolute().bg(flat(theme.foreground)).map(|el| {
            if horizontal {
                el.top(px(-1.0))
                    .bottom(px(-1.0))
                    .left(px(width / 2.0 - 1.5))
                    .w(px(1.0))
            } else {
                el.left(px(-1.0))
                    .right(px(-1.0))
                    .top(px(height / 2.0 - 1.5))
                    .h(px(1.0))
            }
        });

        let label = ctx.aria_label.clone();
        let text = (ctx.format)(ctx.value);
        let mut thumb = div()
            .id("fader-thumb")
            .absolute()
            .left(px(-width / 2.0))
            .top(px(-height / 2.0))
            .w(px(width))
            .h(px(height))
            .flex_none()
            .rounded(radius)
            .bg(theme.background)
            .shadow(
                thumb_shadows(&theme, t, focus_visible)
                    .into_iter()
                    .map(|mut shadow| {
                        if disabled {
                            shadow.color = shadow.color.opacity(0.5);
                        }
                        shadow
                    })
                    .collect(),
            )
            .when(disabled, |el| el.opacity(DISABLED_UNFADE))
            .role(gpui::Role::Slider)
            .aria_value(text)
            .when_some(label, |el, l| el.aria_label(l))
            .when(console, |el| {
                el.border_1().border_color(flat(theme.border)).child(line)
            });
        thumb.style().refine(&self.style);

        let thumb = if ctx.disabled {
            thumb
        } else {
            let hover = ctx.state.clone();
            let down = ctx.clone();
            let keys = ctx.clone();
            thumb
                .track_focus(&focus)
                .on_hover(move |hovered, _, cx| {
                    hover.update(cx, |s, cx| {
                        s.thumb_hovered = *hovered;
                        cx.notify();
                    });
                })
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if e.click_count >= 2 {
                        down.reset(window, cx);
                        return;
                    }
                    let offset = down
                        .thumb_center(cx)
                        .map_or(0.0, |center| down.axis(e.position) - center);
                    // Vertical travel runs bottom-up while y runs top-down.
                    down.start_drag(offset, window, cx);
                })
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    if keys.handle_key(e, window, cx) {
                        cx.stop_propagation();
                    }
                })
        };

        // A zero-size anchor at the value, so the thumb centres on it.
        div()
            .absolute()
            .size_0()
            .map(|el| {
                if horizontal {
                    el.left(relative(ctx.position)).top(relative(0.5))
                } else {
                    el.bottom(relative(ctx.position)).left(relative(0.5))
                }
            })
            .child(thumb)
    }
}

/// A [`DbScale`] beside the fader, matched to its range, taper and
/// orientation, inset by half a thumb so the ticks line up with the travel.
/// Takes the scale's other props (`ticks`, `side`, `labels`, `marks`,
/// `format`, custom ticks); `Styled` applies to the wrapper.
#[derive(IntoElement)]
pub struct FaderScale {
    ticks: Option<Vec<f32>>,
    side: Option<DbScaleSide>,
    labels: Option<bool>,
    marks: Option<bool>,
    format: Option<DbScaleFormat>,
    children: Vec<DbScaleTick>,
    style: StyleRefinement,
}

impl FaderScale {
    pub fn new() -> Self {
        Self {
            ticks: None,
            side: None,
            labels: None,
            marks: None,
            format: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Tick values in dB. Default: the scale's.
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = Some(ticks.into_iter().collect());
        self
    }

    /// Which side of the marks the labels sit on. Default `End`.
    pub fn side(mut self, side: DbScaleSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Show tick labels. Default true.
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Draw the tick marks. Default true.
    pub fn marks(mut self, marks: bool) -> Self {
        self.marks = Some(marks);
        self
    }

    /// Label text for a tick value.
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// A custom tick (React children). Custom ticks replace the default ones.
    pub fn child(mut self, tick: DbScaleTick) -> Self {
        self.children.push(tick);
        self
    }

    /// Custom ticks.
    pub fn children(mut self, ticks: impl IntoIterator<Item = DbScaleTick>) -> Self {
        self.children.extend(ticks);
        self
    }
}

impl Default for FaderScale {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for FaderScale {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderScale {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderScale");
        let half = px(ctx.thumb_size / 2.0);
        let mut scale = DbScale::new()
            .min_db(ctx.min)
            .max_db(ctx.max)
            .orientation(ctx.orientation)
            .taper(ctx.taper.clone())
            .children(self.children);
        if let Some(ticks) = self.ticks {
            scale = scale.ticks(ticks);
        }
        if let Some(side) = self.side {
            scale = scale.side(side);
        }
        if let Some(labels) = self.labels {
            scale = scale.labels(labels);
        }
        if let Some(marks) = self.marks {
            scale = scale.marks(marks);
        }
        if let Some(format) = self.format {
            scale = scale.format(move |db| format(db));
        }
        let mut el = div()
            .map(|el| {
                if ctx.in_strip {
                    el
                } else if ctx.horizontal() {
                    el.px(half)
                } else {
                    el.py(half)
                }
            })
            .child(scale);
        el.style().refine(&self.style);
        el
    }
}

/// The value as text, at a fixed width so moving the fader never shifts the
/// layout. `editable(true)` makes it a button that opens a text field:
/// type "−6", "3 dB" or "-inf", Enter applies, Escape cancels.
#[derive(IntoElement)]
pub struct FaderValue {
    editable: bool,
    style: StyleRefinement,
}

impl FaderValue {
    pub fn new() -> Self {
        Self {
            editable: false,
            style: StyleRefinement::default(),
        }
    }

    /// Click to type a value. Default false.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }
}

impl Default for FaderValue {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for FaderValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderValue {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderValue");
        let theme = cx.theme().clone();
        let ch = ch_width(&theme.font_mono, px(12.0), window);
        let width = ch * widest_value(&*ctx.format, ctx.min, ctx.max) as f32;
        let (editing, draft, editor_focus, value_focus) = {
            let s = ctx.state.read(cx);
            (
                s.editing,
                s.draft.clone(),
                s.editor_focus.clone(),
                s.value_focus.clone(),
            )
        };
        let text = (ctx.format)(ctx.value);
        let base = |el: Div| {
            el.flex()
                .flex_none()
                .items_center()
                .justify_end()
                .font_family(theme.font_mono.clone())
                .text_xs_()
                .whitespace_nowrap()
        };

        if editing {
            let focused = editor_focus.is_focused(window);
            let keys = ctx.clone();
            let mut el = base(div())
                .id("fader-value-input")
                .h(px(24.0))
                .w(width + px(12.0))
                .px(px(5.0))
                .rounded(radius_md(&theme))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .text_color(theme.foreground)
                .aria_label("Value in dB")
                .track_focus(&editor_focus)
                .when(focused, |el| {
                    el.shadow(vec![css_shadow(
                        0.0,
                        0.0,
                        0.0,
                        3.0,
                        theme.ring.opacity(0.3),
                    )])
                })
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    cx.stop_propagation();
                    match e.keystroke.key.as_str() {
                        "enter" | "escape" => {
                            let apply = e.keystroke.key == "enter";
                            let draft = keys.state.update(cx, |s, cx| {
                                s.editing = false;
                                cx.notify();
                                std::mem::take(&mut s.draft.text)
                            });
                            // The input unmounts, so focus returns to the page.
                            window.blur(cx);
                            if apply {
                                keys.apply_draft(&draft, window, cx);
                            }
                        }
                        _ => {
                            keys.state.update(cx, |s, cx| {
                                if s.draft.handle_key(&e.keystroke, cx) {
                                    cx.notify();
                                }
                            });
                        }
                    }
                })
                .child(draft.render(focused, theme.foreground));
            el.style().refine(&self.style);
            return el.into_any_element();
        }

        if self.editable {
            let focus_visible = value_focus.is_focused(window) && window.last_input_was_keyboard();
            let open = ctx.clone();
            let open_editor = move |window: &mut Window, cx: &mut App| {
                let value = open.value;
                let editor = open.state.update(cx, |s, cx| {
                    s.draft = TextDraft::new(if value == SILENCE_DB {
                        "-inf".to_string()
                    } else {
                        js_string(value)
                    });
                    s.draft.select_all();
                    s.editing = true;
                    cx.notify();
                    s.editor_focus.clone()
                });
                window.focus(&editor, cx);
            };
            let open_editor = Rc::new(open_editor);
            let click = open_editor.clone();
            let mut el = base(div())
                .id("fader-value")
                .h(px(24.0))
                .w(width + px(12.0))
                .px(px(6.0))
                .rounded(radius_md(&theme))
                .text_color(theme.muted_foreground)
                .role(gpui::Role::Button)
                .when(focus_visible, |el| {
                    el.shadow(vec![css_shadow(
                        0.0,
                        0.0,
                        0.0,
                        3.0,
                        theme.ring.opacity(0.3),
                    )])
                })
                .when(!ctx.disabled, |el| {
                    let muted = theme.muted;
                    el.track_focus(&value_focus)
                        .hover(move |s| s.bg(muted))
                        .on_click(move |_, window, cx| click(window, cx))
                        .on_key_down(move |e: &KeyDownEvent, window, cx| {
                            if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                                cx.stop_propagation();
                                open_editor(window, cx);
                            }
                        })
                })
                .child(text);
            el.style().refine(&self.style);
            return el.into_any_element();
        }

        let mut el = base(div())
            .w(width)
            .text_color(theme.muted_foreground)
            .child(text);
        el.style().refine(&self.style);
        el.into_any_element()
    }
}

/// A "Reset" button, hidden (`opacity-0`) while the value equals `reset_value`.
#[derive(IntoElement)]
pub struct FaderReset {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl FaderReset {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for FaderReset {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for FaderReset {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for FaderReset {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FaderReset {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = fader_ctx("FaderReset");
        let theme = cx.theme().clone();
        let modified = ctx.value != ctx.reset_value;
        let disabled = ctx.disabled || !modified;
        let focus = ctx.state.read(cx).reset_focus.clone();
        reset_button(
            "fader-reset",
            &theme,
            disabled,
            &focus,
            window,
            self.children,
            self.style,
            move |window, cx| ctx.reset(window, cx),
        )
    }
}

/// The "Reset" button shared by the fader and the parameter slider.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reset_button(
    id: &'static str,
    theme: &Theme,
    disabled: bool,
    focus: &FocusHandle,
    window: &Window,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_reset: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let focus_visible = !disabled && focus.is_focused(window) && window.last_input_was_keyboard();
    let on_reset = Rc::new(on_reset);
    let click = on_reset.clone();
    let (muted, foreground) = (theme.muted, theme.foreground);
    let mut el = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(24.0))
        .px(px(6.0))
        .rounded(radius_md(theme))
        .text_xs_()
        .text_color(theme.muted_foreground)
        .role(gpui::Role::Button)
        .aria_label("Reset")
        .when(focus_visible, |el| {
            el.shadow(vec![css_shadow(
                0.0,
                0.0,
                0.0,
                3.0,
                theme.ring.opacity(0.3),
            )])
        })
        .when(disabled, |el| el.opacity(0.0))
        .when(!disabled, |el| {
            el.track_focus(focus)
                .hover(move |s| s.bg(muted).text_color(foreground))
                .on_click(move |_, window, cx| click(window, cx))
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        on_reset(window, cx);
                    }
                })
        })
        .map(|el| {
            if children.is_empty() {
                el.child("Reset")
            } else {
                el.children(children)
            }
        });
    el.style().refine(&style);
    el.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::taper::{AudioTaperOptions, audio_taper, linear_taper};

    fn law(silence_at_min: bool) -> FaderLaw {
        FaderLaw {
            min: -60.0,
            max: 6.0,
            step: 0.5,
            large_step: 6.0,
            fine_step: 0.1,
            silence_at_min,
            detents: vec![0.0],
            taper: AnyTaper::Linear(linear_taper(-60.0, 6.0)),
        }
    }

    #[test]
    fn fader_reports_its_value_in_db() {
        assert_eq!(default_fader_format(-6.0), "\u{2212}6.0 dB");
        assert_eq!(default_fader_format(SILENCE_DB), "Silent");
    }

    #[test]
    fn fader_steps_by_step_large_step_and_fine_step() {
        let law = law(false);
        let v = law.key_target("down", false, false, 0.0).unwrap();
        assert_eq!(v, -0.5);
        let v = law.key_target("down", true, false, v).unwrap();
        assert_eq!(v, -6.5);
        let v = law.key_target("up", false, true, v).unwrap();
        assert_eq!(v, -6.4);
    }

    #[test]
    fn fader_jumps_to_the_ends_and_to_silence_when_allowed() {
        let law = law(true);
        assert_eq!(law.key_target("end", false, false, 0.0), Some(6.0));
        assert_eq!(law.key_target("home", false, false, 6.0), Some(SILENCE_DB));
        assert_eq!(default_fader_format(SILENCE_DB), "Silent");
        // Without silence, Home is the bottom of the range.
        assert_eq!(law_home(), Some(-60.0));
    }

    fn law_home() -> Option<f32> {
        law(false).key_target("home", false, false, 0.0)
    }

    #[test]
    fn fader_commits_page_up() {
        assert_eq!(
            law(false).key_target("pageup", false, false, -3.0),
            Some(3.0)
        );
        assert_eq!(
            law(false).key_target("pagedown", false, false, -3.0),
            Some(-9.0)
        );
        assert_eq!(law(false).key_target("a", false, false, -3.0), None);
    }

    #[test]
    fn fader_nudges_out_of_and_into_silence() {
        let law = law(true);
        assert_eq!(law.nudge(SILENCE_DB, 1.0, 0.5), -60.0);
        assert_eq!(law.nudge(SILENCE_DB, -1.0, 0.5), SILENCE_DB);
        assert_eq!(law.nudge(-60.0, -1.0, 0.5), SILENCE_DB);
        assert_eq!(super::tests::law(false).nudge(-60.0, -1.0, 0.5), -60.0);
    }

    #[test]
    fn fader_quantizes_to_the_step_and_clamps() {
        let law = law(false);
        assert_eq!(law.quantize(-6.26, 0.5), -6.5);
        assert_eq!(law.quantize(-6.24, 0.5), -6.0);
        assert_eq!(law.quantize(12.0, 0.5), 6.0);
        assert_eq!(law.quantize(-100.0, 0.5), -60.0);
        assert_eq!(law.quantize(SILENCE_DB, 0.5), -60.0);
    }

    #[test]
    fn fader_snaps_to_detents_and_silence_at_the_bottom() {
        let law = law(true);
        let unity = law.to_position(0.0);
        assert_eq!(law.value_at(unity + 0.011, false), 0.0);
        assert_ne!(law.value_at(unity + 0.02, false), 0.0);
        assert_eq!(law.value_at(0.0, false), SILENCE_DB);
        assert_eq!(law.value_at(1.0, false), 6.0);
        // Fine resolution while Alt is held.
        let p = law.to_position(-30.13);
        assert_eq!(law.value_at(p, true), -30.1);
        assert_eq!(law.value_at(p, false), -30.0);
    }

    #[test]
    fn fader_maps_audio_taper_positions() {
        let law = FaderLaw {
            taper: AnyTaper::Audio(audio_taper(AudioTaperOptions {
                min_db: -80.0,
                max_db: 6.0,
                ..Default::default()
            })),
            min: -80.0,
            ..law(true)
        };
        assert!((law.to_position(0.0) - 0.75).abs() < 1e-6);
        assert_eq!(law.to_position(SILENCE_DB), 0.0);
        assert_eq!(law.value_at(0.75, false), 0.0);
    }

    #[test]
    fn fader_parses_typed_values() {
        assert_eq!(parse_db("-6"), Some(-6.0));
        assert_eq!(parse_db("\u{2212}6 dB"), Some(-6.0));
        assert_eq!(parse_db("3.5db"), Some(3.5));
        assert_eq!(parse_db(" -inf "), Some(SILENCE_DB));
        assert_eq!(parse_db("\u{221e}"), Some(SILENCE_DB));
        assert_eq!(parse_db("Infinity"), Some(SILENCE_DB));
        assert_eq!(parse_db(""), None);
        assert_eq!(parse_db("loud"), None);
    }

    #[test]
    fn fader_value_width_covers_the_widest_text() {
        // "Silent" (6), "−60.0 dB" (8), "+6.0 dB", "−8.8 dB", "+5.9 dB".
        assert_eq!(widest_value(&default_fader_format, -60.0, 6.0), 8);
        assert_eq!(widest_value(&default_fader_format, -24.0, 24.0), 8);
    }

    #[test]
    fn fader_drafts_show_js_numbers() {
        assert_eq!(js_string(-6.0), "-6");
        assert_eq!(js_string(-6.4), "-6.4");
        assert_eq!(js_string(0.0), "0");
    }

    #[test]
    fn fader_snaps_positions_to_the_slider_step() {
        assert_eq!(snap_position(0.50024, POSITION_STEP), 0.5);
        assert_eq!(snap_position(0.50026, POSITION_STEP), 0.5005);
        assert_eq!(snap_position(-1.0, POSITION_STEP), 0.0);
        assert_eq!(snap_position(2.0, POSITION_STEP), 1.0);
    }

    #[test]
    fn fader_text_draft_edits() {
        let mut d = TextDraft::new("-6");
        d.select_all();
        d.insert("1");
        assert_eq!(d.text, "1");
        d.insert("2");
        assert_eq!(d.text, "12");
        d.anchor = d.prev(d.cursor);
        d.insert("");
        assert_eq!(d.text, "1");
    }
}
