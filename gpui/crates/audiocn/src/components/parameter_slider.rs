//! Port of `components/ui/parameter-slider.tsx`.
//!
//! A labelled slider with a numeric input, unit, marks and reset, for
//! everything that is not a volume (delay, frequency, ratio, ...). Compound:
//! [`ParameterSlider`] provides its state to [`ParameterSliderHeader`],
//! [`ParameterSliderLabel`], [`ParameterSliderInput`],
//! [`ParameterSliderValue`], [`ParameterSliderReset`],
//! [`ParameterSliderControl`], [`ParameterSliderMarks`] and
//! [`ParameterSliderDescription`].

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, CursorStyle, DispatchPhase, ElementId, Entity, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Refineable, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Window,
    canvas, div, px, relative,
};

use crate::audio::decibels::clamp;
use crate::audio::taper::{AnyTaper, Taper, linear_taper, log_taper};
use crate::components::fader::{
    DISABLED_UNFADE, TextDraft, ThumbRing, ch_width, decimal, disabled_flatten, js_number,
    js_round, js_string, reset_button, round_value, snap_position, thumb_shadows,
};
use crate::config::AudioConfig;
use crate::theme::ActiveTheme;
use crate::ui::styles::{TextSize, css_shadow, provide, radius_lg, use_context};

const POSITION_STEP: f32 = 0.0005;
/// Points along the range sampled to find the widest value text.
const WIDTH_SAMPLES: usize = 24;
const HIT_SLOP: f32 = 6.0;
/// Base UI `NumberField.ScrubArea` `pixelSensitivity`.
const SCRUB_PIXELS: f32 = 2.0;

/// Why a parameter's value changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterChangeReason {
    Drag,
    Keyboard,
    Input,
    Reset,
}

/// Passed to `on_value_change_with_details`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterChangeDetails {
    pub reason: ParameterChangeReason,
}

/// A labelled point under the slider (`marks`).
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterMark {
    pub value: f32,
    pub label: Option<SharedString>,
}

impl ParameterMark {
    pub fn new(value: f32) -> Self {
        Self { value, label: None }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// `scale`: `Log` suits frequency and time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParameterScale {
    #[default]
    Linear,
    Log,
}

type ChangeHandler = Rc<dyn Fn(f32, ParameterChangeDetails, &mut Window, &mut App)>;
type CommitHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type FormatFn = Rc<dyn Fn(f32) -> String>;
type DraftHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Pure logic

/// Digits after the decimal point of `String(step)`.
pub(crate) fn decimals_of(step: f32) -> usize {
    let text = js_string(step);
    text.find('.').map_or(0, |dot| text.len() - dot - 1)
}

/// JavaScript `Number.prototype.toFixed` for the value as React holds it.
pub(crate) fn to_fixed(value: f32, decimals: usize) -> String {
    let v = decimal(value);
    if !v.is_finite() {
        return js_string(value);
    }
    let scale = 10f64.powi(decimals as i32);
    let n = js_round_half_away(v.abs() * scale);
    let p = 10u64.pow(decimals as u32);
    let n = n as u64;
    let digits = if decimals == 0 {
        format!("{n}")
    } else {
        format!("{}.{:0width$}", n / p, n % p, width = decimals)
    };
    if v < 0.0 && n > 0 {
        format!("-{digits}")
    } else if v < 0.0 && n == 0 {
        // (-0.001).toFixed(1) is "-0.0".
        format!("-{digits}")
    } else {
        digits
    }
}

fn js_round_half_away(x: f64) -> f64 {
    // `toFixed` picks the larger n on a tie; x is non-negative here.
    let floor = x.floor();
    if x - floor >= 0.5 - 1e-9 {
        floor + 1.0
    } else {
        floor
    }
}

/// `Intl.NumberFormat("en-US", { minimumFractionDigits: d, maximumFractionDigits: d })`.
pub(crate) fn format_number(value: f32, decimals: usize) -> String {
    let fixed = to_fixed(value, decimals);
    let (sign, rest) = match fixed.strip_prefix('-') {
        Some(rest) if rest.chars().any(|c| c.is_ascii_digit() && c != '0') => ("-", rest),
        Some(rest) => ("-", rest),
        None => ("", fixed.as_str()),
    };
    let (int, frac) = match rest.find('.') {
        Some(dot) => (&rest[..dot], &rest[dot..]),
        None => (rest, ""),
    };
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    // Intl keeps "-0" for negative zero, like React's number field shows it.
    format!("{sign}{grouped}{frac}")
}

/// Parses number-field text ("1,000", "−5", "12.5"). `None` if not a number.
pub(crate) fn parse_number(text: &str) -> Option<f32> {
    let cleaned: String = text
        .replace('\u{2212}', "-")
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    js_number(&cleaned).map(|n| n as f32)
}

/// Step quantizing and keyboard stepping.
#[derive(Clone, Debug)]
pub(crate) struct ParameterLaw {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub large_step: f32,
    pub taper: AnyTaper,
}

impl ParameterLaw {
    pub fn quantize(&self, next: f64) -> f32 {
        let (min, step) = (decimal(self.min), decimal(self.step));
        let stepped = min + js_round((next - min) / step) * step;
        round_value(clamp(stepped as f32, self.min, self.max) as f64) as f32
    }

    pub fn value_at(&self, position: f32) -> f32 {
        self.quantize(decimal(self.taper.to_value(position)))
    }

    pub fn key_target(&self, key: &str, shift: bool, current: f32) -> Option<f32> {
        let increment = decimal(if shift { self.large_step } else { self.step });
        let current = decimal(current);
        let next = match key {
            "up" | "right" => current + increment,
            "down" | "left" => current - increment,
            "pageup" => current + decimal(self.large_step),
            "pagedown" => current - decimal(self.large_step),
            "home" => decimal(self.min),
            "end" => decimal(self.max),
            _ => return None,
        };
        Some(self.quantize(next))
    }

    /// Characters in the widest value along the range.
    pub fn widest_value(&self, format: &dyn Fn(f32) -> String) -> usize {
        (0..=WIDTH_SAMPLES)
            .map(|i| {
                let sample = self.value_at(i as f32 / WIDTH_SAMPLES as f32);
                format(sample).encode_utf16().count()
            })
            .max()
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// State and context

struct ParamState {
    uncontrolled: f32,
    latest: f32,
    thumb_focus: FocusHandle,
    input_focus: FocusHandle,
    reset_focus: FocusHandle,
    dragging: bool,
    drag_offset: f32,
    hovered: bool,
    ring: ThumbRing,
    track: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The number field's text while it has focus.
    draft: Option<TextDraft>,
    /// Scrubbing the unit: (pointer x at the start, value at the start).
    scrub: Option<(f32, f32)>,
    /// `ParameterSliderLabel`'s text: the thumb's accessible name.
    label: Option<SharedString>,
    apply_draft: Option<DraftHandler>,
    _input_blur: Option<Subscription>,
}

#[derive(Clone)]
struct ParamCtx {
    state: Entity<ParamState>,
    controlled: bool,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    large_step: f32,
    unit: Option<SharedString>,
    decimals: usize,
    reset_value: f32,
    position: f32,
    origin_position: f32,
    law: Rc<ParameterLaw>,
    disabled: bool,
    marks: Rc<Vec<ParameterMark>>,
    format: FormatFn,
    on_change: Option<ChangeHandler>,
    on_commit: Option<CommitHandler>,
}

impl ParamCtx {
    fn latest(&self, cx: &App) -> f32 {
        self.state.read(cx).latest
    }

    fn change(&self, next: f32, reason: ParameterChangeReason, window: &mut Window, cx: &mut App) {
        let controlled = self.controlled;
        let changed = self.state.update(cx, |s, cx| {
            if next == s.latest {
                return false;
            }
            s.latest = next;
            if !controlled {
                s.uncontrolled = next;
            }
            cx.notify();
            true
        });
        if changed && let Some(handler) = &self.on_change {
            handler(next, ParameterChangeDetails { reason }, window, cx);
        }
    }

    fn commit(&self, value: f32, window: &mut Window, cx: &mut App) {
        if let Some(handler) = &self.on_commit {
            handler(value, window, cx);
        }
    }

    fn commit_latest(&self, window: &mut Window, cx: &mut App) {
        let latest = self.latest(cx);
        self.commit(latest, window, cx);
    }

    fn reset(&self, window: &mut Window, cx: &mut App) {
        self.change(self.reset_value, ParameterChangeReason::Reset, window, cx);
        self.commit(self.reset_value, window, cx);
    }

    fn handle_key(&self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
        let current = self.latest(cx);
        let Some(next) = self.law.key_target(
            &event.keystroke.key,
            event.keystroke.modifiers.shift,
            current,
        ) else {
            return false;
        };
        self.change(next, ParameterChangeReason::Keyboard, window, cx);
        self.commit(next, window, cx);
        true
    }

    fn pointer_position(&self, pointer: Point<Pixels>, cx: &App) -> Option<f32> {
        let s = self.state.read(cx);
        let track = s.track.get()?;
        let width = track.size.width.as_f32();
        if width <= 0.0 {
            return None;
        }
        let position = (pointer.x.as_f32() - s.drag_offset - track.origin.x.as_f32()) / width;
        Some(snap_position(position, POSITION_STEP))
    }

    fn drag_to(&self, pointer: Point<Pixels>, window: &mut Window, cx: &mut App) {
        if let Some(position) = self.pointer_position(pointer, cx) {
            let next = self.law.value_at(position);
            self.change(next, ParameterChangeReason::Drag, window, cx);
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
            self.commit_latest(window, cx);
        }
    }

    /// The number field's value on blur / Enter: clamped, not snapped.
    fn apply_draft(&self, draft: &str, window: &mut Window, cx: &mut App) {
        let Some(parsed) = parse_number(draft) else {
            return;
        };
        let next = clamp(parsed, self.min, self.max);
        self.change(next, ParameterChangeReason::Input, window, cx);
        self.commit(next, window, cx);
    }

    /// A number-field step (arrow keys, scrubbing): clamped, not snapped.
    fn field_step(&self, from: f32, by: f64) -> f32 {
        clamp(round_value(decimal(from) + by) as f32, self.min, self.max)
    }
}

fn param_ctx(part: &str) -> ParamCtx {
    use_context::<ParamCtx>()
        .unwrap_or_else(|| panic!("{part} must be used inside ParameterSlider."))
}

// ---------------------------------------------------------------------------
// ParameterSlider

/// A labelled slider with a numeric input, unit, marks and reset.
///
/// ```ignore
/// ParameterSlider::new("offset")
///     .min(-1000.0).max(1000.0).step(5.0).large_step(50.0).origin(0.0).unit("ms")
///     .value(offset).on_value_change(|v, _, cx| set_offset(v, cx))
///     .child(ParameterSliderHeader::new()
///         .child(ParameterSliderLabel::new("Sync offset"))
///         .child(ParameterSliderReset::new())
///         .child(ParameterSliderInput::new()))
///     .child(ParameterSliderControl::new())
/// ```
#[derive(IntoElement)]
pub struct ParameterSlider {
    id: ElementId,
    value: Option<f32>,
    default_value: Option<f32>,
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    min: f32,
    max: f32,
    step: f32,
    large_step: f32,
    unit: Option<SharedString>,
    decimals: Option<usize>,
    scale: ParameterScale,
    origin: Option<f32>,
    reset_value: Option<f32>,
    marks: Vec<ParameterMark>,
    format: Option<FormatFn>,
    disabled: Option<bool>,
    config: AudioConfig,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ParameterSlider {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: None,
            on_value_change: None,
            on_value_committed: None,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            large_step: 10.0,
            unit: None,
            decimals: None,
            scale: ParameterScale::Linear,
            origin: None,
            reset_value: None,
            marks: Vec::new(),
            format: None,
            disabled: None,
            config: AudioConfig::default(),
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// The value (controlled).
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    pub fn default_value(mut self, value: f32) -> Self {
        self.default_value = Some(value);
        self
    }

    pub fn on_value_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_change = Some(Rc::new(move |v, _, window, cx| f(v, window, cx)));
        self
    }

    /// `onValueChange(value, details)`.
    pub fn on_value_change_with_details(
        mut self,
        f: impl Fn(f32, ParameterChangeDetails, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    /// Fires when a drag ends, after keyboard input, typing and reset.
    pub fn on_value_committed(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_committed = Some(Rc::new(f));
        self
    }

    /// Default 0.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Default 100.
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Default 1.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Shift+arrow and Page Up/Down. Default 10.
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Suffix such as "ms", "dB" or "Hz".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Digits after the decimal point. Default: from `step`.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// `Log` suits frequency and time. Default `Linear`.
    pub fn scale(mut self, scale: ParameterScale) -> Self {
        self.scale = scale;
        self
    }

    /// Where the fill starts; the middle of a bipolar range. Default `min`.
    pub fn origin(mut self, origin: f32) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Value restored by reset and double-click. Default `default_value` or `min`.
    pub fn reset_value(mut self, value: f32) -> Self {
        self.reset_value = Some(value);
        self
    }

    pub fn marks(mut self, marks: impl IntoIterator<Item = ParameterMark>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Disabled from a surrounding container (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }
}

impl ParentElement for ParameterSlider {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for ParameterSlider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParameterSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let (min, max) = (self.min, self.max);
        let reset_value = self.reset_value.or(self.default_value).unwrap_or(min);
        let initial = clamp(self.default_value.unwrap_or(reset_value), min, max);
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let input_focus = cx.focus_handle().tab_stop(true);
            let blur = cx.on_blur(&input_focus, window, |s: &mut ParamState, window, cx| {
                let Some(draft) = s.draft.take() else {
                    return;
                };
                if let Some(apply) = s.apply_draft.clone() {
                    window.defer(cx, move |window, cx| apply(draft.text, window, cx));
                }
                cx.notify();
            });
            ParamState {
                uncontrolled: initial,
                latest: initial,
                thumb_focus: cx.focus_handle().tab_stop(true),
                input_focus,
                reset_focus: cx.focus_handle().tab_stop(true),
                dragging: false,
                drag_offset: 0.0,
                hovered: false,
                ring: ThumbRing::default(),
                track: Rc::new(Cell::new(None)),
                draft: None,
                scrub: None,
                label: None,
                apply_draft: None,
                _input_blur: Some(blur),
            }
        });
        let controlled = self.value.is_some();
        let value = state.update(cx, |s, _| {
            let value = self.value.unwrap_or(s.uncontrolled);
            // Every render, so a rejected change is dropped.
            s.latest = value;
            if disabled {
                s.dragging = false;
                s.scrub = None;
            }
            value
        });
        let decimals = self.decimals.unwrap_or_else(|| decimals_of(self.step));
        let taper = match self.scale {
            ParameterScale::Log => AnyTaper::Log(log_taper(min, max)),
            ParameterScale::Linear => AnyTaper::Linear(linear_taper(min, max)),
        };
        let format: FormatFn = match self.format {
            Some(f) => f,
            None => {
                let unit = self.unit.clone();
                Rc::new(move |v| {
                    let text = to_fixed(v, decimals);
                    match &unit {
                        Some(unit) if !unit.is_empty() => format!("{text} {unit}"),
                        _ => text,
                    }
                })
            }
        };
        let position = taper.to_position(value);
        let origin_position = taper.to_position(clamp(self.origin.unwrap_or(min), min, max));
        let law = Rc::new(ParameterLaw {
            min,
            max,
            step: self.step,
            large_step: self.large_step,
            taper,
        });
        let ctx = ParamCtx {
            state: state.clone(),
            controlled,
            value,
            min,
            max,
            step: self.step,
            large_step: self.large_step,
            unit: self.unit,
            decimals,
            reset_value,
            position,
            origin_position,
            law,
            disabled,
            marks: Rc::new(self.marks),
            format,
            on_change: self.on_value_change,
            on_commit: self.on_value_committed,
        };
        let apply_ctx = ctx.clone();
        state.update(cx, |s, _| {
            s.apply_draft = Some(Rc::new(move |draft: String, window, cx| {
                apply_ctx.apply_draft(&draft, window, cx);
            }));
        });

        let mut root = div()
            .id(self.id)
            .flex()
            .w_full()
            .flex_col()
            .gap_2()
            .when(disabled, |el| el.opacity(0.5))
            .role(gpui::Role::Group)
            .children(self.children);
        root.style().refine(&self.style);
        provide(ctx, root)
    }
}

// ---------------------------------------------------------------------------
// Parts

macro_rules! styled_parent {
    ($ty:ident) => {
        impl ParentElement for $ty {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }
        impl Styled for $ty {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }
    };
}

/// The row above the slider: `flex min-h-6 items-center gap-2`.
#[derive(IntoElement, Default)]
pub struct ParameterSliderHeader {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ParameterSliderHeader {
    pub fn new() -> Self {
        Self::default()
    }
}

styled_parent!(ParameterSliderHeader);

impl RenderOnce for ParameterSliderHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .min_h(px(24.0))
            .items_center()
            .gap_2()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The parameter's name: `mr-auto text-sm font-medium`.
#[derive(IntoElement)]
pub struct ParameterSliderLabel {
    text: Option<SharedString>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ParameterSliderLabel {
    pub fn new(label: impl Into<SharedString>) -> Self {
        let text = label.into();
        Self {
            text: Some(text.clone()),
            style: StyleRefinement::default(),
            children: vec![text.into_any_element()],
        }
    }

    pub fn empty() -> Self {
        Self {
            text: None,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

styled_parent!(ParameterSliderLabel);

impl RenderOnce for ParameterSliderLabel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderLabel");
        if self.text.is_some() {
            ctx.state.update(cx, |s, _| s.label = self.text.clone());
        }
        let mut el = div()
            .mr_auto()
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The numeric input, with the unit as a scrub handle (drag it sideways).
/// Arrow keys step (Shift: `large_step`); Enter or blur applies typed text.
#[derive(IntoElement)]
pub struct ParameterSliderInput {
    scrub: bool,
    style: StyleRefinement,
}

impl ParameterSliderInput {
    pub fn new() -> Self {
        Self {
            scrub: true,
            style: StyleRefinement::default(),
        }
    }

    /// Drag the unit to change the value. Default true.
    pub fn scrub(mut self, scrub: bool) -> Self {
        self.scrub = scrub;
        self
    }
}

impl Default for ParameterSliderInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for ParameterSliderInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParameterSliderInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderInput");
        let theme = cx.theme().clone();
        let (focus, draft, scrubbing) = {
            let s = ctx.state.read(cx);
            (s.input_focus.clone(), s.draft.clone(), s.scrub.is_some())
        };
        let focused = focus.is_focused(window);
        let shown = format_number(ctx.value, ctx.decimals);

        // Starts editing when focus arrives (click or Tab).
        if focused && draft.is_none() {
            ctx.state.update(cx, |s, _| {
                let mut d = TextDraft::new(shown.clone());
                d.select_all();
                s.draft = Some(d);
            });
        }
        let draft = ctx.state.read(cx).draft.clone();

        let keys = ctx.clone();
        let mut input = div()
            .id("parameter-slider-input")
            .flex()
            .items_center()
            .justify_end()
            .h_full()
            .w(px(64.0))
            .px_2()
            .font_family(theme.font_mono.clone())
            .text_xs_()
            .text_color(theme.foreground)
            .cursor(CursorStyle::IBeam)
            .role(gpui::Role::SpinButton)
            .aria_value(shown.clone())
            .map(|el| match &draft {
                Some(d) if focused => el.child(d.render(true, theme.foreground)),
                _ => el.child(div().whitespace_nowrap().child(shown.clone())),
            })
            .when(!ctx.disabled, |el| {
                el.track_focus(&focus)
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        cx.stop_propagation();
                        let ks = &e.keystroke;
                        match ks.key.as_str() {
                            "enter" => {
                                let text = keys.state.update(cx, |s, cx| {
                                    cx.notify();
                                    s.draft.take().map(|d| d.text)
                                });
                                if let Some(text) = text {
                                    keys.apply_draft(&text, window, cx);
                                }
                                // The field keeps focus and shows the
                                // formatted value, caret at the end.
                                let shown = format_number(keys.latest(cx), keys.decimals);
                                keys.state.update(cx, |s, _| {
                                    let mut d = TextDraft::new(shown);
                                    d.anchor = d.cursor;
                                    s.draft = Some(d);
                                });
                            }
                            "up" | "down" => {
                                let by = if ks.modifiers.shift {
                                    decimal(keys.large_step)
                                } else if ks.modifiers.alt {
                                    0.1
                                } else {
                                    decimal(keys.step)
                                };
                                let sign = if ks.key == "up" { 1.0 } else { -1.0 };
                                let next = keys.field_step(keys.latest(cx), by * sign);
                                keys.change(next, ParameterChangeReason::Input, window, cx);
                                keys.commit(next, window, cx);
                                // Show the stepped value, selected.
                                let decimals = keys.decimals;
                                keys.state.update(cx, |s, cx| {
                                    let mut d = TextDraft::new(format_number(next, decimals));
                                    d.select_all();
                                    s.draft = Some(d);
                                    cx.notify();
                                });
                            }
                            _ => {
                                keys.state.update(cx, |s, cx| {
                                    if let Some(d) = &mut s.draft
                                        && d.handle_key(ks, cx)
                                    {
                                        cx.notify();
                                    }
                                });
                            }
                        }
                    })
            });
        input.style().refine(&self.style);

        let suffix = ctx.unit.clone().map(|unit| {
            let scrub = self.scrub && !ctx.disabled;
            let down = ctx.clone();
            let span = div()
                .id("parameter-slider-unit")
                .px(px(6.0))
                .text_xs_()
                .text_color(theme.muted_foreground)
                .child(unit);
            if !scrub {
                return span.into_any_element();
            }
            let drag = ctx.clone();
            span.cursor(CursorStyle::ResizeLeftRight)
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
                    let start = (e.position.x.as_f32(), down.latest(cx));
                    down.state.update(cx, |s, cx| {
                        s.scrub = Some(start);
                        cx.notify();
                    });
                })
                .child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            if !scrubbing {
                                return;
                            }
                            let moved = drag.clone();
                            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }
                                let Some((x0, v0)) = moved.state.read(cx).scrub else {
                                    return;
                                };
                                let steps = ((e.position.x.as_f32() - x0) / SCRUB_PIXELS).trunc();
                                let next = moved.field_step(v0, steps as f64 * decimal(moved.step));
                                moved.change(next, ParameterChangeReason::Input, window, cx);
                            });
                            let released = drag.clone();
                            window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                                if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                                    return;
                                }
                                let was = released.state.update(cx, |s, cx| {
                                    cx.notify();
                                    s.scrub.take().is_some()
                                });
                                if was {
                                    released.commit_latest(window, cx);
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_0(),
                )
                .into_any_element()
        });

        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(28.0))
            .rounded(radius_lg(&theme))
            .bg(theme.input.opacity(0.5))
            .when(focused, |el| {
                el.shadow(vec![css_shadow(
                    0.0,
                    0.0,
                    0.0,
                    3.0,
                    theme.ring.opacity(0.3),
                )])
            })
            .child(input)
            .children(suffix)
    }
}

/// The formatted value, read-only, at the width of the widest value.
#[derive(IntoElement, Default)]
pub struct ParameterSliderValue {
    style: StyleRefinement,
}

impl ParameterSliderValue {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for ParameterSliderValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParameterSliderValue {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderValue");
        let theme = cx.theme();
        let ch = ch_width(&theme.font_mono, px(12.0), window);
        let width = ch * ctx.law.widest_value(&*ctx.format) as f32;
        let mut el = div()
            .flex()
            .flex_none()
            .justify_end()
            .min_w(width)
            .font_family(theme.font_mono.clone())
            .text_xs_()
            .whitespace_nowrap()
            .text_color(theme.muted_foreground)
            .child((ctx.format)(ctx.value));
        el.style().refine(&self.style);
        el
    }
}

/// A "Reset" button, hidden while the value equals the reset value.
#[derive(IntoElement, Default)]
pub struct ParameterSliderReset {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ParameterSliderReset {
    pub fn new() -> Self {
        Self::default()
    }
}

styled_parent!(ParameterSliderReset);

impl RenderOnce for ParameterSliderReset {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderReset");
        let theme = cx.theme().clone();
        let modified = ctx.value != ctx.reset_value;
        let focus = ctx.state.read(cx).reset_focus.clone();
        reset_button(
            "parameter-slider-reset",
            &theme,
            ctx.disabled || !modified,
            &focus,
            window,
            self.children,
            self.style,
            move |window, cx| ctx.reset(window, cx),
        )
    }
}

/// The slider: drag, click the track, arrow keys (Shift: `large_step`),
/// Page Up/Down, Home/End; double-click the thumb to reset.
#[derive(IntoElement, Default)]
pub struct ParameterSliderControl {
    aria_label: Option<SharedString>,
    style: StyleRefinement,
}

impl ParameterSliderControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// The thumb's accessible name (React labels it by `ParameterSliderLabel`).
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl Styled for ParameterSliderControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParameterSliderControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderControl");
        let theme = cx.theme().clone();
        let start = ctx.origin_position.min(ctx.position);
        let end = ctx.origin_position.max(ctx.position);
        let (track_cell, dragging, focus) = {
            let s = ctx.state.read(cx);
            (s.track.clone(), s.dragging, s.thumb_focus.clone())
        };
        let focus_visible =
            !ctx.disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let (t, animating) = ctx.state.update(cx, |s, _| {
            s.ring
                .update(!ctx.disabled && (s.hovered || s.dragging || focus_visible))
        });
        if animating {
            window.request_animation_frame();
        }

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
                    moved.drag_to(e.position, window, cx);
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

        let press = ctx.clone();
        let slop = px(HIT_SLOP);
        let hit = div()
            .id("parameter-slider-hit")
            .absolute()
            .left_0()
            .right_0()
            .top(-slop)
            .bottom(-slop)
            .when(!ctx.disabled, |el| {
                el.on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    press.start_drag(0.0, window, cx);
                    press.drag_to(e.position, window, cx);
                })
            });

        let range = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(start))
            .w(relative(end - start))
            .rounded_full()
            // Faded as one layer with the thumb (see `disabled_flatten`).
            .map(|el| {
                if ctx.disabled {
                    el.bg(disabled_flatten(theme.primary, theme.background))
                        .opacity(DISABLED_UNFADE)
                } else {
                    el.bg(theme.primary)
                }
            });

        let label = self
            .aria_label
            .clone()
            .or_else(|| ctx.state.read(cx).label.clone());
        let thumb = div()
            .id("parameter-slider-thumb")
            .absolute()
            .left(px(-8.0))
            .top(px(-8.0))
            .size(px(16.0))
            .rounded_full()
            .bg(theme.background)
            .shadow(
                thumb_shadows(&theme, t, focus_visible)
                    .into_iter()
                    .map(|mut shadow| {
                        if ctx.disabled {
                            shadow.color = shadow.color.opacity(0.5);
                        }
                        shadow
                    })
                    .collect(),
            )
            .when(ctx.disabled, |el| el.opacity(DISABLED_UNFADE))
            .role(gpui::Role::Slider)
            .aria_value((ctx.format)(ctx.value))
            .when_some(label, |el, l| el.aria_label(l));
        let thumb = if ctx.disabled {
            thumb
        } else {
            let hover = ctx.state.clone();
            let down = ctx.clone();
            let keys = ctx.clone();
            let position = ctx.position;
            thumb
                .track_focus(&focus)
                .on_hover(move |hovered, _, cx| {
                    hover.update(cx, |s, cx| {
                        s.hovered = *hovered;
                        cx.notify();
                    });
                })
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if e.click_count >= 2 {
                        down.reset(window, cx);
                        return;
                    }
                    let offset = down.state.read(cx).track.get().map_or(0.0, |track| {
                        e.position.x.as_f32()
                            - (track.origin.x.as_f32() + position * track.size.width.as_f32())
                    });
                    down.start_drag(offset, window, cx);
                })
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    if keys.handle_key(e, window, cx) {
                        cx.stop_propagation();
                    }
                })
        };
        let anchor = div()
            .absolute()
            .size_0()
            .left(relative(ctx.position))
            .top(relative(0.5))
            .child(thumb);

        let track = div()
            .relative()
            .h(px(4.0))
            .w_full()
            .flex_grow(1.0)
            .rounded_full()
            .bg(theme.input.opacity(0.9))
            .child(tracker)
            .child(range)
            .child(anchor);

        let control = div()
            .relative()
            .flex()
            .h(px(16.0))
            .w_full()
            .items_center()
            .px_2()
            .child(hit)
            .child(track);

        let mut el = div()
            .relative()
            .flex()
            .w_full()
            .items_center()
            .child(control);
        el.style().refine(&self.style);
        el
    }
}

/// Labels under the slider at each mark (`text-[0.625rem]`).
#[derive(IntoElement, Default)]
pub struct ParameterSliderMarks {
    style: StyleRefinement,
}

impl ParameterSliderMarks {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for ParameterSliderMarks {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParameterSliderMarks {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = param_ctx("ParameterSliderMarks");
        if ctx.marks.is_empty() {
            return div().into_any_element();
        }
        let theme = cx.theme();
        let mut el = div()
            .relative()
            .mx_2()
            .h(px(16.0))
            .text_size(px(10.0))
            .line_height(px(15.0))
            .text_color(theme.muted_foreground)
            .children(ctx.marks.iter().map(|mark| {
                let text = mark
                    .label
                    .clone()
                    .unwrap_or_else(|| (ctx.format)(mark.value).into());
                // `left-[pos] -translate-x-1/2`: centred on a zero-width anchor.
                div()
                    .absolute()
                    .top_0()
                    .left(relative(ctx.law.taper.to_position(mark.value)))
                    .w_0()
                    .flex()
                    .justify_center()
                    .child(div().flex_none().whitespace_nowrap().child(text))
            }));
        el.style().refine(&self.style);
        el.into_any_element()
    }
}

/// Help text under the slider: `text-muted-foreground text-xs`.
#[derive(IntoElement)]
pub struct ParameterSliderDescription {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ParameterSliderDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            style: StyleRefinement::default(),
            children: vec![text.into().into_any_element()],
        }
    }
}

styled_parent!(ParameterSliderDescription);

impl RenderOnce for ParameterSliderDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = param_ctx("ParameterSliderDescription");
        let mut el = div()
            .text_xs_()
            .text_color(cx.theme().muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn law(min: f32, max: f32, step: f32, large_step: f32) -> ParameterLaw {
        ParameterLaw {
            min,
            max,
            step,
            large_step,
            taper: AnyTaper::Linear(linear_taper(min, max)),
        }
    }

    #[test]
    fn parameter_slider_steps_within_its_range() {
        // No default value: the value starts at `min`.
        let law = law(-1000.0, 1000.0, 5.0, 10.0);
        assert_eq!(law.key_target("up", false, -1000.0), Some(-995.0));
        assert_eq!(law.key_target("end", false, -995.0), Some(1000.0));
        assert_eq!(law.key_target("up", false, 1000.0), Some(1000.0));
        assert_eq!(law.key_target("down", true, 0.0), Some(-10.0));
        assert_eq!(law.key_target("pageup", false, 0.0), Some(10.0));
        assert_eq!(law.key_target("home", false, 0.0), Some(-1000.0));
        assert_eq!(law.key_target("x", false, 0.0), None);
    }

    #[test]
    fn parameter_slider_steps_from_the_parents_value() {
        // A controlled parent that rejected a change still says 0: the next
        // step starts from 0 again (latest is re-synced every render).
        let law = law(0.0, 10.0, 1.0, 10.0);
        let rendered = 0.0;
        assert_eq!(law.key_target("up", false, rendered), Some(1.0));
        assert_eq!(law.key_target("up", false, rendered), Some(1.0));
    }

    #[test]
    fn parameter_slider_quantizes_fractional_steps() {
        let law = law(0.0, 1.0, 0.05, 0.1);
        assert_eq!(law.key_target("up", false, 0.6), Some(0.65));
        assert_eq!(law.value_at(0.333), 0.35);
    }

    #[test]
    fn parameter_slider_log_scale_positions() {
        let law = ParameterLaw {
            taper: AnyTaper::Log(log_taper(20.0, 20_000.0)),
            ..law(20.0, 20_000.0, 1.0, 10.0)
        };
        assert!((law.taper.to_position(632.4555) - 0.5).abs() < 1e-4);
        assert_eq!(law.value_at(0.0), 20.0);
        assert_eq!(law.value_at(1.0), 20_000.0);
    }

    #[test]
    fn parameter_slider_formats_like_js() {
        assert_eq!(decimals_of(5.0), 0);
        assert_eq!(decimals_of(0.05), 2);
        assert_eq!(decimals_of(0.1), 1);
        assert_eq!(to_fixed(-995.0, 0), "-995");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.65, 2), "0.65");
        assert_eq!(format_number(-1000.0, 0), "-1,000");
        assert_eq!(format_number(1234567.5, 1), "1,234,567.5");
        assert_eq!(format_number(5.0, 0), "5");
    }

    #[test]
    fn parameter_slider_parses_input() {
        assert_eq!(parse_number("1,000"), Some(1000.0));
        assert_eq!(parse_number("\u{2212}5"), Some(-5.0));
        assert_eq!(parse_number(" 12.5 "), Some(12.5));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("abc"), None);
    }

    #[test]
    fn parameter_slider_value_width() {
        let law = law(-1000.0, 1000.0, 5.0, 50.0);
        let format = |v: f32| format!("{} ms", to_fixed(v, 0));
        assert_eq!(law.widest_value(&format), "-1000 ms".len());
    }
}
