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
//! - The scale's numbers stay upright: React turns each one with SVG
//!   `rotate()`, and GPUI cannot rotate glyphs.
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
        paint_centered_label(
            &text,
            view.at(point_at(angle, LABEL)),
            font_size,
            font.clone(),
            theme.muted_foreground,
            window,
        );
    }
}

/// SVG `<text text-anchor="middle" dominant-baseline="central">`, centred on
/// its point. React also turns each number with `rotate(angle)`; GPUI can't
/// rotate glyphs, so the numbers stay upright.
fn paint_centered_label(
    text: &str,
    center: Point<Pixels>,
    font_size: Pixels,
    font: Font,
    color: Hsla,
    window: &mut Window,
) {
    if text.is_empty() {
        return;
    }
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
    let width = line.width.as_f32();
    let mut xs: Vec<f32> = line
        .runs
        .iter()
        .flat_map(|r| r.glyphs.iter().map(|g| g.position.x.as_f32()))
        .collect();
    xs.push(width);
    let half_height = (line.ascent - line.descent).as_f32() / 2.0;
    let mut index = 0;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let advance = xs[index + 1] - xs[index];
            let along = xs[index] + advance / 2.0 - width / 2.0;
            index += 1;
            let glyph_center = (center.x.as_f32() + along, center.y.as_f32());
            let origin = point(
                px(glyph_center.0 - advance / 2.0),
                px(glyph_center.1 + half_height),
            );
            window
                .paint_glyph(origin, run.font_id, glyph.id, font_size, color)
                .ok();
        }
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

    // Halo: a radial shadow, shade 55% from `halo_from` fading out.
    let halo_inner = g.halo * g.halo_from;
    disk(view, halo_inner, shade.opacity(0.55), window);
    const HALO_RINGS: usize = 16;
    for index in 0..HALO_RINGS {
        let t0 = index as f32 / HALO_RINGS as f32;
        let t1 = (index + 1) as f32 / HALO_RINGS as f32;
        let r0 = halo_inner + (g.halo - halo_inner) * t0;
        let r1 = halo_inner + (g.halo - halo_inner) * t1;
        let alpha = 0.55 * (1.0 - (t0 + t1) / 2.0);
        ring(view, r0, r1, shade.opacity(alpha), window);
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
/// Rings finer than half a pixel blend into an even tint, as they do in a browser.
fn paint_brush(view: View, face: f32, pitch: f32, brush: Hsla, window: &mut Window) {
    if pitch <= 0.0 {
        return;
    }
    if pitch * view.unit < 0.5 {
        disk(view, face, brush.opacity(0.5), window);
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
        // Grey noise (linear-light ~0.5 → light grey), at the filter's 25% opacity.
        let lightness = 0.35 + 0.6 * level as f32 / (LEVELS - 1) as f32;
        fill_polygons(view, cells, gpui::hsla(0.0, 0.0, lightness, 0.12), window);
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
