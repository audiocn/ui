//! Port of `components/ui/pan-control.tsx`: left and right balance with a
//! fill from the centre and a centre detent.
//!
//! React builds it on Base UI's `Slider`; the slider behaviour that matters
//! is ported here: pressing the track jumps the thumb there, pressing the
//! thumb drags it from where it was grabbed, arrows step (Shift for
//! `large_step`), PageUp/PageDown take `large_step`, Home/End go to the ends,
//! and keyboard changes commit immediately. Dragging near the centre snaps
//! to it (`detent`), and double-clicking the thumb centres it.
//!
//! For a knob-style pan, use [`Knob`](crate::Knob) with [`format_pan`] and
//! [`parse_pan`] (see the gallery's `pan-control / knob` example).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Refineable, RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled, Window, canvas,
    div, prelude::FluentBuilder, px, relative,
};

use crate::config::{AudioConfig, AudioSize};
use crate::motion::{Duration, Easing, Instant, Transition};
use crate::theme::{ActiveTheme, mix};
use crate::ui::styles::{ring_shadow, shadow_sm};

const PERCENT: f32 = 100.0;
const DETENT_RANGE: f32 = 0.08;
/// `before:-inset-y-1.5`: the control takes the pointer 6px above and below.
const HIT_SLOP: f32 = 6.0;

/// "L30", "C", "R30".
pub fn format_pan(value: f32) -> String {
    let amount = (value.abs() * PERCENT).round() as i64;
    if amount == 0 {
        return "C".into();
    }
    format!("{}{amount}", if value < 0.0 { "L" } else { "R" })
}

/// Reads "L30", "R15", "C" or a number from −100 to 100; the inverse of [`format_pan`].
pub fn parse_pan(text: &str) -> Option<f32> {
    let trimmed = text.trim().replace('\u{2212}', "-");
    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "c" | "center" | "centre") {
        return Some(0.0);
    }
    let side = trimmed
        .chars()
        .next()
        .filter(|c| matches!(c, 'L' | 'l' | 'R' | 'r'))
        .map(|c| c.to_ascii_uppercase());
    let digits = if side.is_some() {
        &trimmed[1..]
    } else {
        &trimmed[..]
    }
    .trim();
    if digits.is_empty() {
        return None;
    }
    let amount = js_number(digits)? / PERCENT as f64;
    Some(if side == Some('L') {
        -amount.abs()
    } else {
        amount
    } as f32)
}

/// JavaScript `Number(text)` for decimal text (no hex, `Infinity` only).
fn js_number(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("inf") || lower.contains("nan") {
        return match text.trim_start_matches(['+', '-']) {
            "Infinity" => Some(if text.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }),
            _ => None,
        };
    }
    text.parse().ok()
}

/// "Center", "25% right", "30% left".
pub fn describe_pan(value: f32) -> String {
    let amount = (value.abs() * PERCENT).round() as i64;
    if amount == 0 {
        return "Center".into();
    }
    format!("{amount}% {}", if value < 0.0 { "left" } else { "right" })
}

// ---------------------------------------------------------------------------
// Slider behaviour (Base UI), free of GPUI so it can be unit-tested.

/// Digits after the decimal point, like Base UI's `getDecimalPrecision`.
fn decimals(value: f32) -> i32 {
    let text = format!("{value}");
    text.split_once('.')
        .map_or(0, |(_, fraction)| fraction.len() as i32)
}

fn to_fixed(value: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    (value * scale).round() / scale
}

/// An f32 as the decimal it was written as.
fn wide(value: f32) -> f64 {
    format!("{value}").parse().unwrap_or(value as f64)
}

/// `roundValueToStep`.
fn round_value_to_step(value: f32, step: f32, min: f32) -> f32 {
    let (step64, min64) = (wide(step), wide(min));
    let nearest = ((value as f64 - min64) / step64).round() * step64 + min64;
    to_fixed(nearest, decimals(step).max(decimals(min))) as f32
}

/// The thumb's `getNewValue`: a step from the rounded value, clamped.
fn new_value(value: f32, increment: f32, direction: f32, min: f32, max: f32) -> f32 {
    let next = wide(value) + wide(increment) * direction as f64;
    let digits = decimals(value).max(decimals(increment)).max(decimals(min));
    (to_fixed(next, digits) as f32).clamp(min, max)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanReason {
    TrackPress,
    Drag,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PanEffect {
    Change(f32),
    Commit(f32),
}

#[derive(Clone, Copy, Debug)]
struct PanSettings {
    step: f32,
    large_step: f32,
    detent: bool,
    controlled: bool,
    disabled: bool,
}

/// The track's extent in window pixels.
#[derive(Clone, Copy, Debug)]
struct Track {
    left: f32,
    width: f32,
}

impl Track {
    /// The value under a pointer x (`getFingerState`).
    fn value_at(&self, x: f32, step: f32) -> f32 {
        let rescaled = if self.width > 0.0 {
            ((x - self.left) / self.width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        round_value_to_step(-1.0 + 2.0 * rescaled, step, -1.0).clamp(-1.0, 1.0)
    }

    fn x_of(&self, value: f32) -> f32 {
        self.left + (value + 1.0) / 2.0 * self.width
    }
}

#[derive(Clone, Copy, Debug)]
struct Press {
    /// Pointer x minus the thumb's centre when pressed on the thumb.
    offset: f32,
    /// `currentInteractionValueRef`: the last applied pointer value.
    applied: Option<f32>,
}

#[derive(Clone, Debug)]
struct PanModel {
    uncontrolled: f32,
    /// The value Base UI sees (the prop, or ours).
    value: f32,
    press: Option<Press>,
}

impl PanModel {
    fn new(default_value: f32) -> Self {
        let value = default_value.clamp(-1.0, 1.0);
        Self {
            uncontrolled: value,
            value,
            press: None,
        }
    }

    /// PanControl's `setValue`.
    fn set(&mut self, s: &PanSettings, next: f32, fx: &mut Vec<PanEffect>) {
        if !s.controlled {
            self.uncontrolled = next;
        }
        self.value = next;
        fx.push(PanEffect::Change(next));
    }

    /// Base UI's `setValue` through PanControl's `onValueChange` (the detent).
    fn apply(
        &mut self,
        s: &PanSettings,
        next: f32,
        reason: PanReason,
        fx: &mut Vec<PanEffect>,
    ) -> bool {
        if next.is_nan() || next == self.value {
            return false;
        }
        let snapped = if s.detent && reason == PanReason::Drag && next.abs() < DETENT_RANGE {
            0.0
        } else {
            next
        };
        self.set(s, snapped, fx);
        true
    }

    /// PanControl's `onValueCommitted` (the detent applies to every commit).
    fn commit(s: &PanSettings, value: f32, fx: &mut Vec<PanEffect>) {
        let value = if s.detent && value.abs() < DETENT_RANGE {
            0.0
        } else {
            value
        };
        fx.push(PanEffect::Commit(value));
    }

    fn key(&mut self, s: &PanSettings, key: &str, shift: bool, fx: &mut Vec<PanEffect>) -> bool {
        if s.disabled {
            return false;
        }
        let increment = if shift { s.large_step } else { s.step };
        let rounded = round_value_to_step(self.value, s.step, -1.0);
        let next = match key {
            "up" | "right" => new_value(rounded, increment, 1.0, -1.0, 1.0),
            "down" | "left" => new_value(rounded, increment, -1.0, -1.0, 1.0),
            "pageup" => new_value(rounded, s.large_step, 1.0, -1.0, 1.0),
            "pagedown" => new_value(rounded, s.large_step, -1.0, -1.0, 1.0),
            "home" => -1.0,
            "end" => 1.0,
            _ => return false,
        };
        if self.apply(s, next, PanReason::Keyboard, fx) {
            Self::commit(s, next, fx);
        }
        true
    }

    /// A press on the control: on the thumb it grabs, elsewhere it jumps.
    fn pointer_down(
        &mut self,
        s: &PanSettings,
        track: Track,
        x: f32,
        thumb_size: f32,
        fx: &mut Vec<PanEffect>,
    ) -> bool {
        if s.disabled {
            return false;
        }
        let thumb_x = track.x_of(self.value);
        let on_thumb = (x - thumb_x).abs() <= thumb_size / 2.0;
        let mut press = Press {
            offset: if on_thumb { x - thumb_x } else { 0.0 },
            applied: None,
        };
        if !on_thumb {
            let value = track.value_at(x, s.step);
            if self.apply(s, value, PanReason::TrackPress, fx) {
                press.applied = Some(value);
            }
        }
        self.press = Some(press);
        true
    }

    fn pointer_move(&mut self, s: &PanSettings, track: Track, x: f32, fx: &mut Vec<PanEffect>) {
        let Some(press) = self.press else {
            return;
        };
        let value = track.value_at(x - press.offset, s.step);
        if self.apply(s, value, PanReason::Drag, fx) {
            self.press = Some(Press {
                applied: Some(value),
                ..press
            });
        }
    }

    fn pointer_up(&mut self, s: &PanSettings, fx: &mut Vec<PanEffect>) -> bool {
        let Some(press) = self.press.take() else {
            return false;
        };
        if let Some(value) = press.applied {
            Self::commit(s, value, fx);
        }
        true
    }

    /// Double-clicking the thumb centres it.
    fn center(&mut self, s: &PanSettings, fx: &mut Vec<PanEffect>) {
        self.set(s, 0.0, fx);
        fx.push(PanEffect::Commit(0.0));
    }
}

// ---------------------------------------------------------------------------
// Component

type ValueHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// Left and right balance with a fill from the centre and a centre detent.
///
/// ```ignore
/// PanControl::new("pan").value(pan).on_value_change(move |v, _, cx| set_pan(v, cx))
/// ```
#[derive(IntoElement)]
pub struct PanControl {
    id: ElementId,
    value: Option<f32>,
    default_value: f32,
    on_value_change: Option<ValueHandler>,
    on_value_committed: Option<ValueHandler>,
    step: f32,
    large_step: f32,
    detent: bool,
    format: Rc<dyn Fn(f32) -> String>,
    size: Option<AudioSize>,
    disabled: Option<bool>,
    config: AudioConfig,
    style: StyleRefinement,
}

impl PanControl {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: 0.0,
            on_value_change: None,
            on_value_committed: None,
            step: 0.05,
            large_step: 0.25,
            detent: true,
            format: Rc::new(format_pan),
            size: None,
            disabled: None,
            config: AudioConfig::default(),
            style: StyleRefinement::default(),
        }
    }

    /// −1 (left) to 1 (right). Controlled: changes only arrive through `on_value_change`.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Default 0.
    pub fn default_value(mut self, value: f32) -> Self {
        self.default_value = value;
        self
    }

    pub fn on_value_change(
        mut self,
        handler: impl Fn(f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(handler));
        self
    }

    pub fn on_value_committed(
        mut self,
        handler: impl Fn(f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_committed = Some(Rc::new(handler));
        self
    }

    /// Default 0.05.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Shift+arrow and PageUp/PageDown. Default 0.25.
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Snap to the centre while dragging near it. Default true.
    pub fn detent(mut self, detent: bool) -> Self {
        self.detent = detent;
        self
    }

    /// The value's text for assistive technology. Default [`format_pan`].
    /// (GPUI has no accessibility tree yet, so it is not shown.)
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Size and disabled from the surrounding strip, under explicit props.
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }
}

impl Styled for PanControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// `--pan-thumb-size` and `--pan-track-size` for each size.
fn pan_sizes(size: AudioSize) -> (f32, f32) {
    match size {
        AudioSize::Sm => (12.0, 3.0),
        AudioSize::Default => (14.0, 4.0),
        AudioSize::Lg => (16.0, 6.0),
    }
}

struct PanShared {
    settings: PanSettings,
    on_value_change: Option<ValueHandler>,
    on_value_committed: Option<ValueHandler>,
}

struct PanState {
    model: PanModel,
    hovered: bool,
    /// The thumb's ring: 0 = `ring-1`, 1 = `ring-4` (150ms box-shadow transition).
    ring: Transition,
    focus: FocusHandle,
    control: Rc<Cell<Option<Bounds<Pixels>>>>,
    thumb_size: f32,
    shared: Option<Rc<PanShared>>,
}

impl PanState {
    fn track(&self) -> Option<Track> {
        let bounds = self.control.get()?;
        Some(Track {
            left: bounds.origin.x.as_f32() + self.thumb_size / 2.0,
            width: bounds.size.width.as_f32() - self.thumb_size,
        })
    }
}

fn run_effects(shared: &PanShared, effects: Vec<PanEffect>, window: &mut Window, cx: &mut App) {
    for effect in effects {
        match effect {
            PanEffect::Change(value) => {
                if let Some(handler) = &shared.on_value_change {
                    handler(value, window, cx);
                }
            }
            PanEffect::Commit(value) => {
                if let Some(handler) = &shared.on_value_committed {
                    handler(value, window, cx);
                }
            }
        }
    }
}

fn act(
    state: &Entity<PanState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut PanState, &PanSettings) -> (bool, Vec<PanEffect>),
) -> bool {
    let Some(shared) = state.read(cx).shared.clone() else {
        return false;
    };
    let (handled, effects) = state.update(cx, |s, cx| {
        let result = f(s, &shared.settings);
        cx.notify();
        result
    });
    run_effects(&shared, effects, window, cx);
    handled
}

/// Ends a press and commits (`handleTouchEnd`).
fn release(state: &Entity<PanState>, window: &mut Window, cx: &mut App) {
    act(state, window, cx, |s, settings| {
        let mut fx = Vec::new();
        let ended = s.model.pointer_up(settings, &mut fx);
        (ended, fx)
    });
}

impl RenderOnce for PanControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let size = self.size.or(self.config.size).unwrap_or_default();
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let (thumb, track_size) = pan_sizes(size);
        let default_value = self.default_value;
        let state = window.use_keyed_state(self.id.clone(), cx, move |_, cx| PanState {
            model: PanModel::new(default_value),
            hovered: false,
            ring: Transition::new(0.0),
            focus: cx.focus_handle(),
            control: Rc::new(Cell::new(None)),
            thumb_size: thumb,
            shared: None,
        });
        let shared = Rc::new(PanShared {
            settings: PanSettings {
                step: self.step,
                large_step: self.large_step,
                detent: self.detent,
                controlled: self.value.is_some(),
                disabled,
            },
            on_value_change: self.on_value_change,
            on_value_committed: self.on_value_committed,
        });
        let controlled = self.value;
        let focus = state.read(cx).focus.clone();
        let focus_visible =
            !disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let now = Instant::now();
        let (value, pressing, ring, animating, control_cell) = state.update(cx, |s, _| {
            let value = controlled.unwrap_or(s.model.uncontrolled);
            s.model.value = value;
            s.thumb_size = thumb;
            s.shared = Some(shared.clone());
            let pressing = s.model.press.is_some();
            let ringed = !disabled && (s.hovered || pressing || focus_visible);
            s.ring.animate_to(
                if ringed { 1.0 } else { 0.0 },
                Duration::from_millis(150),
                Easing::STANDARD,
            );
            (
                value,
                pressing,
                s.ring.value(now),
                s.ring.is_animating(now),
                s.control.clone(),
            )
        });
        if animating {
            window.request_animation_frame();
        }
        // The value's text (`sr-only`); there is no accessibility tree to put it in.
        let _value_text = (self.format)(value);

        let start = value.min(0.0);
        let end = value.max(0.0);
        // `ring-foreground/15` → `ring-ring/30` (`/40` when focus-visible).
        let lit = theme.ring.opacity(if focus_visible { 0.4 } else { 0.3 });
        let ring_color = mix(theme.foreground.opacity(0.15), lit, ring);
        let ring_width = 1.0 + 3.0 * ring;
        let mut shadows = vec![ring_shadow(ring_width, ring_color)];
        shadows.extend(shadow_sm());

        let hover = state.clone();
        let keys = state.clone();
        let down = state.clone();
        let center = state.clone();
        let drag = state.clone();
        let up = state.clone();
        let focus_on_down = focus.clone();

        let thumb_el = div()
            .id("pan-control-thumb")
            .absolute()
            .top(px((track_size - thumb) / 2.0))
            .left(relative((value + 1.0) / 2.0))
            .ml(px(-thumb / 2.0))
            .size(px(thumb))
            .flex_none()
            .rounded_full()
            .bg(theme.background)
            .shadow(shadows)
            .when(!disabled, |el| {
                el.track_focus(&focus)
                    .on_hover(move |hovered, _, cx| {
                        hover.update(cx, |s, cx| {
                            s.hovered = *hovered;
                            cx.notify();
                        });
                    })
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        let key = e.keystroke.key.clone();
                        let shift = e.keystroke.modifiers.shift;
                        let used = act(&keys, window, cx, |s, settings| {
                            let mut fx = Vec::new();
                            let used = s.model.key(settings, &key, shift, &mut fx);
                            (used, fx)
                        });
                        if used {
                            cx.stop_propagation();
                        }
                    })
                    .on_mouse_up(MouseButton::Left, move |e: &MouseUpEvent, window, cx| {
                        if e.click_count == 2 {
                            act(&center, window, cx, |s, settings| {
                                let mut fx = Vec::new();
                                s.model.center(settings, &mut fx);
                                (true, fx)
                            });
                        }
                    })
            });

        let track = div()
            .relative()
            .h(px(track_size))
            .w_full()
            .flex_grow(1.0)
            .rounded_full()
            .bg(theme.input.opacity(0.9))
            .child(
                div()
                    .absolute()
                    .top(px(-track_size))
                    .left(relative(0.5))
                    .ml(px(-0.5))
                    .h(px(track_size * 3.0))
                    .w(px(1.0))
                    .bg(theme.border),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative((start + 1.0) / 2.0))
                    .w(relative((end - start) / 2.0))
                    .rounded_full()
                    .bg(theme.primary),
            )
            .child(thumb_el);

        let control = div()
            .id("pan-control")
            .relative()
            .flex()
            .w_full()
            .h(px(thumb + HIT_SLOP * 2.0))
            .my(px(-HIT_SLOP))
            .items_center()
            .px(px(thumb / 2.0))
            .child(
                canvas(
                    move |bounds, _, _| {
                        // The control proper, without the hit slop.
                        let mut control = bounds;
                        control.origin.y += px(HIT_SLOP);
                        control.size.height -= px(HIT_SLOP * 2.0);
                        control_cell.set(Some(control));
                    },
                    move |_, _, window, _| {
                        if !pressing {
                            return;
                        }
                        let moved = drag.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            act(&moved, window, cx, |s, settings| {
                                let mut fx = Vec::new();
                                if let Some(track) = s.track() {
                                    s.model.pointer_move(
                                        settings,
                                        track,
                                        e.position.x.as_f32(),
                                        &mut fx,
                                    );
                                }
                                (true, fx)
                            });
                        });
                        let released = drag.clone();
                        window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                                return;
                            }
                            release(&released, window, cx);
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(track)
            .when(!disabled, |el| {
                el.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    release(&up, window, cx)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    move |e: &MouseDownEvent, window, cx| {
                        window.focus(&focus_on_down, cx);
                        act(&down, window, cx, |s, settings| {
                            let mut fx = Vec::new();
                            let thumb_size = s.thumb_size;
                            let started = s.track().is_some_and(|track| {
                                s.model.pointer_down(
                                    settings,
                                    track,
                                    e.position.x.as_f32(),
                                    thumb_size,
                                    &mut fx,
                                )
                            });
                            (started, fx)
                        });
                    },
                )
            });

        let mut root = div()
            .id(self.id)
            .relative()
            .flex()
            .w_full()
            .items_center()
            .when(disabled, |el| el.opacity(0.5));
        root.style().refine(&self.style);
        root.child(control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> PanSettings {
        PanSettings {
            step: 0.05,
            large_step: 0.25,
            detent: true,
            controlled: false,
            disabled: false,
        }
    }

    /// A 200px track from x = 0.
    const TRACK: Track = Track {
        left: 0.0,
        width: 200.0,
    };

    fn changes(fx: &[PanEffect]) -> Vec<f32> {
        fx.iter()
            .filter_map(|e| match e {
                PanEffect::Change(v) => Some(*v),
                PanEffect::Commit(_) => None,
            })
            .collect()
    }

    fn commits(fx: &[PanEffect]) -> Vec<f32> {
        fx.iter()
            .filter_map(|e| match e {
                PanEffect::Commit(v) => Some(*v),
                PanEffect::Change(_) => None,
            })
            .collect()
    }

    #[test]
    fn pan_control_formats_pan_positions() {
        assert_eq!(format_pan(0.0), "C");
        assert_eq!(format_pan(-0.3), "L30");
        assert_eq!(format_pan(1.0), "R100");
        assert_eq!(parse_pan("L30"), Some(-0.3));
        assert_eq!(parse_pan("r15"), Some(0.15));
        assert_eq!(parse_pan("C"), Some(0.0));
        assert_eq!(parse_pan("centre"), Some(0.0));
        assert_eq!(parse_pan("-50"), Some(-0.5));
        assert_eq!(parse_pan("\u{2212}50"), Some(-0.5));
        assert_eq!(parse_pan("left"), None);
        assert_eq!(parse_pan("L"), None);
    }

    #[test]
    fn pan_control_describes_its_value() {
        assert_eq!(describe_pan(0.25), "25% right");
        assert_eq!(describe_pan(-0.3), "30% left");
        assert_eq!(describe_pan(0.001), "Center");
    }

    #[test]
    fn pan_control_steps_with_the_keyboard_and_commits() {
        let s = settings();
        let mut m = PanModel::new(0.0);
        let mut fx = Vec::new();
        m.key(&s, "right", false, &mut fx);
        assert_eq!(m.value, 0.05);
        m.key(&s, "up", true, &mut fx);
        assert_eq!(m.value, 0.3);
        m.key(&s, "pagedown", false, &mut fx);
        assert_eq!(m.value, 0.05);
        // Keyboard changes don't snap, but their commits pass the detent.
        assert_eq!(changes(&fx), [0.05, 0.3, 0.05]);
        assert_eq!(commits(&fx), [0.0, 0.3, 0.0]);
        m.key(&s, "home", false, &mut fx);
        assert_eq!(m.value, -1.0);
        m.key(&s, "left", false, &mut fx);
        assert_eq!(m.value, -1.0);
        m.key(&s, "end", false, &mut fx);
        assert_eq!(m.value, 1.0);
    }

    #[test]
    fn pan_control_jumps_to_a_track_press_and_snaps_drags_to_the_centre() {
        let s = settings();
        let mut m = PanModel::new(-0.3);
        let mut fx = Vec::new();
        // Pressing at 52% of the track jumps to 0.05 (no detent on a press).
        assert!(m.pointer_down(&s, TRACK, 105.0, 14.0, &mut fx));
        assert_eq!(m.value, 0.05);
        // Dragging near the centre snaps to it.
        m.pointer_move(&s, TRACK, 106.0, &mut fx);
        assert_eq!(m.value, 0.05);
        m.pointer_move(&s, TRACK, 98.0, &mut fx);
        assert_eq!(m.value, 0.0);
        m.pointer_move(&s, TRACK, 150.0, &mut fx);
        assert_eq!(m.value, 0.5);
        assert!(m.pointer_up(&s, &mut fx));
        assert_eq!(commits(&fx), [0.5]);
    }

    #[test]
    fn pan_control_drags_the_thumb_from_where_it_is_grabbed() {
        let s = settings();
        let mut m = PanModel::new(0.5);
        let mut fx = Vec::new();
        // The thumb's centre is at 150px; grabbing it 4px right doesn't move it.
        m.pointer_down(&s, TRACK, 154.0, 14.0, &mut fx);
        assert!(fx.is_empty());
        m.pointer_move(&s, TRACK, 174.0, &mut fx);
        assert_eq!(m.value, 0.7);
        // Release without a change commits nothing.
        let mut m = PanModel::new(0.5);
        let mut fx = Vec::new();
        m.pointer_down(&s, TRACK, 150.0, 14.0, &mut fx);
        m.pointer_up(&s, &mut fx);
        assert!(fx.is_empty());
    }

    #[test]
    fn pan_control_centres_on_double_click_and_respects_disabled() {
        let s = settings();
        let mut m = PanModel::new(0.4);
        let mut fx = Vec::new();
        m.center(&s, &mut fx);
        assert_eq!(fx, [PanEffect::Change(0.0), PanEffect::Commit(0.0)]);
        let disabled = PanSettings {
            disabled: true,
            ..s
        };
        let mut fx = Vec::new();
        assert!(!m.key(&disabled, "right", false, &mut fx));
        assert!(!m.pointer_down(&disabled, TRACK, 10.0, 14.0, &mut fx));
        assert!(fx.is_empty());
    }

    #[test]
    fn pan_control_stays_put_when_controlled() {
        let s = PanSettings {
            controlled: true,
            ..settings()
        };
        let mut m = PanModel::new(0.0);
        let mut fx = Vec::new();
        m.key(&s, "right", false, &mut fx);
        assert_eq!(m.uncontrolled, 0.0);
        assert_eq!(changes(&fx), [0.05]);
    }

    #[test]
    fn pan_control_rounds_to_its_step() {
        assert_eq!(round_value_to_step(0.333, 0.05, -1.0), 0.35);
        assert_eq!(round_value_to_step(-0.024, 0.05, -1.0), 0.0);
        assert_eq!(new_value(0.1, 0.05, 1.0, -1.0, 1.0), 0.15);
    }
}
