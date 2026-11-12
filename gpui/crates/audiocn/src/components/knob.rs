//! Port of `components/ui/knob.tsx`: a rotary control for dense layouts.
//!
//! React composes the knob from parts that read a shared context:
//!
//! ```tsx
//! <Knob>
//!   <KnobDial>
//!     <KnobTrack />
//!     <KnobRange />
//!     <KnobPointer />
//!   </KnobDial>
//!   <KnobValue />
//!   <KnobLabel>Gain</KnobLabel>
//! </Knob>
//! ```
//!
//! GPUI has no context, so the parts here are plain builders that [`Knob`]
//! renders with its own state: [`Knob::child`] takes a [`KnobDial`],
//! [`KnobValue`] or [`KnobLabel`], and [`KnobDial::child`] takes the SVG-like
//! parts ([`KnobTrack`], [`KnobRange`], [`KnobPointer`], [`KnobScale`],
//! [`KnobCap`], or your own shapes via [`KnobDial::paint`]), painted in order
//! inside the same 100 × 100 view box as the React SVG.
//!
//! **What changed for GPUI.**
//! - CSS variables become builder options: `--knob-size` →
//!   [`Knob::knob_size`], `--knob-cap-metal` / `--knob-cap-shade` /
//!   `--knob-cap-pitch` → [`KnobCap::metal`] / [`KnobCap::shade`] /
//!   [`KnobCap::pitch`]. `stroke-*` classes → `.color(..)` on each part.
//! - Click sounds: GPUI has no audio output, so [`Knob::click_sound`] decides
//!   *when* to click exactly like React and hands a [`ClickVoice`] to a
//!   [`KnobClickPlayer`] (the knob's own, or the app's global one). Without a
//!   player nothing plays and nothing is synthesised.
//! - The cap's SVG grain filter (`feTurbulence`) is drawn as rotated noise
//!   cells; the CSS conic sheen as quantised wedges.
//! - The scale's numbers are turned like React's SVG `rotate()` by drawing
//!   DM Sans's glyph outlines as paths (GPUI rasterises glyphs upright only).
//! - ARIA attributes have no GPUI equivalent and are dropped.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::LazyLock;

use gpui::{
    AnyElement, App, Background, Bounds, CursorStyle, DispatchPhase, ElementId, Entity,
    FocusHandle, Font, FontWeight, Global, Hsla, InteractiveElement, IntoElement, KeyDownEvent,
    Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PathBuilder, Pixels, Point, Refineable, RenderOnce, ScrollWheelEvent, SharedString,
    StyleRefinement, Styled, Subscription, TextRun, Window, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder, px,
};

use crate::audio::click::{ClickPlayOptions, ClickSound, ClickVoice, create_click_sound};
use crate::audio::taper::{AnyTaper, Taper, TaperInput, resolve_taper};
use crate::config::{AudioConfig, AudioSize};
use crate::theme::{ActiveTheme, Theme};
use crate::ui::styles::TextSize;

const VIEWBOX: f32 = 100.0;
const CENTER: f32 = 50.0;
const RADIUS: f32 = 40.0;
const FINE_FACTOR: f32 = 0.1;
const PRECISION: f64 = 1e6;
const HALF_TURN: f32 = 180.0;

/// Points along the range sampled to find the widest value text.
const WIDTH_SAMPLES: usize = 24;
const THOUSAND: f64 = 1000.0;

/// Share of the dial's radius around its centre where the angle is too jumpy to read.
const DEAD_ZONE: f32 = 0.25;

/// Ticks this close to the lit range's ends still count as lit. (React uses
/// 1e-9 on f64; positions here are f32.)
const TICK_EPSILON: f32 = 1e-6;
/// Positions this close to a detent count as on it.
const DETENT_EPSILON: f32 = 1e-6;
/// Shortest gap between clicks, so a fast turn ticks instead of buzzing.
const CLICK_INTERVAL_MS: f64 = 30.0;

/// Radii of the volume dial, in view box units.
mod scale_geometry {
    pub const LABEL: f32 = 47.5;
    pub const LABEL_SIZE: f32 = 4.5;
    pub const MAJOR_INNER: f32 = 34.5;
    pub const MAJOR_OUTER: f32 = 43.5;
    pub const MINOR_INNER: f32 = 35.5;
    pub const MINOR_OUTER: f32 = 40.5;
}

/// Radii of the cap, in view box units: inside a KnobScale by default, or
/// filling the inside of a KnobTrack when mini. The shadow starts at
/// `halo_from` of the halo's radius.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CapGeometry {
    bezel: f32,
    face: f32,
    halo: f32,
    halo_from: f32,
    rim: f32,
}

const CAP_DEFAULT: CapGeometry = CapGeometry {
    bezel: 25.5,
    face: 22.5,
    halo: 34.0,
    halo_from: 0.6,
    rim: 23.3,
};
const CAP_MINI: CapGeometry = CapGeometry {
    bezel: 32.0,
    face: 28.2,
    halo: 36.0,
    halo_from: 0.8,
    rim: 29.2,
};
/// The default cap's indicator dot.
const CAP_DOT_DISTANCE: f32 = 17.0;
const CAP_DOT_RADIUS: f32 = 1.6;
/// The mini cap's engraved indicator line, which reads at small sizes.
const CAP_LINE_INNER: f32 = 8.5;
const CAP_LINE_LIP: f32 = 0.6;
const CAP_LINE_OUTER: f32 = 23.0;
const CAP_LINE_WIDTH: f32 = 3.4;

// ---------------------------------------------------------------------------
// Public types

/// Why the value changed (`KnobChangeReason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnobChangeReason {
    Drag,
    Keyboard,
    Wheel,
    Reset,
    Input,
}

/// Passed to `on_value_change` (`KnobChangeDetails`; the DOM event is dropped).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnobChangeDetails {
    pub reason: KnobChangeReason,
}

/// How dragging turns the knob (`dragDirection`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KnobDragDirection {
    /// Drag up to increase, down to decrease.
    #[default]
    Vertical,
    /// Drag right to increase, left to decrease.
    Horizontal,
    /// Circle clockwise to increase, anticlockwise to decrease.
    Circular,
}

/// The value law (`scale`: `"linear" | "log"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KnobScaleType {
    #[default]
    Linear,
    Log,
}

/// `clickSound?: boolean | ClickSound`.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum KnobClickSound {
    /// `false`: silent.
    #[default]
    Off,
    /// `true`: the knob's own soft tick.
    Tick,
    /// A custom click in place of the tick.
    Sound(ClickSound),
}

impl From<bool> for KnobClickSound {
    fn from(on: bool) -> Self {
        if on { Self::Tick } else { Self::Off }
    }
}

impl From<ClickSound> for KnobClickSound {
    fn from(sound: ClickSound) -> Self {
        Self::Sound(sound)
    }
}

/// The knob's shared tick (`const knobClick = createClickSound()`).
static KNOB_CLICK: LazyLock<ClickSound> = LazyLock::new(|| create_click_sound(Default::default()));

/// Plays knob clicks: the GPUI stand-in for the `AudioContext` a React knob
/// clicks through. Set one globally with `cx.set_global(player)` (like
/// `AudioContextProvider`), or per knob with [`Knob::click_player`].
#[derive(Clone)]
pub struct KnobClickPlayer {
    /// The rate the player wants voices rendered at.
    pub sample_rate: u32,
    play: ClickPlay,
}

impl KnobClickPlayer {
    pub fn new(sample_rate: u32, play: impl Fn(ClickVoice, &mut App) + 'static) -> Self {
        Self {
            sample_rate,
            play: Rc::new(play),
        }
    }
}

impl Global for KnobClickPlayer {}

type ClickPlay = Rc<dyn Fn(ClickVoice, &mut App)>;

/// Formats a value for display (`format`).
pub type KnobFormat = Rc<dyn Fn(f32) -> String>;
/// Reads a typed value (`parse`); `None` rejects the text.
pub type KnobParse = Rc<dyn Fn(&str) -> Option<f32>>;
type ChangeHandler = Rc<dyn Fn(f32, &KnobChangeDetails, &mut Window, &mut App)>;
type CommitHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type CapRadiusHandler = Rc<dyn Fn(Option<f32>, &mut Window, &mut App)>;
type DialPainter = Rc<dyn Fn(&KnobDialFrame, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Pure maths (exported where React exports it)

fn round_value(value: f64) -> f64 {
    (value * PRECISION).round() / PRECISION
}

/// An f32 as the decimal it was written as (`0.1f32` → `0.1`, not
/// `0.10000000149`), so f64 maths on props matches JavaScript's.
fn wide(value: f32) -> f64 {
    if !value.is_finite() {
        return value as f64;
    }
    format!("{value}").parse().unwrap_or(value as f64)
}

/// `roundValue` for a value that went through f32 maths (a taper): f32 noise
/// (~1e-7 relative) is larger than `roundValue`'s 1e-6 on big values, so
/// round to 6 significant digits first.
fn round_tapered(value: f32) -> f32 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let magnitude = 10f64.powi(5 - (value.abs() as f64).log10().floor() as i32);
    let rounded = (value as f64 * magnitude).round() / magnitude;
    round_value(rounded) as f32
}

/// JavaScript's `String(number)` for the default `format`.
pub fn format_knob_number(value: f32) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if value.is_nan() {
        return "NaN".into();
    }
    format!("{value}")
}

/// The first number in the text. "−" counts as a minus and "k" as thousands.
pub fn parse_knob_value(text: &str) -> Option<f32> {
    let normalized = text.replace('\u{2212}', "-");
    let number = first_number(&normalized)?;
    let thousands = has_thousands(&normalized);
    let value: f64 = number.parse().ok()?;
    Some(if thousands { value * THOUSAND } else { value } as f32)
}

/// `/[-+]?(?:\d+\.?\d*|\.\d+)/` — the leftmost match.
fn first_number(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    for start in 0..bytes.len() {
        let mut at = start;
        if matches!(bytes[at], b'-' | b'+') {
            at += 1;
        }
        if at >= bytes.len() {
            continue;
        }
        let whole = digits(at);
        if whole > 0 {
            let mut end = at + whole;
            if bytes.get(end) == Some(&b'.') {
                end += 1;
                end += digits(end);
            }
            return Some(&text[start..end]);
        }
        if bytes[at] == b'.' {
            let fraction = digits(at + 1);
            if fraction > 0 {
                return Some(&text[start..at + 1 + fraction]);
            }
        }
    }
    None
}

/// `/\d\s*k/i`.
fn has_thousands(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().any(|(index, c)| {
        c.is_ascii_digit()
            && chars[index + 1..]
                .iter()
                .find(|c| !c.is_whitespace())
                .is_some_and(|c| c.eq_ignore_ascii_case(&'k'))
    })
}

/// Degrees clockwise from 12 o'clock for a 0..1 position on an `arc` sweep.
pub fn knob_angle_for(position: f32, arc: f32) -> f32 {
    -arc / 2.0 + position * arc
}

/// A point at `angle` (degrees clockwise from 12 o'clock) and `radius` from
/// the view box centre.
fn point_at(angle: f32, radius: f32) -> (f32, f32) {
    let radians = angle.to_radians();
    (
        CENTER + radius * radians.sin(),
        CENTER - radius * radians.cos(),
    )
}

/// The pointer's angle around the dial centre, clockwise from 12 o'clock, or
/// `None` too close to the centre. `x`/`y` are relative to the centre.
fn pointer_angle(x: f32, y: f32, width: f32) -> Option<f32> {
    if x.hypot(y) < (width / 2.0) * DEAD_ZONE {
        return None;
    }
    Some(x.atan2(-y).to_degrees())
}

/// The shortest turn from one angle to another, in -180..180 degrees.
fn turn_between(from: f32, to: f32) -> f32 {
    (to - from + HALF_TURN * 3.0).rem_euclid(HALF_TURN * 2.0) - HALF_TURN
}

/// Whether moving between positions reaches or passes a detent.
fn reaches_detent(detents: &[f32], from: f32, to: f32) -> bool {
    detents.iter().any(|&detent| {
        if from < to {
            detent > from + DETENT_EPSILON && detent <= to + DETENT_EPSILON
        } else {
            detent < from - DETENT_EPSILON && detent >= to - DETENT_EPSILON
        }
    })
}

/// Whether moving between values reaches or passes a multiple of `every`.
fn reaches_multiple(from: f32, to: f32, min: f32, every: f32) -> bool {
    let start = round_value((wide(from) - wide(min)) / wide(every));
    let end = round_value((wide(to) - wide(min)) / wide(every));
    if from < to {
        end.floor() > start.floor()
    } else {
        start.ceil() > end.ceil()
    }
}

/// Characters in the widest value along the range, so the value keeps one width.
fn widest_value(
    format: &dyn Fn(f32) -> String,
    taper: &AnyTaper,
    snap: impl Fn(f32) -> f32,
) -> usize {
    (0..=WIDTH_SAMPLES)
        .map(|index| {
            let sample = snap(taper.to_value(index as f32 / WIDTH_SAMPLES as f32));
            format(sample).encode_utf16().count()
        })
        .max()
        .unwrap_or(0)
}

/// The long ticks of a [`KnobScale`], as positions (the click detents).
fn scale_detents(ticks: u32, major_every: u32) -> Vec<f32> {
    let count = ticks.max(1);
    let major = major_every.max(1);
    (0..=count)
        .step_by(major as usize)
        .map(|index| index as f32 / count as f32)
        .collect()
}

// ---------------------------------------------------------------------------
// The knob's behaviour, free of GPUI so it can be unit-tested.

/// Where clicks land: on detents (a scale's long ticks) or every `largeStep`.
#[derive(Clone, Debug, PartialEq)]
enum KnobClicks {
    Detents(Vec<f32>),
    Every(f32),
}

/// The resolved props the knob's handlers read (`KnobDialContextValue`).
#[derive(Clone, Debug)]
struct KnobSettings {
    min: f32,
    max: f32,
    step: f32,
    large_step: f32,
    fine_step: f32,
    reset: f32,
    arc: f32,
    drag_direction: KnobDragDirection,
    sensitivity: f32,
    taper: AnyTaper,
    controlled: bool,
    allow_wheel: bool,
    disabled: bool,
    /// `None` for a silent knob.
    clicks: Option<KnobClicks>,
}

impl KnobSettings {
    fn quantize(&self, next: f32, increment: f32) -> f32 {
        let (min, max) = (wide(self.min), wide(self.max));
        let increment = wide(increment);
        let stepped = min + ((wide(next) - min) / increment).round() * increment;
        round_value(stepped.clamp(min.min(max), max.max(min))) as f32
    }

    fn clicks_between(&self, from: f32, to: f32) -> bool {
        match &self.clicks {
            None => false,
            Some(KnobClicks::Detents(detents)) => reaches_detent(
                detents,
                self.taper.to_position(from),
                self.taper.to_position(to),
            ),
            Some(KnobClicks::Every(every)) => reaches_multiple(from, to, self.min, *every),
        }
    }

    fn key_target(&self, key: &str, current: f32, increment: f32) -> Option<f32> {
        Some(match key {
            "down" | "left" => current - increment,
            "up" | "right" => current + increment,
            "end" => self.max,
            "home" => self.min,
            "pagedown" => current - self.large_step,
            "pageup" => current + self.large_step,
            _ => return None,
        })
    }

    fn increment_for(&self, alt: bool, shift: bool) -> f32 {
        if alt {
            self.fine_step
        } else if shift {
            self.large_step
        } else {
            self.step
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DragState {
    x: f32,
    y: f32,
    position: f32,
    /// The pointer's angle around the dial, or `None` too close to its centre.
    angle: Option<f32>,
}

/// Something the handlers must do once the state is released.
#[derive(Clone, Debug, PartialEq)]
enum KnobEffect {
    Change(f32, KnobChangeReason),
    Commit(f32),
    Click,
}

/// A pointer event relative to the dial: window position, dial centre, dial width.
#[derive(Clone, Copy, Debug)]
struct DialPointer {
    x: f32,
    y: f32,
    center_x: f32,
    center_y: f32,
    width: f32,
}

impl DialPointer {
    fn angle(&self, settings: &KnobSettings) -> Option<f32> {
        (settings.drag_direction == KnobDragDirection::Circular)
            .then(|| pointer_angle(self.x - self.center_x, self.y - self.center_y, self.width))
            .flatten()
    }
}

/// `useKnobValue` + the dial's handlers.
#[derive(Clone, Debug)]
struct KnobModel {
    uncontrolled: f32,
    /// `latestRef`: the value the next change starts from.
    latest: f32,
    drag: Option<DragState>,
}

impl KnobModel {
    fn new(settings: &KnobSettings, default_value: Option<f32>) -> Self {
        let value = default_value
            .unwrap_or(settings.reset)
            .clamp(settings.min, settings.max);
        Self {
            uncontrolled: value,
            latest: value,
            drag: None,
        }
    }

    fn change(
        &mut self,
        s: &KnobSettings,
        next: f32,
        reason: KnobChangeReason,
        fx: &mut Vec<KnobEffect>,
    ) {
        let previous = self.latest;
        if next == previous {
            return;
        }
        self.latest = next;
        if !s.controlled {
            self.uncontrolled = next;
        }
        fx.push(KnobEffect::Change(next, reason));
        if s.clicks_between(previous, next) {
            fx.push(KnobEffect::Click);
        }
    }

    fn reset(&mut self, s: &KnobSettings, fx: &mut Vec<KnobEffect>) {
        self.change(s, s.reset, KnobChangeReason::Reset, fx);
        fx.push(KnobEffect::Commit(s.reset));
    }

    /// Arrow keys, PageUp/Down, Home/End. Returns whether the key was used.
    fn key(
        &mut self,
        s: &KnobSettings,
        key: &str,
        alt: bool,
        shift: bool,
        fx: &mut Vec<KnobEffect>,
    ) -> bool {
        if s.disabled {
            return false;
        }
        let increment = s.increment_for(alt, shift);
        let Some(next) = s.key_target(key, self.latest, increment) else {
            return false;
        };
        let quantized = s.quantize(next, increment.min(s.step));
        self.change(s, quantized, KnobChangeReason::Keyboard, fx);
        fx.push(KnobEffect::Commit(quantized));
        true
    }

    /// `delta` uses the DOM's sign (negative = wheel up = increase).
    fn wheel(
        &mut self,
        s: &KnobSettings,
        delta: f32,
        fine: bool,
        fx: &mut Vec<KnobEffect>,
    ) -> bool {
        if s.disabled || delta == 0.0 {
            return false;
        }
        let increment = if fine { s.fine_step } else { s.step };
        let direction = if delta < 0.0 { 1.0 } else { -1.0 };
        let next = s.quantize(self.latest + direction * increment, increment);
        self.change(s, next, KnobChangeReason::Wheel, fx);
        fx.push(KnobEffect::Commit(next));
        true
    }

    /// Starts a drag, or resets on Alt+click. Returns whether a drag started.
    fn pointer_down(
        &mut self,
        s: &KnobSettings,
        pointer: DialPointer,
        alt: bool,
        fx: &mut Vec<KnobEffect>,
    ) -> bool {
        if s.disabled {
            return false;
        }
        if alt {
            self.reset(s, fx);
            return false;
        }
        self.drag = Some(DragState {
            angle: pointer.angle(s),
            position: s.taper.to_position(self.latest),
            x: pointer.x,
            y: pointer.y,
        });
        true
    }

    /// The next position from the movement since the last pointer event, so
    /// pressing or releasing Shift mid-drag changes the speed without a jump.
    /// Circular drags add the turn since the last event, so the knob turns
    /// from where it is grabbed and stops at its ends.
    fn pointer_move(
        &mut self,
        s: &KnobSettings,
        pointer: DialPointer,
        shift: bool,
        fx: &mut Vec<KnobEffect>,
    ) {
        let Some(last) = self.drag else {
            return;
        };
        let angle = pointer.angle(s);
        let fine = if shift { FINE_FACTOR } else { 1.0 };
        let next = match s.drag_direction {
            KnobDragDirection::Circular => match (last.angle, angle) {
                (Some(from), Some(to)) => {
                    (last.position + (turn_between(from, to) / s.arc) * fine).clamp(0.0, 1.0)
                }
                _ => last.position,
            },
            direction => {
                let delta = if direction == KnobDragDirection::Vertical {
                    last.y - pointer.y
                } else {
                    pointer.x - last.x
                };
                (last.position + (delta / s.sensitivity) * fine).clamp(0.0, 1.0)
            }
        };
        self.drag = Some(DragState {
            angle,
            position: next,
            x: pointer.x,
            y: pointer.y,
        });
        let increment = if shift { s.fine_step } else { s.step };
        let value = s.quantize(s.taper.to_value(next), increment);
        self.change(s, value, KnobChangeReason::Drag, fx);
    }

    /// Ends a drag and commits. Returns whether a drag was in progress.
    fn pointer_up(&mut self, fx: &mut Vec<KnobEffect>) -> bool {
        if self.drag.take().is_none() {
            return false;
        }
        fx.push(KnobEffect::Commit(self.latest));
        true
    }

    /// Applies a typed value (`KnobValueInput`'s `finish(true)`).
    fn apply_typed(&mut self, s: &KnobSettings, parsed: Option<f32>, fx: &mut Vec<KnobEffect>) {
        let Some(parsed) = parsed.filter(|v| !v.is_nan()) else {
            return;
        };
        let next = s.quantize(parsed.clamp(s.min, s.max), s.fine_step.min(s.step));
        self.change(s, next, KnobChangeReason::Input, fx);
        fx.push(KnobEffect::Commit(next));
    }
}

// ---------------------------------------------------------------------------
// Dial parts

/// The empty arc: `<KnobTrack />` (`stroke-input`, 8 units, round caps).
#[derive(Clone, Debug, Default)]
pub struct KnobTrack {
    color: Option<Hsla>,
}

impl KnobTrack {
    pub fn new() -> Self {
        Self::default()
    }

    /// The stroke colour. Default `theme.input`.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// The lit arc from `origin` to the value: `<KnobRange />` (`stroke-primary`).
#[derive(Clone, Debug, Default)]
pub struct KnobRange {
    color: Option<Hsla>,
}

impl KnobRange {
    pub fn new() -> Self {
        Self::default()
    }

    /// The stroke colour (`className="stroke-meter-ok"`). Default `theme.primary`.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// A flat face with a line pointing at the value: `<KnobPointer />`.
#[derive(Clone, Debug, Default)]
pub struct KnobPointer {
    color: Option<Hsla>,
}

impl KnobPointer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The pointer line's colour. Default `theme.foreground`.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// Tick marks and numbers around the dial, lit from `origin` to the value.
///
/// ```ignore
/// KnobDial::new().child(KnobScale::new().ticks(100)).child(KnobCap::new())
/// ```
#[derive(Clone)]
pub struct KnobScale {
    ticks: u32,
    major_every: u32,
    label_every: u32,
    format: Option<KnobFormat>,
    active_color: Option<Hsla>,
}

impl Default for KnobScale {
    fn default() -> Self {
        Self {
            ticks: 50,
            major_every: 5,
            label_every: 10,
            format: None,
            active_color: None,
        }
    }
}

impl KnobScale {
    pub fn new() -> Self {
        Self::default()
    }

    /// Divisions across the arc; draws `ticks + 1` marks. Default 50.
    pub fn ticks(mut self, ticks: u32) -> Self {
        self.ticks = ticks;
        self
    }

    /// Every nth tick is long. Default 5.
    pub fn major_every(mut self, major_every: u32) -> Self {
        self.major_every = major_every;
        self
    }

    /// Every nth tick is numbered; 0 hides the numbers. Default 10.
    pub fn label_every(mut self, label_every: u32) -> Self {
        self.label_every = label_every;
        self
    }

    /// Text for each number. Default: the knob's `format`.
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// Colour of the lit ticks (`**:data-active:stroke-*`). Default `theme.foreground`.
    pub fn active_color(mut self, color: impl Into<Hsla>) -> Self {
        self.active_color = Some(color.into());
        self
    }
}

/// `KnobCap`'s variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KnobCapVariant {
    /// Sits inside a KnobScale and marks the value with a dot.
    #[default]
    Default,
    /// Fills the inside of a KnobTrack and marks the value with an engraved
    /// line, for small knobs.
    Mini,
}

/// A brushed aluminium cap in a dark bezel, with a marker that turns with the
/// value. The metal is `metal` (white) shaded by `shade` (black).
///
/// ```ignore
/// KnobDial::new().child(KnobTrack::new()).child(KnobRange::new()).child(KnobCap::new().variant(KnobCapVariant::Mini))
/// ```
#[derive(Clone, Debug, Default)]
pub struct KnobCap {
    variant: KnobCapVariant,
    metal: Option<Hsla>,
    shade: Option<Hsla>,
    pitch: Option<f32>,
}

impl KnobCap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn variant(mut self, variant: KnobCapVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `--knob-cap-metal`. Default white.
    pub fn metal(mut self, color: impl Into<Hsla>) -> Self {
        self.metal = Some(color.into());
        self
    }

    /// `--knob-cap-shade`. Default black.
    pub fn shade(mut self, color: impl Into<Hsla>) -> Self {
        self.shade = Some(color.into());
        self
    }

    /// `--knob-cap-pitch`: spacing of the brushed rings, in view box units.
    /// Default 0.3 (`default`) or 0.9 (`mini`).
    pub fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = Some(pitch);
        self
    }

    /// The bezel's radius in view box units, which `DialProvider` reports.
    pub fn radius(&self) -> f32 {
        self.geometry().bezel
    }

    fn geometry(&self) -> CapGeometry {
        match self.variant {
            KnobCapVariant::Default => CAP_DEFAULT,
            KnobCapVariant::Mini => CAP_MINI,
        }
    }
}

/// What a custom dial part paints with: the dial's bounds and live angle.
#[derive(Clone, Copy, Debug)]
pub struct KnobDialFrame {
    pub bounds: Bounds<Pixels>,
    /// The turn shown by the cap and pointer (`--knob-angle`): degrees
    /// clockwise from 12 o'clock.
    pub angle: f32,
    /// 0..1 along the arc; 0.5 in a bare `DialProvider`.
    pub position: f32,
    pub origin_position: f32,
    pub arc: f32,
}

impl KnobDialFrame {
    /// Pixels per view box unit (the view box is 100 × 100).
    pub fn unit(&self) -> f32 {
        self.bounds.size.width.as_f32() / VIEWBOX
    }

    /// A view box point in window pixels.
    pub fn point(&self, x: f32, y: f32) -> Point<Pixels> {
        let unit = self.unit();
        point(
            self.bounds.origin.x + px(x * unit),
            self.bounds.origin.y + px(y * unit),
        )
    }
}

/// One shape inside a [`KnobDial`], painted in order.
#[derive(Clone)]
pub enum KnobDialPart {
    Track(KnobTrack),
    Range(KnobRange),
    Pointer(KnobPointer),
    Scale(KnobScale),
    Cap(KnobCap),
    /// Your own shapes next to the built-in ones.
    Paint(DialPainter),
}

impl From<KnobTrack> for KnobDialPart {
    fn from(part: KnobTrack) -> Self {
        Self::Track(part)
    }
}
impl From<KnobRange> for KnobDialPart {
    fn from(part: KnobRange) -> Self {
        Self::Range(part)
    }
}
impl From<KnobPointer> for KnobDialPart {
    fn from(part: KnobPointer) -> Self {
        Self::Pointer(part)
    }
}
impl From<KnobScale> for KnobDialPart {
    fn from(part: KnobScale) -> Self {
        Self::Scale(part)
    }
}
impl From<KnobCap> for KnobDialPart {
    fn from(part: KnobCap) -> Self {
        Self::Cap(part)
    }
}

/// The turning part of a [`Knob`]: a focusable slider that responds to drag,
/// keyboard, wheel and double-click, sized by the knob's `--knob-size`.
///
/// ```ignore
/// KnobDial::new().child(KnobTrack::new()).child(KnobRange::new()).child(KnobPointer::new())
/// ```
#[derive(Clone, Default)]
pub struct KnobDial {
    parts: Vec<KnobDialPart>,
}

impl KnobDial {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, part: impl Into<KnobDialPart>) -> Self {
        self.parts.push(part.into());
        self
    }

    pub fn children(mut self, parts: impl IntoIterator<Item = impl Into<KnobDialPart>>) -> Self {
        self.parts.extend(parts.into_iter().map(Into::into));
        self
    }

    /// Paints your own shapes at this point in the dial.
    pub fn paint(
        mut self,
        painter: impl Fn(&KnobDialFrame, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.parts.push(KnobDialPart::Paint(Rc::new(painter)));
        self
    }
}

/// The value as text. Double-click it, or press Enter on the dial, to type
/// one; Enter applies it, Escape cancels.
#[derive(Clone)]
pub struct KnobValue {
    editable: bool,
    style: StyleRefinement,
}

impl Default for KnobValue {
    fn default() -> Self {
        Self {
            editable: true,
            style: StyleRefinement::default(),
        }
    }
}

impl KnobValue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Double-click the value, or press Enter on the dial, to type one. Default true.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }
}

impl Styled for KnobValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The knob's name (`text-xs font-medium`). Double-click it to type a value.
#[derive(Clone)]
pub struct KnobLabel {
    text: SharedString,
    sr_only: bool,
    style: StyleRefinement,
}

impl KnobLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            sr_only: false,
            style: StyleRefinement::default(),
        }
    }

    /// `className="sr-only"`: names the knob without showing the text.
    pub fn sr_only(mut self) -> Self {
        self.sr_only = true;
        self
    }
}

impl Styled for KnobLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One child of a [`Knob`].
pub enum KnobPart {
    Dial(KnobDial),
    Value(KnobValue),
    Label(KnobLabel),
    Element(AnyElement),
}

impl From<KnobDial> for KnobPart {
    fn from(part: KnobDial) -> Self {
        Self::Dial(part)
    }
}
impl From<KnobValue> for KnobPart {
    fn from(part: KnobValue) -> Self {
        Self::Value(part)
    }
}
impl From<KnobLabel> for KnobPart {
    fn from(part: KnobLabel) -> Self {
        Self::Label(part)
    }
}
impl From<AnyElement> for KnobPart {
    fn from(element: AnyElement) -> Self {
        Self::Element(element)
    }
}

// ---------------------------------------------------------------------------
// Knob

/// A rotary control for dense layouts. Drag up to increase, Shift for fine
/// control, double-click or Alt+click to reset, arrows / PageUp / PageDown /
/// Home / End on the keyboard, Enter to type a value.
///
/// Without children it renders a dial with a track, range and pointer.
///
/// ```ignore
/// Knob::new("gain")
///     .min(-24.0)
///     .max(24.0)
///     .origin(0.0)
///     .format(|db| format_db(db, FormatDbOptions { decimals: 0, ..Default::default() }))
///     .child(KnobDial::new().child(KnobTrack::new()).child(KnobRange::new()).child(KnobPointer::new()))
///     .child(KnobValue::new())
///     .child(KnobLabel::new("Gain"))
/// ```
#[derive(IntoElement)]
pub struct Knob {
    id: ElementId,
    value: Option<f32>,
    default_value: Option<f32>,
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    min: f32,
    max: f32,
    step: f32,
    large_step: f32,
    fine_step: Option<f32>,
    reset_value: Option<f32>,
    origin: Option<f32>,
    arc: f32,
    drag_direction: KnobDragDirection,
    sensitivity: f32,
    scale: KnobScaleType,
    allow_wheel: bool,
    click_sound: KnobClickSound,
    click_player: Option<KnobClickPlayer>,
    format: Option<KnobFormat>,
    parse: Option<KnobParse>,
    size: Option<AudioSize>,
    knob_size: Option<Pixels>,
    disabled: Option<bool>,
    config: AudioConfig,
    children: Vec<KnobPart>,
    style: StyleRefinement,
}

impl Knob {
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
            fine_step: None,
            reset_value: None,
            origin: None,
            arc: 270.0,
            drag_direction: KnobDragDirection::Vertical,
            sensitivity: 200.0,
            scale: KnobScaleType::Linear,
            allow_wheel: false,
            click_sound: KnobClickSound::Off,
            click_player: None,
            format: None,
            parse: None,
            size: None,
            knob_size: None,
            disabled: None,
            config: AudioConfig::default(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Controlled value: changes are only reported through `on_value_change`.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Initial value when uncontrolled. Default `reset_value` or `min`.
    pub fn default_value(mut self, value: f32) -> Self {
        self.default_value = Some(value);
        self
    }

    pub fn on_value_change(
        mut self,
        handler: impl Fn(f32, &KnobChangeDetails, &mut Window, &mut App) + 'static,
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

    /// Default 10.
    pub fn large_step(mut self, large_step: f32) -> Self {
        self.large_step = large_step;
        self
    }

    /// Shift+drag, Shift+wheel and Alt+arrow. Default `step / 10`.
    pub fn fine_step(mut self, fine_step: f32) -> Self {
        self.fine_step = Some(fine_step);
        self
    }

    /// Double-click or Alt+click restores this. Default `default_value` or `min`.
    pub fn reset_value(mut self, reset_value: f32) -> Self {
        self.reset_value = Some(reset_value);
        self
    }

    /// Where the arc starts; the centre for bipolar knobs. Default `min`.
    pub fn origin(mut self, origin: f32) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Sweep in degrees. Default 270.
    pub fn arc(mut self, arc: f32) -> Self {
        self.arc = arc;
        self
    }

    /// How dragging turns the knob. Default vertical.
    pub fn drag_direction(mut self, direction: KnobDragDirection) -> Self {
        self.drag_direction = direction;
        self
    }

    /// Pixels of vertical or horizontal drag for the full range. Default 200.
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Default linear.
    pub fn scale(mut self, scale: KnobScaleType) -> Self {
        self.scale = scale;
        self
    }

    /// The wheel adjusts the value while focused. Default false.
    pub fn allow_wheel(mut self, allow_wheel: bool) -> Self {
        self.allow_wheel = allow_wheel;
        self
    }

    /// Plays a soft click on each graduation: KnobScale's long ticks, or every
    /// `large_step` without a scale. Pass a `ClickSound` to replace the tick.
    /// Default false. Plays through [`Knob::click_player`] or the global
    /// [`KnobClickPlayer`]; silent without one.
    pub fn click_sound(mut self, click_sound: impl Into<KnobClickSound>) -> Self {
        self.click_sound = click_sound.into();
        self
    }

    /// The player for this knob's clicks, in place of the global one.
    pub fn click_player(mut self, player: KnobClickPlayer) -> Self {
        self.click_player = Some(player);
        self
    }

    /// Default: `String(value)`.
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// Reads a typed value. Default [`parse_knob_value`]: the first number,
    /// with "k" as thousands.
    pub fn parse(mut self, parse: impl Fn(&str) -> Option<f32> + 'static) -> Self {
        self.parse = Some(Rc::new(parse));
        self
    }

    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    /// `--knob-size`: the dial's diameter, overriding `size` (sm 36, default 48, lg 64).
    pub fn knob_size(mut self, size: impl Into<Pixels>) -> Self {
        self.knob_size = Some(size.into());
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

    pub fn child(mut self, part: impl Into<KnobPart>) -> Self {
        self.children.push(part.into());
        self
    }

    pub fn children(mut self, parts: impl IntoIterator<Item = impl Into<KnobPart>>) -> Self {
        self.children.extend(parts.into_iter().map(Into::into));
        self
    }

    /// Any element inside the knob's column.
    pub fn element(mut self, element: impl IntoElement) -> Self {
        self.children
            .push(KnobPart::Element(element.into_any_element()));
        self
    }
}

impl Styled for Knob {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// `--knob-size` for each `size`.
fn knob_size_for(size: AudioSize) -> Pixels {
    match size {
        AudioSize::Sm => px(36.0),
        AudioSize::Default => px(48.0),
        AudioSize::Lg => px(64.0),
    }
}

/// Everything a knob's handlers need, refreshed each render.
struct KnobShared {
    settings: KnobSettings,
    on_value_change: Option<ChangeHandler>,
    on_value_committed: Option<CommitHandler>,
    format: KnobFormat,
    parse: KnobParse,
    sound: Option<ClickSound>,
    player: Option<KnobClickPlayer>,
}

/// The inline editor's text, caret and selection.
#[derive(Clone, Debug, Default, PartialEq)]
struct ValueDraft {
    text: String,
    cursor: usize,
    anchor: usize,
}

impl ValueDraft {
    /// The text, all selected (`node.select()`).
    fn new(text: String) -> Self {
        Self {
            cursor: text.len(),
            anchor: 0,
            text,
        }
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
    fn key(&mut self, keystroke: &Keystroke) -> bool {
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
            "home" | "up" => self.move_to(0, shift),
            "end" | "down" => self.move_to(self.text.len(), shift),
            "a" if command => {
                self.anchor = 0;
                self.cursor = self.text.len();
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
}

struct KnobState {
    model: KnobModel,
    /// A typed value is being entered in KnobValue.
    editing: bool,
    draft: ValueDraft,
    /// `doneRef`: the editor already finished (Enter/Escape, then blur).
    done: bool,
    focus: FocusHandle,
    input_focus: FocusHandle,
    dial_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    shared: Option<Rc<KnobShared>>,
    _blur: Option<Subscription>,
}

impl KnobState {
    fn open_editor(&mut self, window: &mut Window, cx: &mut App) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        if shared.settings.disabled {
            return;
        }
        self.editing = true;
        self.done = false;
        self.draft = ValueDraft::new((shared.format)(self.model.latest));
        window.focus(&self.input_focus, cx);
    }

    /// `finish(apply)`.
    fn finish_editing(&mut self, apply: bool) -> Vec<KnobEffect> {
        let mut fx = Vec::new();
        if self.done {
            return fx;
        }
        self.done = true;
        self.editing = false;
        let Some(shared) = self.shared.clone() else {
            return fx;
        };
        if apply {
            let parsed = (shared.parse)(&self.draft.text);
            self.model.apply_typed(&shared.settings, parsed, &mut fx);
        }
        fx
    }
}

fn run_effects(shared: &KnobShared, effects: Vec<KnobEffect>, window: &mut Window, cx: &mut App) {
    for effect in effects {
        match effect {
            KnobEffect::Change(value, reason) => {
                if let Some(handler) = &shared.on_value_change {
                    handler(value, &KnobChangeDetails { reason }, window, cx);
                }
            }
            KnobEffect::Commit(value) => {
                if let Some(handler) = &shared.on_value_committed {
                    handler(value, window, cx);
                }
            }
            KnobEffect::Click => play_click(shared, cx),
        }
    }
}

/// Plays the click through the knob's player or the global one; nothing
/// (not even synthesis) happens without a player.
fn play_click(shared: &KnobShared, cx: &mut App) {
    let Some(sound) = &shared.sound else {
        return;
    };
    let player = shared
        .player
        .clone()
        .or_else(|| cx.try_global::<KnobClickPlayer>().cloned());
    let Some(player) = player else {
        return;
    };
    let options = ClickPlayOptions {
        when: None,
        min_interval_ms: CLICK_INTERVAL_MS,
    };
    if let Some(voice) = sound.play(options, player.sample_rate) {
        (player.play)(voice, cx);
    }
}

/// Runs a state update that produces effects, then the effects.
fn act(
    state: &Entity<KnobState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut KnobState, &KnobShared, &mut Window, &mut App) -> (bool, Vec<KnobEffect>),
) -> bool {
    let Some(shared) = state.read(cx).shared.clone() else {
        return false;
    };
    let (handled, effects) = state.update(cx, |s, cx| {
        let result = f(s, &shared, window, cx);
        cx.notify();
        result
    });
    run_effects(&shared, effects, window, cx);
    handled
}

impl RenderOnce for Knob {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let size = self.size.or(self.config.size).unwrap_or_default();
        let knob_size = self.knob_size.unwrap_or_else(|| knob_size_for(size));
        let (min, max) = (self.min, self.max);
        let reset = self.reset_value.or(self.default_value).unwrap_or(min);
        let taper = resolve_taper(
            &match self.scale {
                KnobScaleType::Linear => TaperInput::Linear,
                KnobScaleType::Log => TaperInput::Log,
            },
            min,
            max,
        );
        let format: KnobFormat = self
            .format
            .clone()
            .unwrap_or_else(|| Rc::new(format_knob_number));
        let parse: KnobParse = self
            .parse
            .clone()
            .unwrap_or_else(|| Rc::new(parse_knob_value));

        let mut children = self.children;
        if children.is_empty() {
            children.push(KnobPart::Dial(
                KnobDial::new()
                    .child(KnobTrack::new())
                    .child(KnobRange::new())
                    .child(KnobPointer::new()),
            ));
        }

        // Clicks land on KnobScale's long ticks, or every largeStep without one.
        let sound = match &self.click_sound {
            KnobClickSound::Off => None,
            KnobClickSound::Tick => Some(KNOB_CLICK.clone()),
            KnobClickSound::Sound(sound) => Some(sound.clone()),
        };
        let detents = children.iter().find_map(|part| match part {
            KnobPart::Dial(dial) => dial.parts.iter().find_map(|part| match part {
                KnobDialPart::Scale(scale) => Some(scale_detents(scale.ticks, scale.major_every)),
                _ => None,
            }),
            _ => None,
        });
        let clicks = sound.as_ref().map(|_| match detents {
            Some(detents) => KnobClicks::Detents(detents),
            None => KnobClicks::Every(self.large_step),
        });

        let settings = KnobSettings {
            min,
            max,
            step: self.step,
            large_step: self.large_step,
            fine_step: self.fine_step.unwrap_or(self.step * FINE_FACTOR),
            reset,
            arc: self.arc,
            drag_direction: self.drag_direction,
            sensitivity: self.sensitivity,
            taper: taper.clone(),
            controlled: self.value.is_some(),
            allow_wheel: self.allow_wheel,
            disabled,
            clicks,
        };

        let default_value = self.default_value;
        let init_settings = settings.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, move |window, cx| {
            let input_focus = cx.focus_handle();
            // Leaving the editor applies the typed value (`onBlur={() => finish(true)}`).
            let blur = cx.on_blur(&input_focus, window, |s: &mut KnobState, window, cx| {
                if !s.editing {
                    return;
                }
                let effects = s.finish_editing(true);
                cx.notify();
                if let Some(shared) = s.shared.clone() {
                    window.defer(cx, move |window, cx| {
                        run_effects(&shared, effects, window, cx)
                    });
                }
            });
            KnobState {
                model: KnobModel::new(&init_settings, default_value),
                editing: false,
                draft: ValueDraft::default(),
                done: false,
                focus: cx.focus_handle(),
                input_focus,
                dial_bounds: Rc::new(Cell::new(None)),
                shared: None,
                _blur: Some(blur),
            }
        });

        let step = self.step;
        let value_width = widest_value(&*format, &taper, |next| settings.quantize(next, step));
        let shared = Rc::new(KnobShared {
            settings,
            on_value_change: self.on_value_change,
            on_value_committed: self.on_value_committed,
            format: format.clone(),
            parse,
            sound,
            player: self.click_player,
        });
        let controlled = self.value;
        let (value, editing, draft, dragging) = state.update(cx, |s, _| {
            let value = controlled.unwrap_or(s.model.uncontrolled);
            // `useLatest(value)`.
            s.model.latest = value;
            s.shared = Some(shared.clone());
            (value, s.editing, s.draft.clone(), s.model.drag.is_some())
        });

        let position = taper.to_position(value);
        let origin_value = self.origin.unwrap_or(min).clamp(min, max);
        let origin_position = taper.to_position(origin_value);
        let arc = self.arc;

        let ctx = Rc::new(RenderCtx {
            theme,
            state: state.clone(),
            shared,
            value,
            position,
            origin_position,
            arc,
            angle: knob_angle_for(position, arc),
            knob_size,
            disabled,
            dragging,
            editing,
            draft,
            value_width,
        });

        let mut root = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .when(disabled, |el| el.opacity(0.5));
        root.style().refine(&self.style);
        root.children(children.into_iter().filter_map(|part| match part {
            KnobPart::Dial(dial) => Some(render_dial(dial, &ctx, window, cx)),
            KnobPart::Value(value) => Some(render_value(value, &ctx, window, cx)),
            KnobPart::Label(label) => render_label(label, &ctx),
            KnobPart::Element(element) => Some(element),
        }))
    }
}

/// What every part renders from (`KnobContextValue`).
struct RenderCtx {
    theme: Theme,
    state: Entity<KnobState>,
    shared: Rc<KnobShared>,
    value: f32,
    position: f32,
    origin_position: f32,
    arc: f32,
    angle: f32,
    knob_size: Pixels,
    disabled: bool,
    dragging: bool,
    editing: bool,
    draft: ValueDraft,
    /// Characters in the widest value, for a steady value label.
    value_width: usize,
}

fn dial_pointer(position: Point<Pixels>, bounds: Bounds<Pixels>) -> DialPointer {
    let center = bounds.center();
    DialPointer {
        x: position.x.as_f32(),
        y: position.y.as_f32(),
        center_x: center.x.as_f32(),
        center_y: center.y.as_f32(),
        width: bounds.size.width.as_f32(),
    }
}

fn render_dial(
    dial: KnobDial,
    ctx: &Rc<RenderCtx>,
    _window: &mut Window,
    _cx: &mut App,
) -> AnyElement {
    let state = ctx.state.clone();
    let (focus, bounds_cell) = {
        let s = state.read(_cx);
        (s.focus.clone(), s.dial_bounds.clone())
    };
    let disabled = ctx.disabled;
    let dragging = ctx.dragging;
    let direction = ctx.shared.settings.drag_direction;
    let drag_cursor = match direction {
        KnobDragDirection::Vertical => CursorStyle::ResizeUpDown,
        KnobDragDirection::Horizontal => CursorStyle::ResizeLeftRight,
        KnobDragDirection::Circular => CursorStyle::ClosedHand,
    };
    let cursor = if disabled {
        CursorStyle::Arrow
    } else if dragging {
        drag_cursor
    } else {
        CursorStyle::OpenHand
    };
    let paint = PaintCtx {
        theme: ctx.theme.clone(),
        format: ctx.shared.format.clone(),
        taper: ctx.shared.settings.taper.clone(),
        position: ctx.position,
        origin_position: ctx.origin_position,
        arc: ctx.arc,
        angle: ctx.angle,
    };
    let parts = dial.parts;
    let prepaint_cell = bounds_cell.clone();
    let drag_state = state.clone();

    let el = div()
        .id("knob-dial")
        .relative()
        .flex_none()
        .size(ctx.knob_size)
        .rounded_full()
        .cursor(cursor)
        .child(
            canvas(
                move |bounds, _, _| prepaint_cell.set(Some(bounds)),
                move |bounds, _, window, cx| {
                    paint_dial_parts(&parts, &paint, bounds, window, cx);
                    if !dragging {
                        return;
                    }
                    window.set_window_cursor_style(drag_cursor);
                    // Pointer capture: follow the pointer anywhere while dragging.
                    let moved = drag_state.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        act(&moved, window, cx, |s, shared, _, _| {
                            let mut fx = Vec::new();
                            if let Some(bounds) = s.dial_bounds.get() {
                                let pointer = dial_pointer(e.position, bounds);
                                s.model.pointer_move(
                                    &shared.settings,
                                    pointer,
                                    e.modifiers.shift,
                                    &mut fx,
                                );
                            }
                            (true, fx)
                        });
                    });
                    let released = drag_state.clone();
                    window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                            return;
                        }
                        release_dial(&released, e.click_count, window, cx);
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    if disabled {
        return el.into_any_element();
    }

    let down = state.clone();
    let keys = state.clone();
    let wheel = state.clone();
    let wheel_focus = focus.clone();
    let focus_on_down = focus.clone();
    let up = state.clone();
    el.track_focus(&focus)
        // Also on the dial itself, for a release before the drag's first frame.
        .on_mouse_up(MouseButton::Left, move |e: &MouseUpEvent, window, cx| {
            release_dial(&up, e.click_count, window, cx);
        })
        .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
            let Some(bounds) = bounds_cell.get() else {
                return;
            };
            let pointer = dial_pointer(e.position, bounds);
            // Only the circle (`r = 50`) takes the pointer, not the corners.
            let (dx, dy) = (pointer.x - pointer.center_x, pointer.y - pointer.center_y);
            if dx.hypot(dy) > pointer.width / 2.0 {
                return;
            }
            cx.stop_propagation();
            if !e.modifiers.alt {
                window.focus(&focus_on_down, cx);
            }
            act(&down, window, cx, |s, shared, _, _| {
                let mut fx = Vec::new();
                let started =
                    s.model
                        .pointer_down(&shared.settings, pointer, e.modifiers.alt, &mut fx);
                (started, fx)
            });
        })
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let keystroke = &e.keystroke;
            if keystroke.key == "enter" {
                cx.stop_propagation();
                keys.update(cx, |s, cx| {
                    s.open_editor(window, cx);
                    cx.notify();
                });
                return;
            }
            let handled = act(&keys, window, cx, |s, shared, _, _| {
                let mut fx = Vec::new();
                let m = &keystroke.modifiers;
                let used = s
                    .model
                    .key(&shared.settings, &keystroke.key, m.alt, m.shift, &mut fx);
                (used, fx)
            });
            if handled {
                cx.stop_propagation();
            }
        })
        .when(ctx.shared.settings.allow_wheel, |el| {
            el.on_scroll_wheel(move |e: &ScrollWheelEvent, window, cx| {
                if !wheel_focus.is_focused(window) {
                    return;
                }
                let delta = e.delta.pixel_delta(px(16.0));
                // Shift turns the wheel sideways on some systems.
                let gpui_delta = if delta.y.as_f32() != 0.0 {
                    delta.y
                } else {
                    delta.x
                }
                .as_f32();
                // GPUI's wheel delta is the DOM's negated: up is positive.
                let fine = e.modifiers.shift || e.modifiers.alt;
                let used = act(&wheel, window, cx, |s, shared, _, _| {
                    let mut fx = Vec::new();
                    let used = s.model.wheel(&shared.settings, -gpui_delta, fine, &mut fx);
                    (used, fx)
                });
                if used {
                    cx.stop_propagation();
                }
            })
        })
        .into_any_element()
}

/// Ends a drag (`endDrag`), then resets on a double-click (`onDoubleClick`
/// fires after the second release).
fn release_dial(state: &Entity<KnobState>, click_count: usize, window: &mut Window, cx: &mut App) {
    act(state, window, cx, |s, shared, _, _| {
        let mut fx = Vec::new();
        let ended = s.model.pointer_up(&mut fx);
        if ended && click_count >= 2 && !shared.settings.disabled {
            s.model.reset(&shared.settings, &mut fx);
        }
        (ended, fx)
    });
}

fn render_value(
    part: KnobValue,
    ctx: &Rc<RenderCtx>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = &ctx.theme;
    let ch = ch_width(&theme.font_mono, px(12.0), window);
    let width = ch * ctx.value_width as f32;
    if part.editable && ctx.editing {
        return render_value_input(part, ctx, width, window, cx);
    }
    let editable = part.editable && !ctx.disabled;
    let state = ctx.state.clone();
    let mut el = div()
        .id("knob-value")
        .min_w(width)
        .flex()
        .justify_center()
        .font_family(theme.font_mono.clone())
        .text_xs_()
        .whitespace_nowrap()
        .text_color(theme.muted_foreground)
        .when(editable, |el| el.cursor(CursorStyle::IBeam))
        .child((ctx.shared.format)(ctx.value));
    el.style().refine(&part.style);
    el.when(editable, |el| {
        el.on_mouse_up(MouseButton::Left, move |e: &MouseUpEvent, window, cx| {
            if e.click_count == 2 {
                state.update(cx, |s, cx| {
                    s.open_editor(window, cx);
                    cx.notify();
                });
            }
        })
    })
    .into_any_element()
}

/// The inline editor KnobValue shows while a value is typed.
fn render_value_input(
    part: KnobValue,
    ctx: &Rc<RenderCtx>,
    width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = &ctx.theme;
    let state = ctx.state.clone();
    let (input_focus, focus) = {
        let s = state.read(cx);
        (s.input_focus.clone(), s.focus.clone())
    };
    let focused = input_focus.is_focused(window);
    // `ring-1 ring-ring/50`, `focus-visible:ring-2 ring-foreground` (inputs
    // are focus-visible whenever focused).
    let (ring_width, ring_color) = if focused {
        (2.0, theme.foreground)
    } else {
        (1.0, theme.ring.opacity(0.5))
    };
    let draft = ctx.draft.clone();
    let (start, end) = draft.selection();
    let caret_at_end = draft.cursor == end;
    let caret = || {
        div()
            .flex_none()
            .w(px(1.0))
            .h(px(14.0))
            .bg(theme.foreground)
    };
    let highlight = gpui::hsla(212.0 / 360.0, 1.0, 0.6, 0.35);
    let text = |s: &str| div().flex_none().child(s.to_string());
    let mut el = div()
        .id("knob-value-input")
        .track_focus(&input_focus)
        .h(px(16.0))
        .w(width)
        .min_w_0()
        .rounded(crate::ui::styles::radius_sm(theme))
        .bg(theme.background)
        .shadow(vec![crate::ui::styles::ring_shadow(ring_width, ring_color)])
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .font_family(theme.font_mono.clone())
        .text_xs_()
        .text_color(theme.foreground)
        .whitespace_nowrap()
        .cursor(CursorStyle::IBeam)
        .child(text(&draft.text[..start]))
        .when(focused && (start == end || !caret_at_end), |el| {
            el.child(caret())
        })
        .when(start != end, |el| {
            el.child(text(&draft.text[start..end]).when(focused, |el| el.bg(highlight)))
        })
        .when(focused && start != end && caret_at_end, |el| {
            el.child(caret())
        })
        .child(text(&draft.text[end..]));
    el.style().refine(&part.style);
    el.on_key_down(move |e: &KeyDownEvent, window, cx| {
        cx.stop_propagation();
        let key = e.keystroke.key.as_str();
        if key == "enter" || key == "escape" {
            let apply = key == "enter";
            let Some(shared) = state.read(cx).shared.clone() else {
                return;
            };
            let effects = state.update(cx, |s, cx| {
                cx.notify();
                s.finish_editing(apply)
            });
            window.focus(&focus, cx);
            run_effects(&shared, effects, window, cx);
            return;
        }
        state.update(cx, |s, cx| {
            if s.draft.key(&e.keystroke) {
                cx.notify();
            }
        });
    })
    .into_any_element()
}

fn render_label(part: KnobLabel, ctx: &Rc<RenderCtx>) -> Option<AnyElement> {
    if part.sr_only {
        return None;
    }
    let disabled = ctx.disabled;
    let state = ctx.state.clone();
    let mut el = div()
        .id(SharedString::from(format!("knob-label-{}", part.text)))
        .text_xs_()
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(part.text.clone());
    el.style().refine(&part.style);
    Some(
        el.when(!disabled, |el| {
            el.on_mouse_up(MouseButton::Left, move |e: &MouseUpEvent, window, cx| {
                if e.click_count == 2 {
                    state.update(cx, |s, cx| {
                        s.open_editor(window, cx);
                        cx.notify();
                    });
                }
            })
        })
        .into_any_element(),
    )
}

/// Width of "0" (the CSS `ch` unit) in a font at a size.
fn ch_width(family: &SharedString, size: Pixels, window: &Window) -> Pixels {
    let text_system = window.text_system();
    let id = text_system.resolve_font(&gpui::font(family.clone()));
    text_system.ch_advance(id, size).unwrap_or(size * 0.6)
}

// ---------------------------------------------------------------------------
// DialProvider

/// One shape inside a [`DialProvider`]: the parts that only need an angle.
#[derive(Clone)]
pub enum DialPart {
    Cap(KnobCap),
    Pointer(KnobPointer),
    Paint(DialPainter),
}

impl From<KnobCap> for DialPart {
    fn from(part: KnobCap) -> Self {
        Self::Cap(part)
    }
}
impl From<KnobPointer> for DialPart {
    fn from(part: KnobPointer) -> Self {
        Self::Pointer(part)
    }
}

/// Lets KnobCap and KnobPointer turn inside any dial, not only a Knob. A dial
/// that hugs its cap sizes its drag area from `on_cap_radius_change`. Fills
/// its parent by default (the 100 × 100 view box is scaled to its width).
///
/// ```ignore
/// DialProvider::new("tempo-dial", angle).size(px(96.)).child(KnobCap::new())
/// ```
#[derive(IntoElement)]
pub struct DialProvider {
    id: ElementId,
    angle: f32,
    on_cap_radius_change: Option<CapRadiusHandler>,
    parts: Vec<DialPart>,
    style: StyleRefinement,
}

impl DialProvider {
    /// `angle`: the turn shown by the cap and pointer, degrees clockwise from 12 o'clock.
    pub fn new(id: impl Into<ElementId>, angle: f32) -> Self {
        Self {
            id: id.into(),
            angle,
            on_cap_radius_change: None,
            parts: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// The cap inside reports its outer radius (view box units), or `None`
    /// when it goes away. Reported once per change, not every render.
    pub fn on_cap_radius_change(
        mut self,
        handler: impl Fn(Option<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cap_radius_change = Some(Rc::new(handler));
        self
    }

    pub fn child(mut self, part: impl Into<DialPart>) -> Self {
        self.parts.push(part.into());
        self
    }

    /// Paints your own shapes at this point.
    pub fn paint(
        mut self,
        painter: impl Fn(&KnobDialFrame, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.parts.push(DialPart::Paint(Rc::new(painter)));
        self
    }
}

impl Styled for DialProvider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialProvider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let radius = self.parts.iter().find_map(|part| match part {
            DialPart::Cap(cap) => Some(cap.radius()),
            _ => None,
        });
        // Last reported radius: `Some(None)` = reported "no cap".
        let reported = window.use_keyed_state(self.id.clone(), cx, |_, _| None::<Option<f32>>);
        let previous = *reported.read(cx);
        let changed = match previous {
            None => radius.is_some(),
            Some(last) => last != radius,
        };
        if changed {
            reported.update(cx, |r, _| *r = Some(radius));
            if let Some(handler) = self.on_cap_radius_change.clone() {
                window.defer(cx, move |window, cx| handler(radius, window, cx));
            }
        }
        let paint = PaintCtx {
            theme: cx.theme().clone(),
            format: Rc::new(format_knob_number),
            taper: resolve_taper(&TaperInput::Linear, 0.0, 1.0),
            position: 0.5,
            origin_position: 0.0,
            arc: 270.0,
            angle: self.angle,
        };
        let parts: Vec<KnobDialPart> = self
            .parts
            .into_iter()
            .map(|part| match part {
                DialPart::Cap(cap) => KnobDialPart::Cap(cap),
                DialPart::Pointer(pointer) => KnobDialPart::Pointer(pointer),
                DialPart::Paint(painter) => KnobDialPart::Paint(painter),
            })
            .collect();
        let mut el = div().id(self.id).relative().size_full();
        el.style().refine(&self.style);
        el.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    // A square view box, centred in the bounds.
                    let side = bounds.size.width.min(bounds.size.height);
                    let square = Bounds::centered_at(bounds.center(), gpui::size(side, side));
                    paint_dial_parts(&parts, &paint, square, window, cx);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

// ---------------------------------------------------------------------------
// Painting

/// What the parts paint from.
struct PaintCtx {
    theme: Theme,
    format: KnobFormat,
    taper: AnyTaper,
    position: f32,
    origin_position: f32,
    arc: f32,
    angle: f32,
}

/// The view box mapped onto the dial's bounds.
#[derive(Clone, Copy, Debug)]
struct View {
    x: f32,
    y: f32,
    unit: f32,
}

impl View {
    fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
            unit: bounds.size.width.as_f32() / VIEWBOX,
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Point<Pixels> {
        point(px(self.x + x * self.unit), px(self.y + y * self.unit))
    }

    /// Segments for a smooth circle of `radius` view box units.
    fn segments(&self, radius: f32) -> usize {
        ((std::f32::consts::TAU * radius * self.unit / 1.5) as usize).clamp(24, 360)
    }
}

/// Fills polygons (in view box units) as one path, even-odd.
fn fill_polygons(
    view: View,
    polygons: &[Vec<(f32, f32)>],
    color: impl Into<Background>,
    window: &mut Window,
) {
    if polygons.is_empty() {
        return;
    }
    let mut builder = PathBuilder::fill();
    for polygon in polygons {
        let points: Vec<Point<Pixels>> = polygon.iter().map(|&p| view.at(p)).collect();
        builder.add_polygon(&points, true);
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn circle(cx: f32, cy: f32, radius: f32, segments: usize) -> Vec<(f32, f32)> {
    (0..segments)
        .map(|index| {
            let a = index as f32 / segments as f32 * std::f32::consts::TAU;
            (cx + radius * a.cos(), cy + radius * a.sin())
        })
        .collect()
}

fn disk(view: View, radius: f32, color: impl Into<Background>, window: &mut Window) {
    let n = view.segments(radius);
    fill_polygons(view, &[circle(CENTER, CENTER, radius, n)], color, window);
}

/// An SVG circle stroke: a ring from `inner` to `outer`.
fn ring(view: View, inner: f32, outer: f32, color: impl Into<Background>, window: &mut Window) {
    let n = view.segments(outer);
    fill_polygons(
        view,
        &[
            circle(CENTER, CENTER, outer, n),
            circle(CENTER, CENTER, inner.max(0.0), n),
        ],
        color,
        window,
    );
}

/// `stroke-linecap="round"`: a semicircle around `c` from `+u` through `+t` to `-u`.
fn round_cap(points: &mut Vec<(f32, f32)>, c: (f32, f32), u: (f32, f32), t: (f32, f32), half: f32) {
    const STEPS: usize = 12;
    for index in 1..STEPS {
        let phi = index as f32 / STEPS as f32 * std::f32::consts::PI;
        let (s, co) = phi.sin_cos();
        points.push((
            c.0 + half * (u.0 * co + t.0 * s),
            c.1 + half * (u.1 * co + t.1 * s),
        ));
    }
}

/// An SVG arc path (`arcPath`) stroked `width` wide with round caps, as one
/// polygon so translucent strokes don't overlap themselves.
fn arc_stroke(from: f32, to: f32, radius: f32, width: f32, view: View) -> Option<Vec<(f32, f32)>> {
    let (start, end) = (from.min(to), from.max(to));
    if end - start < 0.01 {
        return None;
    }
    let half = width / 2.0;
    let steps = ((end - start) / 360.0 * view.segments(radius) as f32)
        .ceil()
        .max(2.0) as usize;
    let mut points = Vec::with_capacity(steps * 2 + 24);
    let along =
        |r: f32, index: usize| point_at(start + (end - start) * index as f32 / steps as f32, r);
    for index in 0..=steps {
        points.push(along(radius + half, index));
    }
    let radial = |a: f32| {
        let r = a.to_radians();
        ((r.sin(), -r.cos()), (r.cos(), r.sin()))
    };
    let (u, t) = radial(end);
    round_cap(&mut points, point_at(end, radius), u, t, half);
    for index in (0..=steps).rev() {
        points.push(along(radius - half, index));
    }
    let (u, t) = radial(start);
    round_cap(
        &mut points,
        point_at(start, radius),
        (-u.0, -u.1),
        (-t.0, -t.1),
        half,
    );
    Some(points)
}

/// An SVG line stroked `width` wide, with round or butt caps.
fn line_stroke(a: (f32, f32), b: (f32, f32), width: f32, round: bool) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy).max(1e-6);
    let d = (dx / length, dy / length);
    let n = (-d.1, d.0);
    let half = width / 2.0;
    let offset = |p: (f32, f32), s: f32| (p.0 + n.0 * half * s, p.1 + n.1 * half * s);
    let mut points = vec![offset(a, 1.0), offset(b, 1.0)];
    if round {
        round_cap(&mut points, b, n, d, half);
    }
    points.push(offset(b, -1.0));
    points.push(offset(a, -1.0));
    if round {
        round_cap(&mut points, a, (-n.0, -n.1), (-d.0, -d.1), half);
    }
    points
}

fn paint_dial_parts(
    parts: &[KnobDialPart],
    ctx: &PaintCtx,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let view = View::new(bounds);
    for part in parts {
        match part {
            KnobDialPart::Track(track) => {
                let color = track.color.unwrap_or(ctx.theme.input);
                if let Some(points) = arc_stroke(-ctx.arc / 2.0, ctx.arc / 2.0, RADIUS, 8.0, view) {
                    fill_polygons(view, &[points], color, window);
                }
            }
            KnobDialPart::Range(range) => {
                let color = range.color.unwrap_or(ctx.theme.primary);
                let from = knob_angle_for(ctx.origin_position, ctx.arc);
                let to = knob_angle_for(ctx.position, ctx.arc);
                if let Some(points) = arc_stroke(from, to, RADIUS, 8.0, view) {
                    fill_polygons(view, &[points], color, window);
                }
            }
            KnobDialPart::Pointer(pointer) => paint_pointer(pointer, ctx, view, window),
            KnobDialPart::Scale(scale) => paint_scale(scale, ctx, view, window),
            KnobDialPart::Cap(cap) => paint_cap(cap, ctx.angle, view, window),
            KnobDialPart::Paint(painter) => {
                let frame = KnobDialFrame {
                    bounds,
                    angle: ctx.angle,
                    position: ctx.position,
                    origin_position: ctx.origin_position,
                    arc: ctx.arc,
                };
                painter(&frame, window, cx);
            }
        }
    }
}

fn paint_pointer(pointer: &KnobPointer, ctx: &PaintCtx, view: View, window: &mut Window) {
    let face = RADIUS * 0.8;
    disk(view, face, ctx.theme.muted, window);
    ring(view, face - 0.5, face + 0.5, ctx.theme.border, window);
    let inner = point_at(ctx.angle, RADIUS * 0.3);
    let outer = point_at(ctx.angle, RADIUS * 0.72);
    let color = pointer.color.unwrap_or(ctx.theme.foreground);
    fill_polygons(view, &[line_stroke(inner, outer, 6.0, true)], color, window);
}

fn paint_scale(scale: &KnobScale, ctx: &PaintCtx, view: View, window: &mut Window) {
    use scale_geometry::*;
    let theme = &ctx.theme;
    let lit_from = ctx.origin_position.min(ctx.position) - TICK_EPSILON;
    let lit_to = ctx.origin_position.max(ctx.position) + TICK_EPSILON;
    let count = scale.ticks.max(1);
    // One step for the long ticks drawn and the detents they click on.
    let major_step = scale.major_every.max(1);
    // [minor, major, lit]
    let mut groups: [Vec<Vec<(f32, f32)>>; 3] = Default::default();
    for index in 0..=count {
        let position = index as f32 / count as f32;
        let angle = knob_angle_for(position, ctx.arc);
        let major = index % major_step == 0;
        let (inner, outer) = if major {
            (MAJOR_INNER, MAJOR_OUTER)
        } else {
            (MINOR_INNER, MINOR_OUTER)
        };
        let lit = position >= lit_from && position <= lit_to;
        let width = if major { 1.1 } else { 0.55 };
        let line = line_stroke(point_at(angle, inner), point_at(angle, outer), width, false);
        let group = if lit {
            2
        } else if major {
            1
        } else {
            0
        };
        groups[group].push(line);
    }
    let [minor, major, lit] = groups;
    fill_polygons(view, &minor, theme.muted_foreground.opacity(0.45), window);
    fill_polygons(view, &major, theme.muted_foreground, window);
    fill_polygons(
        view,
        &lit,
        scale.active_color.unwrap_or(theme.foreground),
        window,
    );

    if scale.label_every == 0 {
        return;
    }
    let format = scale.format.clone().unwrap_or_else(|| ctx.format.clone());
    let font = gpui::font(theme.font_sans.clone());
    let font_size = px(LABEL_SIZE * view.unit);
    for index in 0..=count / scale.label_every {
        let position = (index * scale.label_every) as f32 / count as f32;
        let angle = knob_angle_for(position, ctx.arc);
        let text = format(round_tapered(ctx.taper.to_value(position)));
        paint_rotated_label(
            &text,
            point_at(angle, LABEL),
            angle,
            font_size,
            font.clone(),
            theme.muted_foreground,
            view,
            window,
        );
    }
}

/// SVG `<text text-anchor="middle" dominant-baseline="central"
/// transform="rotate(angle x y)">`: the text centred on `center` (view box
/// units) and turned `angle` degrees clockwise about it.
///
/// GPUI rasterises glyphs upright only, so the glyphs are drawn as paths from
/// DM Sans's outlines ([`dm_sans_outlines`]), placed where the text system
/// shapes them. Text in another font, or characters without an outline, fall
/// back to upright glyphs.
#[allow(clippy::too_many_arguments)]
fn paint_rotated_label(
    text: &str,
    center: (f32, f32),
    angle: f32,
    font_size: Pixels,
    font: Font,
    color: Hsla,
    view: View,
    window: &mut Window,
) {
    if text.is_empty() {
        return;
    }
    let outlines = font.family.as_ref() == dm_sans_outlines::FAMILY;
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(
        SharedString::from(text.to_string()),
        font_size,
        &[run],
        None,
    );
    let size = font_size.as_f32();
    let width = line.width.as_f32();
    let scale = size / dm_sans_outlines::UNITS_PER_EM;
    // `central`: the middle of the em box (ascent over descent) sits on `center`.
    let baseline = (dm_sans_outlines::ASCENT + dm_sans_outlines::DESCENT) / 2.0 * scale;
    let origin = view.at(center);
    let (sin, cos) = angle.to_radians().sin_cos();
    // Text space (x along the baseline from the text's centre, y down) to window.
    let place = |x: f32, y: f32| {
        point(
            origin.x + px(x * cos - y * sin),
            origin.y + px(x * sin + y * cos),
        )
    };
    let mut builder = PathBuilder::fill();
    let mut drawn = false;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let x0 = glyph.position.x.as_f32() - width / 2.0;
            let ch = text[glyph.index..].chars().next().unwrap_or(' ');
            match outlines.then(|| dm_sans_outlines::outline(ch)).flatten() {
                Some(outline) => {
                    dm_sans_outlines::trace(outline, &mut builder, |fx, fy| {
                        place(x0 + fx * scale, baseline - fy * scale)
                    });
                    drawn = true;
                }
                None => {
                    // Upright fallback, centred on the glyph's rotated position.
                    let advance =
                        dm_sans_outlines::advance(ch).map_or(size * 0.6, |advance| advance * scale);
                    let mid = place(x0 + advance / 2.0, 0.0);
                    let origin = point(mid.x - px(advance / 2.0), mid.y + px(baseline));
                    window
                        .paint_glyph(origin, run.font_id, glyph.id, font_size, color)
                        .ok();
                }
            }
        }
    }
    if drawn && let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// The conic sheen across the face: `conic-gradient(from 15deg, sheen,
/// transparent 9%, sheen 21%, transparent 32%, sheen-deep 46%, transparent
/// 58%, sheen 70%, transparent 83%, sheen)` as the shade's alpha.
const SHEEN_STOPS: [(f32, f32); 9] = [
    (0.0, 0.34),
    (0.09, 0.0),
    (0.21, 0.34),
    (0.32, 0.0),
    (0.46, 0.5),
    (0.58, 0.0),
    (0.70, 0.34),
    (0.83, 0.0),
    (1.0, 0.34),
];
const SHEEN_FROM: f32 = 15.0;

fn sheen_alpha(t: f32) -> f32 {
    for pair in SHEEN_STOPS.windows(2) {
        let ((t0, a0), (t1, a1)) = (pair[0], pair[1]);
        if t <= t1 {
            return a0 + (a1 - a0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        }
    }
    SHEEN_STOPS[SHEEN_STOPS.len() - 1].1
}

/// Cheap integer hash → 0..1, for the grain.
fn grain_hash(i: i32, j: i32, seed: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(374_761_393)
        ^ (j as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0
}

fn paint_cap(cap: &KnobCap, angle: f32, view: View, window: &mut Window) {
    let g = cap.geometry();
    let metal = cap.metal.unwrap_or_else(gpui::white);
    let shade = cap.shade.unwrap_or_else(gpui::black);

    // Halo: a radial shadow, shade 55% from `halo_from` fading out. GPUI has
    // no radial gradient, so it is a stack of nested disks (no shared edges to
    // leave seams), each adding just enough alpha that the stack reaches the
    // gradient's alpha at its edge, about one per device pixel of the fade.
    let halo_inner = g.halo * g.halo_from;
    let fade = (g.halo - halo_inner) * view.unit * window.scale_factor();
    let steps = (fade.ceil() as usize).clamp(8, 96);
    let alpha_at = |index: usize| 0.55 * index as f32 / steps as f32;
    for index in 1..=steps {
        let radius = g.halo - (g.halo - halo_inner) * (index as f32 - 0.5) / steps as f32;
        let added = 1.0 - (1.0 - alpha_at(index)) / (1.0 - alpha_at(index - 1));
        disk(view, radius, shade.opacity(added), window);
    }

    // Bezel, lit from above.
    disk(view, g.bezel, shade.opacity(0.85), window);
    ring(
        view,
        g.bezel - 0.2,
        g.bezel + 0.2,
        metal.opacity(0.15),
        window,
    );
    disk(
        view,
        g.bezel,
        linear_gradient(
            180.0,
            linear_color_stop(metal.opacity(0.16), 0.0),
            linear_color_stop(metal.opacity(0.0), 1.0),
        ),
        window,
    );
    // Rim, brighter at the bottom.
    disk(view, g.rim, metal, window);
    disk(
        view,
        g.rim,
        linear_gradient(
            180.0,
            linear_color_stop(shade.opacity(0.62), 0.0),
            linear_color_stop(shade.opacity(0.08), 1.0),
        ),
        window,
    );

    // Face: metal, the conic sheen (fixed lighting), then the brushed rings.
    disk(view, g.face, metal, window);
    paint_sheen(view, g.face, shade, window);
    let pitch = cap.pitch.unwrap_or(match cap.variant {
        KnobCapVariant::Default => 0.3,
        KnobCapVariant::Mini => 0.9,
    });
    paint_brush(view, g.face, pitch, shade.opacity(0.07), window);
    paint_grain(view, g.face, angle, window);

    // The indicator.
    match cap.variant {
        KnobCapVariant::Default => {
            let dot = point_at(angle, CAP_DOT_DISTANCE);
            let n = view.segments(CAP_DOT_RADIUS);
            let stroke = 0.35 / 2.0;
            fill_polygons(
                view,
                &[circle(dot.0, dot.1, CAP_DOT_RADIUS, n)],
                shade.opacity(0.85),
                window,
            );
            fill_polygons(
                view,
                &[
                    circle(dot.0, dot.1, CAP_DOT_RADIUS + stroke, n),
                    circle(dot.0, dot.1, CAP_DOT_RADIUS - stroke, n),
                ],
                shade.opacity(0.45),
                window,
            );
        }
        KnobCapVariant::Mini => {
            let inner = point_at(angle, CAP_LINE_INNER);
            let outer = point_at(angle, CAP_LINE_OUTER);
            // Light from above catches the groove's lower wall.
            let lip = |p: (f32, f32)| (p.0, p.1 + CAP_LINE_LIP);
            fill_polygons(
                view,
                &[line_stroke(lip(inner), lip(outer), CAP_LINE_WIDTH, true)],
                metal.opacity(0.7),
                window,
            );
            fill_polygons(
                view,
                &[line_stroke(inner, outer, CAP_LINE_WIDTH, true)],
                shade.opacity(0.8),
                window,
            );
        }
    }
}

fn paint_sheen(view: View, face: f32, shade: Hsla, window: &mut Window) {
    const WEDGES: usize = 360;
    const LEVELS: usize = 64;
    let mut levels: Vec<Vec<Vec<(f32, f32)>>> = vec![Vec::new(); LEVELS];
    let max_alpha = 0.5;
    for index in 0..WEDGES {
        let a0 = index as f32 / WEDGES as f32 * 360.0;
        let a1 = (index + 1) as f32 / WEDGES as f32 * 360.0;
        let t = ((a0 + a1) / 2.0 - SHEEN_FROM).rem_euclid(360.0) / 360.0;
        let alpha = sheen_alpha(t);
        let level = ((alpha / max_alpha) * (LEVELS - 1) as f32).round() as usize;
        if level == 0 {
            continue;
        }
        levels[level.min(LEVELS - 1)].push(vec![
            (CENTER, CENTER),
            point_at(a0, face),
            point_at(a1, face),
        ]);
    }
    for (level, wedges) in levels.iter().enumerate() {
        let alpha = level as f32 / (LEVELS - 1) as f32 * max_alpha;
        fill_polygons(view, wedges, shade.opacity(alpha), window);
    }
}

/// `repeating-radial-gradient(circle, brush 0 pitch, transparent pitch 2*pitch)`.
///
/// A browser samples a gradient once per device pixel with hard stops, so
/// rings a pixel or two apart alias into the fine moiré that gives the cap
/// its brushed look. Those are sampled the same way here; wider rings are
/// drawn as rings.
fn paint_brush(view: View, face: f32, pitch: f32, brush: Hsla, window: &mut Window) {
    if pitch <= 0.0 {
        return;
    }
    let scale = window.scale_factor();
    let period = 2.0 * pitch * view.unit * scale;
    if period < 4.0 {
        let cx = (view.x + CENTER * view.unit) * scale;
        let cy = (view.y + CENTER * view.unit) * scale;
        let radius = face * view.unit * scale;
        // Inside the face's anti-aliased edge.
        let inside = (radius - 0.5).max(0.0);
        let quad = |x0: i32, x1: i32, row: i32| {
            gpui::fill(
                Bounds::new(
                    point(px(x0 as f32 / scale), px(row as f32 / scale)),
                    gpui::size(px((x1 - x0) as f32 / scale), px(1.0 / scale)),
                ),
                brush,
            )
        };
        for row in (cy - radius).floor() as i32..(cy + radius).ceil() as i32 {
            let dy = row as f32 + 0.5 - cy;
            if dy.abs() > inside {
                continue;
            }
            let half = (inside * inside - dy * dy).sqrt();
            let mut run: Option<i32> = None;
            let end = (cx + half).ceil() as i32;
            for col in (cx - half).floor() as i32..=end {
                let dx = col as f32 + 0.5 - cx;
                let distance = dx.hypot(dy);
                let lit = col < end && distance <= inside && (distance / period).fract() < 0.5;
                match (lit, run) {
                    (true, None) => run = Some(col),
                    (false, Some(start)) => {
                        window.paint_quad(quad(start, col, row));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        return;
    }
    let mut rings = Vec::new();
    let n = view.segments(face);
    let mut r = 0.0;
    while r < face {
        let outer = (r + pitch).min(face);
        rings.push(circle(CENTER, CENTER, outer, n));
        if r > 0.0 {
            rings.push(circle(CENTER, CENTER, r, n));
        }
        r += pitch * 2.0;
    }
    fill_polygons(view, &rings, brush, window);
}

/// The SVG grain filter (fractal noise at 25%), turning with the value.
fn paint_grain(view: View, face: f32, angle: f32, window: &mut Window) {
    const LEVELS: usize = 8;
    let cell = 1.1_f32.max(1.2 / view.unit);
    let (sin, cos) = angle.to_radians().sin_cos();
    let rotate = |x: f32, y: f32| (CENTER + x * cos - y * sin, CENTER + x * sin + y * cos);
    let cells = (face / cell).ceil() as i32;
    let mut levels: Vec<Vec<Vec<(f32, f32)>>> = vec![Vec::new(); LEVELS];
    for i in -cells..cells {
        for j in -cells..cells {
            let (x0, y0) = (i as f32 * cell, j as f32 * cell);
            let (cx, cy) = (x0 + cell / 2.0, y0 + cell / 2.0);
            if cx.hypot(cy) + cell * 0.71 > face {
                continue;
            }
            let level = ((grain_hash(i, j, 7) * LEVELS as f32) as usize).min(LEVELS - 1);
            levels[level].push(vec![
                rotate(x0, y0),
                rotate(x0 + cell, y0),
                rotate(x0 + cell, y0 + cell),
                rotate(x0, y0 + cell),
            ]);
        }
    }
    for (level, cells) in levels.iter().enumerate() {
        // `fractalNoise` is ~0.5 ± 0.15 per channel in linear light, so
        // desaturated it is a light grey (~0.73 sRGB) varying by about ±0.1,
        // with ~50% alpha; at the circle's 25% opacity that is a faint film.
        let lightness = 0.63 + 0.2 * level as f32 / (LEVELS - 1) as f32;
        fill_polygons(view, cells, gpui::hsla(0.0, 0.0, lightness, 0.125), window);
    }
}

/// Glyph outlines of DM Sans Regular (the theme's `font_sans`, as bundled in
/// `DMSans-Regular.ttf`) for printable ASCII and a few symbols, so the knob
/// scale can draw its numbers turned like React's SVG `<text>`.
mod dm_sans_outlines {
    use gpui::{PathBuilder, Pixels, Point};

    pub const FAMILY: &str = "DM Sans";
    pub const UNITS_PER_EM: f32 = 1000.0;
    /// `hhea` ascender and descender, in font units.
    pub const ASCENT: f32 = 992.0;
    pub const DESCENT: f32 = -310.0;

    /// `(char, advance, outline)`: the outline is TrueType quadratic contours
    /// in font units (y up) as `M x y`, `L x y`, `Q cx cy x y` and `Z`.
    const GLYPHS: &[(char, u16, &str)] = &[
        (
            '!',
            250,
            "M90 199L80 700L171 700L161 199ZM125 -5Q101 -5 84 11Q68 28 68 52Q68 76 84 92Q101 109 125 109Q151 109 166 92Q182 76 182 52Q182 28 166 11Q151 -5 125 -5Z",
        ),
        (
            '"',
            291,
            "M185 551L173 755L253 755L243 551ZM49 551L38 755L118 755L107 551Z",
        ),
        (
            '#',
            804,
            "M434 0L582 706L661 706L514 0ZM144 0L292 706L372 706L224 0ZM46 197L46 272L714 272L714 197ZM87 435L87 509L757 509L757 435Z",
        ),
        (
            '$',
            580,
            "M255 -87L255 791L324 791L324 -87ZM298 -12Q221 -12 164 16Q107 44 76 94Q45 144 45 210L133 210Q133 169 152 134Q171 100 207 79Q244 59 298 59Q345 59 378 74Q412 90 429 117Q447 144 447 178Q447 219 429 244Q412 270 382 286Q352 302 312 314Q273 327 230 341Q147 369 108 411Q69 453 69 520Q69 577 95 620Q122 663 171 687Q221 712 289 712Q356 712 405 687Q455 662 483 618Q511 575 511 518L423 518Q423 547 408 575Q393 603 362 621Q332 640 286 640Q248 641 218 627Q189 614 172 589Q156 564 156 528Q156 494 170 473Q185 452 212 437Q240 423 277 411Q314 399 359 384Q410 367 449 342Q489 318 511 280Q534 242 534 183Q534 133 507 88Q481 44 429 16Q377 -12 298 -12Z",
        ),
        (
            '%',
            786,
            "M145 0L552 700L636 700L229 0ZM586 -12Q541 -12 505 8Q469 29 447 67Q426 105 426 156Q426 208 447 245Q469 283 505 303Q541 324 586 324Q632 324 667 303Q703 283 724 245Q745 208 745 156Q745 105 724 67Q703 29 667 8Q631 -12 586 -12ZM585 53Q609 53 629 64Q649 76 661 99Q674 122 674 156Q674 191 662 214Q650 237 630 248Q610 260 586 260Q563 260 542 248Q522 237 509 214Q497 191 497 156Q497 122 509 99Q522 76 542 64Q562 53 585 53ZM201 375Q156 375 120 395Q84 416 62 454Q41 492 41 543Q41 595 62 632Q84 670 120 690Q157 711 202 711Q247 711 283 690Q319 670 340 632Q361 595 361 543Q361 492 340 454Q319 416 282 395Q246 375 201 375ZM201 440Q225 440 245 451Q265 463 277 486Q289 509 289 543Q289 578 277 601Q265 625 245 636Q225 648 202 648Q179 648 158 636Q137 625 124 601Q112 578 112 543Q112 509 124 486Q137 463 157 451Q178 440 201 440Z",
        ),
        (
            '&',
            734,
            "M603 0L222 388Q176 434 156 472Q136 510 136 556Q136 601 158 636Q180 672 220 692Q261 712 318 712Q373 712 413 691Q454 671 476 635Q498 599 497 552L414 552Q414 597 385 620Q357 644 318 644Q272 644 245 619Q219 595 219 557Q219 525 237 494Q255 463 289 426L709 0ZM279 -12Q208 -12 154 14Q101 41 71 89Q42 137 42 202Q42 252 62 294Q83 337 124 369Q165 402 224 422L256 433L300 378L262 365Q193 340 159 300Q126 260 126 205Q126 164 145 131Q165 98 200 79Q235 60 281 60Q340 60 385 85Q431 111 478 169Q499 195 521 229Q543 264 568 307L599 362L688 362L643 282Q616 232 589 191Q563 150 536 118Q479 49 417 18Q356 -12 279 -12Z",
        ),
        ('\'', 159, "M51 551L38 755L121 755L109 551Z"),
        (
            '(',
            373,
            "M249 -145Q195 -93 149 -21Q103 50 75 139Q48 228 48 333Q48 438 75 527Q103 616 149 688Q195 760 249 811L335 811L335 803Q274 743 228 669Q183 595 157 510Q132 426 132 333Q132 240 157 155Q183 71 229 -3Q275 -77 335 -137L335 -145Z",
        ),
        (
            ')',
            373,
            "M124 -145L38 -145L38 -137Q99 -77 144 -3Q190 71 215 155Q241 240 241 333Q241 426 215 510Q190 595 144 669Q99 743 38 803L38 811L124 811Q179 760 224 688Q270 616 297 527Q325 438 325 333Q325 228 297 139Q270 50 224 -21Q179 -93 124 -145Z",
        ),
        (
            '*',
            480,
            "M209 360L221 513L97 426L62 487L201 548L63 608L96 667L222 580L208 733L276 733L261 580L386 667L418 609L280 547L419 486L386 427L261 513L275 360Z",
        ),
        (
            '+',
            550,
            "M237 72L237 508L312 508L312 72ZM46 255L46 326L503 326L503 255Z",
        ),
        (',', 182, "M4 -123L62 98L149 98L62 -123Z"),
        ('-', 541, "M72 260L72 331L468 331L468 260Z"),
        (
            '.',
            198,
            "M100 -5Q75 -5 58 11Q42 28 42 52Q42 76 58 92Q75 109 100 109Q125 109 141 92Q157 76 157 52Q157 28 141 11Q125 -5 100 -5Z",
        ),
        ('/', 392, "M28 -95L279 764L362 764L111 -95Z"),
        (
            '0',
            684,
            "M343 -12Q251 -12 184 33Q118 79 83 160Q48 242 48 350Q48 458 83 539Q118 621 184 666Q251 712 343 712Q435 712 500 666Q566 621 601 539Q636 458 636 350Q636 242 601 160Q566 79 500 33Q435 -12 343 -12ZM342 62Q402 62 449 96Q496 131 523 195Q550 260 550 350Q550 440 523 504Q496 569 449 603Q402 637 342 637Q282 637 234 603Q187 569 160 504Q134 440 134 350Q134 260 160 195Q187 131 234 96Q282 62 342 62Z",
        ),
        (
            '1',
            312,
            "M152 0L152 606L32 576L32 636L180 700L237 700L237 0Z",
        ),
        (
            '2',
            576,
            "M64 0L64 59Q135 115 199 172Q264 229 314 285Q364 342 392 396Q421 450 421 500Q421 537 408 568Q395 600 366 620Q338 640 290 640Q243 640 212 619Q181 599 165 565Q150 532 150 492L69 492Q70 561 98 610Q127 660 177 686Q227 712 291 712Q354 712 402 688Q450 665 477 619Q505 573 505 503Q505 454 484 405Q464 356 430 309Q396 263 354 220Q313 177 269 139Q226 101 188 70L529 70L529 0Z",
        ),
        (
            '3',
            591,
            "M293 -12Q226 -12 171 12Q117 37 85 86Q53 135 51 208L136 208Q137 168 155 134Q174 101 209 81Q244 61 293 61Q342 61 375 80Q408 99 425 130Q442 162 442 201Q442 250 418 280Q395 311 355 325Q316 340 269 340L217 340L217 411L269 411Q338 411 377 443Q416 475 416 531Q416 577 385 608Q355 639 293 639Q234 639 199 605Q164 571 159 518L74 518Q77 575 105 619Q133 663 181 687Q229 712 294 712Q362 712 408 688Q454 664 477 624Q501 584 501 536Q501 500 488 468Q475 437 449 413Q423 389 383 379Q424 371 456 348Q489 325 508 287Q527 250 527 201Q527 144 500 95Q474 47 422 17Q370 -12 293 -12Z",
        ),
        (
            '4',
            607,
            "M392 0L392 150L37 150L37 215L377 700L475 700L475 224L579 224L579 150L475 150L475 0ZM130 224L396 224L396 607L396 607Z",
        ),
        (
            '5',
            610,
            "M311 -12Q239 -12 186 14Q134 40 103 85Q73 130 65 186L147 186Q159 130 202 95Q245 60 312 60Q362 60 397 83Q433 106 452 146Q472 186 472 235Q472 286 452 325Q432 364 397 385Q362 407 315 407Q258 407 218 382Q178 357 158 318L77 318L135 700L505 700L505 627L200 627L161 412Q186 441 227 459Q269 478 324 478Q376 478 418 460Q460 442 490 409Q520 376 536 332Q553 288 553 236Q553 185 536 140Q520 96 489 61Q458 27 413 7Q368 -12 311 -12Z",
        ),
        (
            '6',
            628,
            "M326 -12Q250 -12 198 16Q146 45 115 92Q84 140 69 200Q55 260 55 322Q55 438 87 525Q119 613 179 662Q240 712 325 712Q391 712 438 686Q486 661 514 618Q542 575 548 522L470 522Q460 578 421 608Q383 639 324 639Q272 639 230 606Q188 574 163 507Q139 441 135 338Q135 337 135 335Q135 333 135 331Q151 362 179 389Q208 416 248 432Q288 448 337 448Q396 448 448 420Q500 392 532 341Q565 291 565 223Q565 164 536 110Q508 56 454 22Q400 -12 326 -12ZM320 62Q366 62 402 82Q439 103 459 138Q480 174 480 219Q480 266 459 301Q439 336 402 355Q366 375 320 375Q274 375 237 354Q200 333 178 297Q157 262 157 218Q157 173 178 137Q200 102 237 82Q274 62 320 62Z",
        ),
        (
            '7',
            534,
            "M138 0L411 628L27 628L27 700L497 700L497 637L227 0Z",
        ),
        (
            '8',
            608,
            "M304 -12Q236 -12 182 13Q128 39 96 84Q64 130 64 190Q64 233 81 270Q98 307 128 334Q159 362 199 373Q143 393 112 435Q82 477 82 526Q82 578 109 620Q136 662 185 687Q235 712 304 712Q374 712 423 687Q473 662 499 620Q526 578 526 526Q526 477 495 435Q465 393 409 373Q450 362 480 334Q511 307 527 270Q544 233 544 190Q544 130 512 84Q481 39 426 13Q372 -12 304 -12ZM304 62Q383 62 421 98Q460 135 460 195Q460 263 417 298Q375 333 304 333Q233 333 190 298Q148 263 148 195Q148 135 187 98Q226 62 304 62ZM304 403Q367 403 404 436Q441 469 441 521Q441 580 404 610Q368 640 304 640Q241 640 204 610Q168 580 168 521Q168 469 204 436Q241 403 304 403Z",
        ),
        (
            '9',
            628,
            "M303 -12Q237 -12 190 13Q143 39 115 82Q88 126 82 178L160 178Q170 122 208 91Q246 61 304 61Q357 61 398 93Q439 125 464 190Q490 256 493 357Q493 360 493 363Q493 366 493 369Q478 338 449 311Q420 284 380 268Q341 252 292 252Q233 252 180 280Q128 308 95 358Q63 409 63 477Q63 536 92 590Q121 644 175 678Q229 712 303 712Q378 712 430 683Q482 655 513 607Q545 560 559 500Q573 440 573 378Q573 263 541 175Q510 87 449 37Q389 -12 303 -12ZM309 325Q355 325 392 346Q429 367 450 403Q472 439 472 482Q472 528 450 563Q429 598 392 618Q355 638 309 638Q263 638 226 617Q190 597 169 562Q148 527 148 481Q148 434 169 399Q190 364 226 344Q263 325 309 325Z",
        ),
        (
            ':',
            202,
            "M100 -5Q75 -5 58 11Q42 28 42 52Q42 76 58 92Q75 109 100 109Q125 109 141 92Q157 76 157 52Q157 28 141 11Q125 -5 100 -5ZM100 402Q75 402 58 418Q42 435 42 459Q42 483 58 499Q75 516 100 516Q125 516 141 499Q157 483 157 459Q157 435 141 418Q125 402 100 402Z",
        ),
        (
            ';',
            227,
            "M125 402Q100 402 83 418Q67 435 67 459Q67 483 83 499Q100 516 125 516Q150 516 166 499Q182 483 182 459Q182 435 166 418Q150 402 125 402ZM24 -123L82 98L169 98L82 -123Z",
        ),
        (
            '<',
            550,
            "M451 43L344 43L100 273L100 273L344 504L451 504L206 273Z",
        ),
        (
            '=',
            550,
            "M75 359L75 431L474 431L474 359ZM75 151L75 223L474 223L474 151Z",
        ),
        (
            '>',
            550,
            "M99 43L344 273L99 504L206 504L450 273L450 273L206 43Z",
        ),
        (
            '?',
            524,
            "M158 201L154 357L185 357Q227 357 265 362Q304 368 333 383Q363 399 380 429Q398 459 398 507Q398 549 380 579Q362 609 330 625Q298 641 256 641Q214 641 182 625Q150 609 132 580Q115 552 115 512L35 512Q35 572 63 617Q91 662 141 687Q191 712 256 712Q321 712 371 687Q422 662 450 616Q479 570 479 507Q479 433 447 387Q416 341 361 320Q306 299 234 299L231 201ZM194 -5Q169 -5 152 11Q136 28 136 52Q136 76 152 92Q169 109 194 109Q219 109 235 92Q251 76 251 52Q251 28 235 11Q219 -5 194 -5Z",
        ),
        (
            '@',
            1005,
            "M430 -199Q325 -199 245 -157Q165 -116 120 -39Q76 37 76 141Q76 247 111 337Q147 428 212 496Q277 564 367 602Q457 640 566 640Q676 640 757 597Q839 555 884 479Q929 403 929 303Q929 245 915 190Q902 135 875 91Q848 47 809 20Q770 -6 719 -6Q659 -6 632 31Q606 68 613 126L616 160L633 130Q608 67 554 30Q500 -6 436 -6Q361 -6 319 40Q277 86 277 165Q277 220 294 270Q311 320 342 359Q374 399 417 422Q461 445 515 445Q576 445 612 412Q648 380 654 317L634 304L657 434L728 434L678 147Q671 105 682 78Q694 52 731 52Q763 52 787 75Q812 98 828 135Q845 172 853 215Q861 258 861 299Q861 388 824 450Q787 513 718 547Q650 581 556 581Q465 581 389 548Q314 515 259 455Q204 396 174 317Q144 239 144 148Q144 56 180 -8Q217 -72 285 -105Q353 -139 446 -139Q489 -139 531 -130Q574 -122 611 -105L625 -162Q580 -181 531 -190Q482 -199 430 -199ZM460 58Q503 58 538 81Q573 104 596 144Q619 185 624 239Q629 278 618 310Q607 342 582 361Q557 381 517 381Q478 381 448 363Q418 346 397 317Q376 288 365 252Q354 216 354 178Q354 124 381 91Q408 58 460 58Z",
        ),
        (
            'A',
            664,
            "M30 0L288 700L377 700L634 0L545 0L332 595L332 595L118 0ZM148 180L172 248L489 248L513 180Z",
        ),
        (
            'B',
            603,
            "M75 0L75 700L321 700Q395 700 443 676Q492 653 516 612Q540 572 540 522Q540 468 516 432Q493 396 455 377Q418 358 376 354L390 364Q435 363 473 340Q511 317 533 278Q556 239 556 191Q556 138 530 94Q504 51 453 25Q403 0 329 0ZM159 70L320 70Q391 70 430 103Q470 137 470 197Q470 256 429 291Q389 327 318 327L159 327ZM159 394L315 394Q383 394 418 426Q454 458 454 513Q454 566 418 598Q383 630 312 630L159 630Z",
        ),
        (
            'C',
            717,
            "M379 -12Q277 -12 202 33Q128 79 88 160Q48 242 48 350Q48 458 88 539Q128 621 202 666Q277 712 379 712Q499 712 574 655Q649 598 670 494L577 494Q561 559 512 598Q463 638 379 638Q304 638 249 603Q194 569 164 504Q134 440 134 350Q134 260 164 195Q194 131 249 96Q304 62 379 62Q463 62 512 100Q561 139 577 203L670 203Q649 102 574 45Q499 -12 379 -12Z",
        ),
        (
            'D',
            688,
            "M75 0L75 700L281 700Q404 700 483 657Q563 615 601 536Q640 458 640 348Q640 240 601 162Q563 84 483 42Q404 0 281 0ZM159 70L279 70Q381 70 441 103Q501 137 527 199Q553 262 553 348Q553 436 527 499Q501 562 441 596Q381 630 279 630L159 630Z",
        ),
        (
            'E',
            565,
            "M75 0L75 700L508 700L508 631L159 631L159 387L478 387L478 319L159 319L159 69L508 69L508 0Z",
        ),
        (
            'F',
            535,
            "M75 0L75 700L501 700L501 631L159 631L159 385L450 385L450 317L159 317L159 0Z",
        ),
        (
            'G',
            758,
            "M376 -12Q278 -12 204 33Q130 79 89 160Q48 241 48 349Q48 456 89 538Q131 620 207 666Q284 712 388 712Q506 712 585 655Q664 599 686 498L589 498Q575 561 523 598Q471 635 388 635Q311 635 253 600Q196 566 165 502Q134 438 134 349Q134 260 165 195Q196 131 251 97Q307 63 379 63Q491 63 548 129Q605 195 613 308L411 308L411 374L699 374L699 0L622 0L615 115L615 115Q590 74 557 45Q525 17 481 2Q437 -12 376 -12Z",
        ),
        (
            'H',
            681,
            "M522 0L522 700L606 700L606 0ZM75 0L75 700L159 700L159 0ZM143 322L143 391L543 391L543 322Z",
        ),
        ('I', 234, "M75 0L75 700L159 700L159 0Z"),
        (
            'J',
            501,
            "M231 -12Q168 -12 122 12Q77 36 52 81Q28 126 28 188L113 188Q113 153 124 124Q136 96 161 79Q187 63 230 63Q272 63 296 79Q321 95 331 122Q342 150 342 185L342 700L426 700L426 185Q426 91 374 39Q323 -12 231 -12Z",
        ),
        (
            'K',
            584,
            "M75 0L75 700L159 700L159 383L451 700L557 700L297 421L561 0L459 0L237 356L159 273L159 0Z",
        ),
        ('L', 527, "M75 0L75 700L159 700L159 67L485 67L485 0Z"),
        (
            'M',
            841,
            "M75 0L75 700L173 700L420 211L420 211L668 700L767 700L767 0L683 0L683 550L683 550L452 98L389 98L159 548L159 548L159 0Z",
        ),
        (
            'N',
            691,
            "M75 0L75 700L159 700L532 139L532 139L532 700L616 700L616 0L532 0L159 561L159 561L159 0Z",
        ),
        (
            'O',
            774,
            "M387 -12Q286 -12 209 33Q133 79 90 160Q48 242 48 350Q48 457 90 538Q133 620 209 666Q286 712 387 712Q490 712 566 666Q642 620 684 538Q727 457 727 350Q727 242 684 160Q642 79 566 33Q490 -12 387 -12ZM387 62Q463 62 520 96Q578 131 609 195Q641 260 641 350Q641 440 609 504Q578 569 520 603Q463 637 387 637Q312 637 254 603Q197 569 165 504Q134 440 134 350Q134 260 165 195Q197 131 254 96Q312 62 387 62Z",
        ),
        (
            'P',
            580,
            "M75 0L75 700L303 700Q386 700 439 672Q493 645 518 599Q544 553 544 494Q544 437 518 390Q493 344 440 316Q387 288 303 288L159 288L159 0ZM159 359L302 359Q386 359 422 396Q458 433 458 494Q458 559 422 594Q386 630 302 630L159 630Z",
        ),
        (
            'Q',
            774,
            "M573 -95L338 273L435 273L670 -95ZM387 -12Q286 -12 209 33Q133 79 90 160Q48 242 48 350Q48 457 90 538Q133 620 209 666Q286 712 387 712Q490 712 566 666Q642 620 684 538Q727 457 727 350Q727 242 684 160Q642 79 566 33Q490 -12 387 -12ZM387 62Q463 62 520 96Q578 131 609 195Q641 260 641 350Q641 440 609 504Q578 569 520 603Q463 637 387 637Q312 637 254 603Q197 569 165 504Q134 440 134 350Q134 260 165 195Q197 131 254 96Q312 62 387 62Z",
        ),
        (
            'R',
            594,
            "M75 0L75 700L301 700Q383 700 435 672Q487 645 512 599Q537 553 537 495Q537 439 511 392Q486 345 433 317Q380 289 297 289L159 289L159 0ZM445 0L291 313L385 313L544 0ZM159 355L295 355Q375 355 412 394Q450 433 450 494Q450 556 413 592Q377 629 294 629L159 629Z",
        ),
        (
            'S',
            580,
            "M298 -12Q221 -12 164 16Q107 44 76 94Q45 144 45 210L133 210Q133 169 152 134Q171 100 207 79Q244 59 298 59Q345 59 378 74Q412 90 429 117Q447 144 447 178Q447 219 429 244Q412 270 382 286Q352 302 312 314Q273 327 230 341Q147 369 108 411Q69 453 69 520Q69 577 95 620Q122 663 171 687Q221 712 289 712Q356 712 405 687Q455 662 483 618Q511 575 511 518L423 518Q423 547 408 575Q393 603 362 621Q332 640 286 640Q248 641 218 627Q189 614 172 589Q156 564 156 528Q156 494 170 473Q185 452 212 437Q240 423 277 411Q314 399 359 384Q410 367 449 342Q489 318 511 280Q534 242 534 183Q534 133 507 88Q481 44 429 16Q377 -12 298 -12Z",
        ),
        (
            'T',
            561,
            "M239 0L239 631L30 631L30 700L532 700L532 631L323 631L323 0Z",
        ),
        (
            'U',
            655,
            "M327 -12Q255 -12 196 17Q138 46 103 105Q68 165 68 258L68 700L152 700L152 257Q152 189 174 146Q197 103 237 83Q277 63 329 63Q382 63 421 83Q460 103 482 146Q504 189 504 257L504 700L588 700L588 258Q588 165 553 105Q518 46 459 17Q400 -12 327 -12Z",
        ),
        (
            'V',
            669,
            "M285 0L22 700L113 700L334 89L334 89L557 700L646 700L383 0Z",
        ),
        (
            'W',
            968,
            "M219 0L27 700L117 700L268 100L268 100L439 700L533 700L700 98L700 98L853 700L943 700L747 0L651 0L484 579L484 579L313 0Z",
        ),
        (
            'X',
            603,
            "M38 0L252 353L38 700L133 700L305 421L305 421L469 700L563 700L349 350L564 0L469 0L296 282L296 282L133 0Z",
        ),
        (
            'Y',
            580,
            "M248 0L248 266L20 700L115 700L303 321L278 321L465 700L559 700L332 266L332 0Z",
        ),
        (
            'Z',
            541,
            "M45 0L45 66L394 627L50 627L50 700L492 700L492 634L142 73L496 73L496 0Z",
        ),
        (
            '[',
            314,
            "M75 -151L75 811L254 811L254 742L154 742L154 -81L254 -81L254 -151Z",
        ),
        ('\\', 392, "M362 -95L279 -95L28 764L111 764Z"),
        (
            ']',
            314,
            "M239 -151L60 -151L60 -81L160 -81L160 742L60 742L60 811L239 811Z",
        ),
        (
            '^',
            622,
            "M38 210L273 700L350 700L585 210L499 210L312 601L312 601L124 210Z",
        ),
        ('_', 660, "M72 -115L72 -36L588 -36L588 -115Z"),
        ('`', 213, "M204 547L0 663L12 735L12 735L213 607Z"),
        (
            'a',
            544,
            "M234 -12Q173 -12 132 9Q91 31 71 66Q51 102 51 144Q51 195 77 230Q104 266 152 284Q201 303 267 303L400 303Q400 350 385 381Q371 413 343 429Q316 445 275 445Q227 445 192 421Q157 397 149 350L63 350Q69 404 99 440Q130 477 177 496Q224 516 275 516Q345 516 391 490Q438 465 461 419Q484 374 484 312L484 0L409 0L404 89L404 89Q394 68 378 49Q363 31 342 17Q321 4 294 -4Q268 -12 234 -12ZM247 59Q282 59 310 73Q339 88 359 113Q379 139 389 170Q400 201 400 235L400 238L274 238Q225 238 195 226Q165 215 152 194Q139 174 139 148Q139 121 151 101Q164 81 188 70Q213 59 247 59Z",
        ),
        (
            'b',
            626,
            "M334 -12Q291 -12 256 0Q222 13 196 34Q170 55 153 83L153 83L146 0L70 0L70 720L154 720L154 421Q178 460 223 488Q268 516 334 516Q407 516 462 481Q517 447 547 387Q578 328 578 252Q578 176 547 116Q517 56 462 22Q407 -12 334 -12ZM324 61Q373 61 411 85Q449 109 470 151Q492 194 492 252Q492 309 470 352Q449 395 411 419Q373 443 324 443Q274 443 236 419Q198 395 177 352Q156 309 156 252Q156 194 177 151Q198 109 236 85Q274 61 324 61Z",
        ),
        (
            'c',
            572,
            "M297 -12Q226 -12 169 21Q113 54 81 113Q49 173 49 251Q49 331 81 390Q113 449 169 482Q226 516 297 516Q387 516 447 469Q507 423 523 345L437 345Q427 392 388 418Q349 444 296 444Q253 444 216 422Q179 401 157 358Q135 315 135 252Q135 205 148 168Q161 132 183 108Q205 84 234 71Q264 59 296 59Q332 59 361 70Q390 82 410 105Q430 128 437 159L523 159Q507 83 446 35Q386 -12 297 -12Z",
        ),
        (
            'd',
            627,
            "M292 -12Q219 -12 164 22Q109 57 79 117Q49 177 49 253Q49 329 79 388Q109 448 164 482Q220 516 293 516Q355 516 401 491Q448 466 473 421L473 720L557 720L557 0L481 0L474 83L474 83Q458 58 433 36Q408 15 373 1Q338 -12 292 -12ZM302 61Q352 61 390 85Q428 109 449 151Q470 194 470 252Q470 309 449 352Q428 395 390 419Q352 443 302 443Q253 443 215 419Q177 395 156 352Q135 309 135 252Q135 194 156 151Q177 109 215 85Q253 61 302 61Z",
        ),
        (
            'e',
            568,
            "M291 -12Q220 -12 165 21Q111 54 80 113Q49 173 49 252Q49 332 79 391Q110 450 165 483Q221 516 293 516Q367 516 418 483Q470 450 497 396Q525 343 525 279Q525 269 525 258Q525 247 524 233L112 233L112 298L443 298Q440 367 397 406Q355 445 291 445Q248 445 211 425Q175 406 153 368Q131 331 131 275L131 247Q131 185 153 143Q176 101 212 80Q249 59 291 59Q344 59 378 82Q413 106 428 146L511 146Q499 101 469 65Q439 29 394 8Q350 -12 291 -12Z",
        ),
        (
            'f',
            340,
            "M98 0L98 582Q98 630 114 660Q130 691 161 705Q193 720 239 720L296 720L296 648L252 648Q215 648 198 633Q182 618 182 580L182 0ZM21 433L21 504L309 504L309 433Z",
        ),
        (
            'g',
            558,
            "M269 -232Q201 -232 148 -214Q96 -197 67 -161Q38 -125 38 -72Q38 -49 47 -23Q57 3 80 28Q104 53 147 73L205 39Q152 17 136 -11Q120 -40 120 -64Q120 -97 139 -119Q158 -141 192 -152Q226 -163 269 -163Q312 -163 343 -151Q374 -139 391 -117Q408 -95 408 -65Q408 -30 382 -6Q356 17 283 21Q224 25 184 32Q144 39 118 48Q92 58 74 70Q57 82 43 95L43 118L140 214L213 188L113 98L126 133Q137 125 147 118Q157 111 174 106Q191 101 221 97Q252 93 302 89Q371 84 412 65Q454 46 472 14Q491 -18 491 -62Q491 -105 467 -143Q444 -182 395 -207Q346 -232 269 -232ZM268 158Q205 158 160 182Q116 206 93 246Q70 287 70 337Q70 387 93 427Q117 467 161 491Q205 516 268 516Q331 516 375 491Q419 467 442 427Q465 387 465 337Q465 287 442 246Q419 206 375 182Q331 158 268 158ZM268 227Q322 227 353 255Q384 283 384 337Q384 390 353 418Q322 446 268 446Q215 446 182 418Q150 390 150 337Q150 283 182 255Q214 227 268 227ZM350 433L325 504L531 504L531 441Z",
        ),
        (
            'h',
            575,
            "M70 0L70 720L154 720L154 419L154 419Q179 464 224 490Q270 516 324 516Q382 516 425 492Q468 469 491 421Q514 373 514 300L514 0L431 0L431 291Q431 367 399 405Q367 444 306 444Q263 444 228 423Q194 402 174 363Q154 324 154 267L154 0Z",
        ),
        (
            'i',
            240,
            "M78 0L78 504L162 504L162 0ZM120 611Q95 611 78 627Q62 644 62 669Q62 694 78 709Q95 725 120 725Q144 725 161 709Q178 694 178 669Q178 644 161 627Q144 611 120 611Z",
        ),
        (
            'j',
            243,
            "M-27 -220L-27 -148L12 -148Q50 -148 66 -132Q82 -117 82 -80L82 504L166 504L166 -82Q166 -130 150 -160Q134 -191 102 -205Q71 -220 25 -220ZM125 611Q100 611 83 627Q67 644 67 669Q67 694 83 709Q100 725 125 725Q149 725 165 709Q182 694 182 669Q182 644 165 627Q149 611 125 611Z",
        ),
        (
            'k',
            504,
            "M380 0L140 276L359 504L462 504L211 246L212 306L487 0ZM70 0L70 720L154 720L154 0Z",
        ),
        ('l', 223, "M70 0L70 720L154 720L154 0Z"),
        (
            'm',
            891,
            "M70 0L70 504L146 504L151 432L151 432Q175 471 215 493Q256 516 304 516Q343 516 374 505Q406 495 430 473Q454 452 469 419L469 419Q495 465 540 490Q586 516 638 516Q696 516 739 492Q783 469 806 421Q830 374 830 301L830 0L747 0L747 292Q747 368 716 406Q685 444 629 444Q590 444 559 423Q528 403 510 364Q492 325 492 268L492 0L408 0L408 292Q408 368 377 406Q346 444 289 444Q251 444 220 423Q190 403 172 364Q154 325 154 268L154 0Z",
        ),
        (
            'n',
            574,
            "M70 0L70 504L146 504L150 416L150 416Q174 463 218 489Q263 516 320 516Q379 516 422 492Q465 469 489 421Q513 374 513 301L513 0L429 0L429 292Q429 368 395 406Q362 444 301 444Q259 444 226 423Q193 403 173 364Q154 326 154 269L154 0Z",
        ),
        (
            'o',
            593,
            "M295 -12Q225 -12 169 20Q113 53 81 112Q49 172 49 251Q49 332 81 391Q114 451 170 483Q227 516 297 516Q369 516 424 483Q480 451 512 391Q545 332 545 252Q545 172 512 112Q479 53 423 20Q367 -12 295 -12ZM296 60Q339 60 376 81Q413 103 436 146Q459 189 459 252Q459 316 436 358Q414 401 377 422Q341 444 297 444Q255 444 217 422Q180 401 157 358Q135 316 135 252Q135 189 157 146Q180 103 216 81Q253 60 296 60Z",
        ),
        (
            'p',
            626,
            "M70 -220L70 504L146 504L153 422L153 422Q169 446 194 467Q219 489 254 502Q289 516 334 516Q408 516 462 481Q517 446 547 386Q578 327 578 251Q578 175 547 115Q517 56 462 22Q407 -12 333 -12Q272 -12 225 13Q179 38 154 83L154 -220ZM324 61Q373 61 411 85Q449 109 470 151Q492 194 492 252Q492 309 470 352Q449 395 411 419Q373 443 324 443Q274 443 236 419Q198 395 177 352Q156 309 156 252Q156 194 177 151Q198 109 236 85Q274 61 324 61Z",
        ),
        (
            'q',
            627,
            "M473 -220L473 82Q449 43 403 15Q358 -12 293 -12Q219 -12 164 22Q109 57 79 116Q49 176 49 252Q49 328 79 387Q109 447 164 481Q220 516 293 516Q336 516 370 504Q405 492 431 470Q457 449 473 421L473 421L481 504L557 504L557 -220ZM302 61Q352 61 390 85Q428 109 449 151Q470 194 470 252Q470 309 449 352Q428 395 390 419Q352 443 302 443Q253 443 215 419Q177 395 156 352Q135 309 135 252Q135 194 156 151Q177 109 215 85Q253 61 302 61Z",
        ),
        (
            'r',
            370,
            "M70 0L70 504L146 504L152 408L152 408Q169 442 195 466Q222 490 259 503Q297 516 345 516L345 428L314 428Q282 428 253 419Q224 411 201 392Q179 373 166 340Q154 308 154 260L154 0Z",
        ),
        (
            's',
            502,
            "M260 -12Q197 -12 150 9Q104 30 76 67Q49 105 43 156L129 156Q134 130 150 108Q166 86 193 72Q221 59 261 59Q296 59 319 69Q342 80 353 98Q365 116 365 137Q365 168 350 184Q336 200 308 209Q280 219 240 226Q207 232 175 242Q143 252 118 268Q93 285 78 309Q64 333 64 368Q64 411 86 444Q108 478 149 497Q191 516 246 516Q327 516 377 477Q427 438 436 364L353 364Q348 402 320 423Q293 445 245 445Q198 445 173 426Q149 407 149 376Q149 356 162 340Q176 325 202 314Q229 303 268 296Q318 287 359 272Q400 257 425 227Q451 198 451 144Q451 97 427 62Q404 27 361 7Q318 -12 260 -12Z",
        ),
        (
            't',
            392,
            "M268 0Q223 0 190 14Q157 28 139 61Q122 95 122 152L122 433L34 433L34 504L122 504L133 626L206 626L206 504L352 504L352 433L206 433L206 152Q206 105 225 88Q244 72 292 72L346 72L346 0Z",
        ),
        (
            'u',
            573,
            "M253 -12Q195 -12 151 11Q108 34 84 82Q61 130 61 202L61 504L145 504L145 211Q145 136 178 98Q211 60 271 60Q313 60 347 80Q381 101 400 139Q420 178 420 234L420 504L504 504L504 0L428 0L423 88L423 88Q400 41 355 14Q310 -12 253 -12Z",
        ),
        (
            'v',
            527,
            "M215 0L24 504L112 504L264 81L264 81L417 504L503 504L312 0Z",
        ),
        (
            'w',
            763,
            "M172 0L24 504L108 504L223 79L208 79L333 504L428 504L555 80L539 79L653 504L738 504L591 0L505 0L373 444L390 444L258 0Z",
        ),
        (
            'x',
            496,
            "M20 0L191 252L20 504L112 504L248 298L248 298L386 504L476 504L305 252L476 0L386 0L248 207L248 207L112 0Z",
        ),
        (
            'y',
            556,
            "M125 -220L248 56L219 56L21 504L112 504L276 119L276 119L448 504L535 504L213 -220Z",
        ),
        (
            'z',
            458,
            "M36 0L36 69L315 434L41 434L41 504L411 504L411 435L131 70L416 70L416 0Z",
        ),
        (
            '{',
            427,
            "M321 -152Q253 -152 211 -114Q169 -77 169 -2Q169 32 174 59Q180 87 185 116Q191 146 191 184Q191 209 182 231Q173 254 150 271Q127 288 84 296L84 364Q127 372 150 388Q173 405 182 428Q191 451 191 476Q191 515 185 543Q180 572 174 599Q169 627 169 661Q169 737 211 774Q253 811 321 811L375 811L375 739L336 739Q295 739 273 719Q251 700 251 654Q251 625 256 598Q262 572 267 542Q272 513 272 473Q272 421 246 382Q220 344 161 331L161 329Q220 316 246 277Q272 239 272 186Q272 147 267 117Q262 88 256 61Q251 35 251 5Q251 -41 273 -60Q295 -80 336 -80L375 -80L375 -152Z",
        ),
        ('|', 234, "M75 -100L75 720L159 720L159 -100Z"),
        (
            '}',
            427,
            "M106 -152L52 -152L52 -80L91 -80Q132 -80 154 -60Q176 -41 176 5Q176 35 171 61Q166 88 160 117Q155 147 155 186Q155 239 181 277Q208 316 266 329L266 331Q208 344 181 382Q155 421 155 473Q155 513 160 542Q166 572 171 598Q176 625 176 654Q176 700 154 719Q132 739 91 739L52 739L52 811L106 811Q174 811 216 774Q258 737 258 661Q258 627 252 599Q247 572 241 543Q236 515 236 476Q236 451 245 428Q255 405 278 388Q301 372 343 364L343 296Q301 288 278 271Q255 254 245 231Q236 209 236 184Q236 146 241 116Q247 87 252 59Q258 32 258 -2Q258 -77 216 -114Q174 -152 106 -152Z",
        ),
        (
            '~',
            550,
            "M358 248Q330 248 308 258Q286 269 266 282Q247 296 228 306Q209 317 187 317Q160 317 143 298Q127 279 122 250L58 250Q65 298 83 329Q101 361 129 376Q157 392 192 392Q221 392 243 381Q266 371 285 358Q304 345 323 334Q342 324 363 324Q391 324 407 342Q424 361 428 391L492 391Q486 343 468 311Q450 280 422 264Q394 248 358 248Z",
        ),
        ('−', 550, "M77 260L77 331L473 331L473 260Z"),
        (
            '°',
            343,
            "M171 444Q135 444 105 460Q75 477 57 507Q39 537 39 577Q39 619 57 649Q76 679 106 695Q136 712 172 712Q208 712 237 695Q267 679 285 649Q303 619 303 577Q303 537 285 507Q267 477 237 460Q207 444 171 444ZM171 506Q195 506 216 524Q238 542 238 578Q238 614 217 632Q196 650 172 650Q147 650 126 632Q105 614 105 578Q105 542 125 524Q146 506 171 506Z",
        ),
        (
            'µ',
            584,
            "M70 -220L70 496L154 496L154 215Q154 138 186 99Q219 60 282 60Q325 60 358 81Q392 102 411 142Q431 182 431 239L431 496L515 496L515 0L439 0L434 88L434 88Q410 42 364 15Q319 -12 262 -12Q231 -12 203 -4Q176 4 154 20L154 -220Z",
        ),
    ];

    fn find(ch: char) -> Option<&'static (char, u16, &'static str)> {
        GLYPHS.iter().find(|(c, _, _)| *c == ch)
    }

    /// The outline of `ch`, if it has one (not for spaces).
    pub fn outline(ch: char) -> Option<&'static str> {
        find(ch).map(|(_, _, outline)| *outline)
    }

    /// The advance width of `ch`, in font units.
    pub fn advance(ch: char) -> Option<f32> {
        find(ch).map(|(_, advance, _)| f32::from(*advance))
    }

    /// Adds an outline's contours to `builder`, mapping font units through `place`.
    pub fn trace(
        outline: &str,
        builder: &mut PathBuilder,
        place: impl Fn(f32, f32) -> Point<Pixels>,
    ) {
        let mut tokens = outline
            .split(|c: char| c == ' ' || c.is_ascii_alphabetic())
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f32>().unwrap_or(0.0));
        let mut next = || {
            let x = tokens.next().unwrap_or(0.0);
            let y = tokens.next().unwrap_or(0.0);
            place(x, y)
        };
        for command in outline.chars().filter(char::is_ascii_alphabetic) {
            match command {
                'M' => builder.move_to(next()),
                'L' => builder.line_to(next()),
                'Q' => {
                    let ctrl = next();
                    builder.curve_to(next(), ctrl);
                }
                _ => builder.close(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: logic-level ports of components/ui/controls.test.tsx and
// controls-effects.test.tsx.

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> KnobSettings {
        KnobSettings {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            large_step: 10.0,
            fine_step: 0.1,
            reset: 0.0,
            arc: 270.0,
            drag_direction: KnobDragDirection::Vertical,
            sensitivity: 200.0,
            taper: resolve_taper(&TaperInput::Linear, 0.0, 100.0),
            controlled: false,
            allow_wheel: false,
            disabled: false,
            clicks: None,
        }
    }

    /// `<Knob defaultValue={50} />`: reset falls back to defaultValue.
    fn knob(value: f32) -> (KnobSettings, KnobModel) {
        let s = KnobSettings {
            reset: value,
            ..settings()
        };
        let model = KnobModel::new(&s, Some(value));
        (s, model)
    }

    fn last_change(fx: &[KnobEffect]) -> Option<(f32, KnobChangeReason)> {
        fx.iter().rev().find_map(|e| match e {
            KnobEffect::Change(v, r) => Some((*v, *r)),
            _ => None,
        })
    }

    fn clicks(fx: &[KnobEffect]) -> usize {
        fx.iter().filter(|e| **e == KnobEffect::Click).count()
    }

    fn dial(x: f32, y: f32) -> DialPointer {
        // A 100px dial at the page origin.
        DialPointer {
            x,
            y,
            center_x: 50.0,
            center_y: 50.0,
            width: 100.0,
        }
    }

    #[test]
    fn knob_responds_to_the_keyboard_and_double_click() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        assert!(m.key(&s, "up", false, true, &mut fx));
        assert_eq!(last_change(&fx), Some((60.0, KnobChangeReason::Keyboard)));
        m.reset(&s, &mut fx);
        assert_eq!(last_change(&fx), Some((50.0, KnobChangeReason::Reset)));
    }

    #[test]
    fn knob_steps_with_alt_page_home_and_end() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        m.key(&s, "up", true, false, &mut fx);
        assert_eq!(m.latest, 50.1);
        m.key(&s, "pagedown", false, false, &mut fx);
        // PageDown snaps to `step`: min(increment, step).
        assert_eq!(m.latest, 40.0);
        m.key(&s, "end", false, false, &mut fx);
        assert_eq!(m.latest, 100.0);
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(m.latest, 100.0);
        m.key(&s, "home", false, false, &mut fx);
        assert_eq!(m.latest, 0.0);
        assert!(!m.key(&s, "x", false, false, &mut fx));
        assert_eq!(fx.last(), Some(&KnobEffect::Commit(0.0)));
    }

    #[test]
    fn knob_resets_on_alt_click() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        m.key(&s, "up", false, false, &mut fx);
        let started = m.pointer_down(&s, dial(50.0, 50.0), true, &mut fx);
        assert!(!started);
        assert_eq!(last_change(&fx), Some((50.0, KnobChangeReason::Reset)));
    }

    #[test]
    fn knob_drags_ten_times_finer_with_shift_without_jumping() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        assert!(m.pointer_down(&s, dial(0.0, 100.0), false, &mut fx));
        // 20px up is a tenth of the 200px sensitivity: 10 units, or 1 with Shift.
        m.pointer_move(&s, dial(0.0, 80.0), true, &mut fx);
        assert_eq!(last_change(&fx), Some((51.0, KnobChangeReason::Drag)));
        m.pointer_move(&s, dial(0.0, 60.0), false, &mut fx);
        assert_eq!(last_change(&fx), Some((61.0, KnobChangeReason::Drag)));
        assert!(m.pointer_up(&mut fx));
        assert_eq!(fx.last(), Some(&KnobEffect::Commit(61.0)));
    }

    #[test]
    fn knob_drags_horizontally() {
        let s = KnobSettings {
            drag_direction: KnobDragDirection::Horizontal,
            ..knob(50.0).0
        };
        let mut m = KnobModel::new(&s, Some(50.0));
        let mut fx = Vec::new();
        m.pointer_down(&s, dial(10.0, 10.0), false, &mut fx);
        m.pointer_move(&s, dial(50.0, 90.0), false, &mut fx);
        assert_eq!(m.latest, 70.0);
    }

    #[test]
    fn knob_turns_from_where_it_is_grabbed_when_circled_and_stops_at_its_ends() {
        let s = KnobSettings {
            drag_direction: KnobDragDirection::Circular,
            ..knob(50.0).0
        };
        let mut m = KnobModel::new(&s, Some(50.0));
        let mut fx = Vec::new();
        // Grabbed at 3 o'clock, a quarter turn is a third of the 270° arc.
        m.pointer_down(&s, dial(90.0, 50.0), false, &mut fx);
        m.pointer_move(&s, dial(50.0, 90.0), false, &mut fx);
        assert_eq!(last_change(&fx).unwrap().0, 83.0);
        // Past the end, across the gap at the bottom, it stays at the end.
        m.pointer_move(&s, dial(10.0, 50.0), false, &mut fx);
        assert_eq!(last_change(&fx).unwrap().0, 100.0);
        // Turning back moves it straight away.
        m.pointer_move(&s, dial(50.0, 90.0), false, &mut fx);
        assert_eq!(last_change(&fx).unwrap().0, 67.0);
    }

    #[test]
    fn knob_ignores_the_pointer_near_the_centre_while_circling() {
        let s = KnobSettings {
            drag_direction: KnobDragDirection::Circular,
            ..knob(50.0).0
        };
        let mut m = KnobModel::new(&s, Some(50.0));
        let mut fx = Vec::new();
        m.pointer_down(&s, dial(90.0, 50.0), false, &mut fx);
        m.pointer_move(&s, dial(48.0, 52.0), false, &mut fx);
        // Leaving the centre picks up from there, without a jump.
        m.pointer_move(&s, dial(50.0, 10.0), false, &mut fx);
        assert_eq!(last_change(&fx), None);
        m.pointer_move(&s, dial(90.0, 50.0), false, &mut fx);
        assert_eq!(last_change(&fx).unwrap().0, 83.0);
    }

    #[test]
    fn knob_applies_typed_values_clamped_and_quantized() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        m.apply_typed(&s, parse_knob_value("72"), &mut fx);
        assert_eq!(last_change(&fx), Some((72.0, KnobChangeReason::Input)));
        assert_eq!(fx.last(), Some(&KnobEffect::Commit(72.0)));
        m.apply_typed(&s, parse_knob_value("1.2k"), &mut fx);
        assert_eq!(m.latest, 100.0);
        let before = fx.len();
        m.apply_typed(&s, parse_knob_value("gain"), &mut fx);
        assert_eq!(fx.len(), before);
    }

    #[test]
    fn knob_stays_put_when_controlled_without_an_update() {
        let s = KnobSettings {
            controlled: true,
            ..knob(50.0).0
        };
        let mut m = KnobModel::new(&s, Some(50.0));
        let mut fx = Vec::new();
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(m.uncontrolled, 50.0);
        assert_eq!(last_change(&fx).unwrap().0, 51.0);
    }

    #[test]
    fn knob_wheel_turns_an_enabled_dial_only() {
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        assert!(m.wheel(&s, -100.0, false, &mut fx));
        assert_ne!(m.latest, 50.0);
        let disabled = KnobSettings {
            disabled: true,
            ..knob(50.0).0
        };
        let mut m = KnobModel::new(&disabled, Some(50.0));
        assert!(!m.wheel(&disabled, -100.0, false, &mut fx));
        assert_eq!(m.latest, 50.0);
        assert!(!m.key(&disabled, "up", false, false, &mut fx));
        assert!(!m.pointer_down(&disabled, dial(50.0, 50.0), false, &mut fx));
    }

    #[test]
    fn knob_reads_typed_values() {
        assert_eq!(parse_knob_value("\u{2212}12 dB"), Some(-12.0));
        assert_eq!(parse_knob_value("1.2k"), Some(1200.0));
        assert_eq!(parse_knob_value("gain"), None);
        assert_eq!(parse_knob_value("+3.5"), Some(3.5));
        assert_eq!(parse_knob_value("x.5"), Some(0.5));
        assert_eq!(parse_knob_value("20 K"), Some(20_000.0));
    }

    #[test]
    fn knob_formats_like_javascript_string() {
        assert_eq!(format_knob_number(50.0), "50");
        assert_eq!(format_knob_number(-0.0), "0");
        assert_eq!(format_knob_number(0.05), "0.05");
        assert_eq!(format_knob_number(50.1), "50.1");
    }

    #[test]
    fn knob_maps_value_to_angle() {
        assert_eq!(knob_angle_for(0.5, 270.0), 0.0);
        assert_eq!(knob_angle_for(0.0, 270.0), -135.0);
        assert_eq!(knob_angle_for(1.0, 270.0), 135.0);
        let (x, y) = point_at(90.0, 10.0);
        assert!((x - 60.0).abs() < 1e-4 && (y - 50.0).abs() < 1e-4);
        let (x, y) = point_at(0.0, 17.0);
        assert!((x - 50.0).abs() < 1e-4 && y < 50.0);
    }

    #[test]
    fn knob_draws_a_numbered_scale_lit_from_the_origin() {
        let s = knob(33.0).0;
        let position = s.taper.to_position(33.0);
        let lit = (0..=100)
            .filter(|i| {
                let p = *i as f32 / 100.0;
                p >= 0.0 - TICK_EPSILON && p <= position + TICK_EPSILON
            })
            .count();
        assert_eq!(lit, 34);
        assert_eq!(scale_detents(100, 5).len(), 21);
        let labels: Vec<String> = (0..=100 / 10)
            .map(|i| format_knob_number(round_tapered(s.taper.to_value(i as f32 * 10.0 / 100.0))))
            .collect();
        assert_eq!(
            labels,
            [
                "0", "10", "20", "30", "40", "50", "60", "70", "80", "90", "100"
            ]
        );
    }

    #[test]
    fn knob_lights_a_bipolar_scale_from_its_centre() {
        let taper = resolve_taper(&TaperInput::Linear, -24.0, 24.0);
        let position = taper.to_position(-12.0);
        let origin = taper.to_position(0.0);
        let (from, to) = (
            position.min(origin) - TICK_EPSILON,
            position.max(origin) + TICK_EPSILON,
        );
        let lit: Vec<u32> = (0..=48)
            .filter(|i| {
                let p = *i as f32 / 48.0;
                p >= from && p <= to
            })
            .collect();
        // -12 dB to 0 dB is ticks 12 to 24 of 48.
        assert_eq!(lit, (12..=24).collect::<Vec<_>>());
    }

    #[test]
    fn knob_clicks_on_graduations_only_with_click_sound() {
        // Silent knob.
        let (s, mut m) = knob(50.0);
        let mut fx = Vec::new();
        m.key(&s, "pageup", false, false, &mut fx);
        assert_eq!(clicks(&fx), 0);

        // Without a scale it clicks every largeStep: 59 is silent, 60 clicks.
        let s = KnobSettings {
            clicks: Some(KnobClicks::Every(10.0)),
            ..knob(58.0).0
        };
        let mut m = KnobModel::new(&s, Some(58.0));
        let mut fx = Vec::new();
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(clicks(&fx), 0);
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(clicks(&fx), 1);
        // Leaving a graduation is silent; reaching the one below clicks.
        m.key(&s, "down", false, false, &mut fx);
        assert_eq!(clicks(&fx), 1);
        m.key(&s, "pagedown", false, false, &mut fx);
        assert_eq!(clicks(&fx), 2);

        // With a scale it clicks on the long ticks, every 5 here.
        let s = KnobSettings {
            clicks: Some(KnobClicks::Detents(scale_detents(100, 5))),
            ..knob(33.0).0
        };
        let mut m = KnobModel::new(&s, Some(33.0));
        let mut fx = Vec::new();
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(clicks(&fx), 0);
        m.key(&s, "up", false, false, &mut fx);
        assert_eq!(clicks(&fx), 1);
    }

    #[test]
    fn knob_clicks_at_most_every_30_ms() {
        let sound = create_click_sound(Default::default());
        let options = ClickPlayOptions {
            when: None,
            min_interval_ms: CLICK_INTERVAL_MS,
        };
        let mut random = || 0.5;
        assert!(
            sound
                .play_at(options, 48_000, 1000.0, &mut random)
                .is_some()
        );
        assert!(
            sound
                .play_at(options, 48_000, 1010.0, &mut random)
                .is_none()
        );
        assert!(
            sound
                .play_at(options, 48_000, 1040.0, &mut random)
                .is_some()
        );
    }

    #[test]
    fn knob_keeps_one_value_width() {
        let taper = resolve_taper(&TaperInput::Linear, 0.0, 100.0);
        let s = settings();
        assert_eq!(
            widest_value(&format_knob_number, &taper, |v| s.quantize(v, 1.0)),
            3
        );
    }

    #[test]
    fn knob_cap_reports_its_bezel_radius() {
        assert_eq!(KnobCap::new().radius(), 25.5);
        assert_eq!(KnobCap::new().variant(KnobCapVariant::Mini).radius(), 32.0);
    }

    #[test]
    fn knob_turn_between_is_shortest() {
        assert_eq!(turn_between(170.0, -170.0), 20.0);
        assert_eq!(turn_between(-170.0, 170.0), -20.0);
        assert_eq!(turn_between(90.0, 180.0), 90.0);
    }

    #[test]
    fn knob_editor_draft_replaces_the_selection() {
        let mut draft = ValueDraft::new("50".into());
        assert_eq!(draft.selection(), (0, 2));
        draft.insert("7");
        draft.insert("2");
        assert_eq!(draft.text, "72");
    }
}
