//! Port of `components/ui/volume-control.tsx`.
//!
//! A simple volume slider with a mute button, for players. The volume is a
//! 0..1 gain like `HTMLMediaElement.volume`; the slider maps it through a
//! perceptual (squared) or linear curve. Compound: [`VolumeControl`] provides
//! its state to [`VolumeControlMute`], [`VolumeControlSlider`] and
//! [`VolumeControlValue`]; without children it renders mute + slider.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, ElementId, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Refineable, RenderOnce, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, canvas, div, px, relative,
};

use crate::audio::decibels::clamp;
use crate::audio::types::Orientation;
use crate::components::fader::{ThumbRing, decimal, js_round, round_value, thumb_shadows};
use crate::config::{AudioConfig, AudioSize};
use crate::theme::ActiveTheme;
use crate::ui::styles::{TextSize, css_shadow, provide, radius_lg, use_context};

const PERCENT: f32 = 100.0;
const LOW_LEVEL: f32 = 0.34;
const MEDIUM_LEVEL: f32 = 0.67;
const PERCEPTUAL_EXPONENT: f32 = 2.0;
/// Base UI's default `largeStep` (in value units, so it spans 0..1).
const LARGE_STEP: f32 = 10.0;
const HIT_SLOP: f32 = 6.0;

/// How loud the control is, for picking a speaker icon (`data-level`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VolumeLevel {
    Muted,
    Low,
    Medium,
    High,
}

/// How slider position maps to volume (`curve`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VolumeCurve {
    Linear,
    /// Position = √volume, so the slider's travel follows loudness.
    #[default]
    Perceptual,
}

impl VolumeCurve {
    pub fn to_position(self, volume: f32) -> f32 {
        match self {
            Self::Linear => volume,
            Self::Perceptual => volume.powf(1.0 / PERCEPTUAL_EXPONENT),
        }
    }

    pub fn to_volume(self, position: f32) -> f32 {
        match self {
            Self::Linear => position,
            Self::Perceptual => position.powf(PERCEPTUAL_EXPONENT),
        }
    }
}

/// The level shown by the mute button.
pub fn volume_level_for(volume: f32, muted: bool) -> VolumeLevel {
    if muted || volume <= 0.0 {
        VolumeLevel::Muted
    } else if volume < LOW_LEVEL {
        VolumeLevel::Low
    } else if volume < MEDIUM_LEVEL {
        VolumeLevel::Medium
    } else {
        VolumeLevel::High
    }
}

/// The mute button's action: unmuting at zero restores the last audible
/// volume. Returns (volume to restore, new muted state).
fn toggle_muted(volume: f32, muted: bool, last_audible: f32) -> (Option<f32>, bool) {
    let restore = (muted && volume <= 0.0).then_some(last_audible);
    (restore, !muted)
}

/// The volume to come back to when unmuting at zero (`lastAudibleRef`'s start).
fn initial_last_audible(volume: f32) -> f32 {
    if volume > 0.0 { volume } else { 1.0 }
}

/// What a [`VolumeControlMute`] content closure sees (`data-level`, `aria-pressed`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeControlMuteState {
    pub level: VolumeLevel,
    pub muted: bool,
}

type VolumeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type MutedHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Rounds a 0..1 position to `step` (Base UI `roundValueToStep`).
fn snap(position: f32, step: f32) -> f32 {
    let step = decimal(step);
    let p = decimal(position.clamp(0.0, 1.0));
    (round_value(js_round(p / step) * step) as f32).clamp(0.0, 1.0)
}

struct VolumeState {
    volume: f32,
    muted: bool,
    last_audible: f32,
    thumb_focus: FocusHandle,
    mute_focus: FocusHandle,
    dragging: bool,
    drag_offset: f32,
    hovered: bool,
    ring: ThumbRing,
    track: Rc<Cell<Option<Bounds<Pixels>>>>,
}

#[derive(Clone)]
struct VolumeCtx {
    state: Entity<VolumeState>,
    volume: f32,
    muted: bool,
    level: VolumeLevel,
    position: f32,
    step: f32,
    curve: VolumeCurve,
    orientation: Orientation,
    disabled: bool,
    thumb_size: f32,
    track_size: f32,
    volume_controlled: bool,
    muted_controlled: bool,
    on_value_change: Option<VolumeHandler>,
    on_value_committed: Option<VolumeHandler>,
    on_muted_change: Option<MutedHandler>,
}

impl VolumeCtx {
    fn horizontal(&self) -> bool {
        self.orientation == Orientation::Horizontal
    }

    /// The volume and muted state as of the latest change.
    fn current(&self, cx: &App) -> (f32, bool) {
        let s = self.state.read(cx);
        (s.volume, s.muted)
    }

    fn set_volume(&self, next: f32, window: &mut Window, cx: &mut App) {
        let controlled = self.volume_controlled;
        self.state.update(cx, |s, cx| {
            if next > 0.0 {
                s.last_audible = next;
            }
            // Mirrors the latest value so a following toggle or commit sees it.
            s.volume = next;
            if !controlled {
                cx.notify();
            }
        });
        if let Some(handler) = &self.on_value_change {
            handler(next, window, cx);
        }
    }

    fn set_muted(&self, next: bool, window: &mut Window, cx: &mut App) {
        let controlled = self.muted_controlled;
        self.state.update(cx, |s, cx| {
            s.muted = next;
            if !controlled {
                cx.notify();
            }
        });
        if let Some(handler) = &self.on_muted_change {
            handler(next, window, cx);
        }
    }

    fn set_position(&self, position: f32, window: &mut Window, cx: &mut App) {
        let (_, muted) = self.current(cx);
        let next = clamp(self.curve.to_volume(position), 0.0, 1.0);
        self.set_volume(next, window, cx);
        if muted && next > 0.0 {
            self.set_muted(false, window, cx);
        }
    }

    fn commit(&self, window: &mut Window, cx: &mut App) {
        let (volume, _) = self.current(cx);
        if let Some(handler) = &self.on_value_committed {
            handler(volume, window, cx);
        }
    }

    fn toggle_muted(&self, window: &mut Window, cx: &mut App) {
        let (volume, muted) = self.current(cx);
        let last = self.state.read(cx).last_audible;
        let (restore, next_muted) = toggle_muted(volume, muted, last);
        if let Some(restore) = restore {
            self.set_volume(restore, window, cx);
        }
        self.set_muted(next_muted, window, cx);
    }

    /// The slider's shown value (0 while muted).
    fn shown(&self, cx: &App) -> f32 {
        let (volume, muted) = self.current(cx);
        if muted {
            0.0
        } else {
            self.curve.to_position(volume)
        }
    }

    /// Base UI `onValueChange`: only when the slider value moves.
    fn slide_to(&self, position: f32, window: &mut Window, cx: &mut App) {
        let position = snap(position, self.step);
        if position != self.shown(cx) {
            self.set_position(position, window, cx);
        }
    }

    fn pointer_position(&self, pointer: Point<Pixels>, cx: &App) -> Option<f32> {
        let s = self.state.read(cx);
        let track = s.track.get()?;
        let offset = s.drag_offset;
        Some(if self.horizontal() {
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
        })
    }

    fn drag_to(&self, pointer: Point<Pixels>, window: &mut Window, cx: &mut App) {
        if let Some(position) = self.pointer_position(pointer, cx) {
            self.slide_to(position, window, cx);
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
            self.commit(window, cx);
        }
    }

    fn handle_key(&self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
        let shown = self.shown(cx);
        let increment = if event.keystroke.modifiers.shift {
            LARGE_STEP
        } else {
            self.step
        };
        let next = match event.keystroke.key.as_str() {
            "up" | "right" => shown + increment,
            "down" | "left" => shown - increment,
            "pageup" => shown + LARGE_STEP,
            "pagedown" => shown - LARGE_STEP,
            "home" => 0.0,
            "end" => 1.0,
            _ => return false,
        };
        self.slide_to(next.clamp(0.0, 1.0), window, cx);
        self.commit(window, cx);
        true
    }
}

fn volume_ctx(part: &str) -> VolumeCtx {
    use_context::<VolumeCtx>()
        .unwrap_or_else(|| panic!("{part} must be used inside VolumeControl."))
}

/// A simple volume slider with a mute button, for players.
///
/// ```ignore
/// VolumeControl::new("volume")
///     .default_value(0.6)
///     .child(VolumeControlMute::new().content(|state| speaker_icon(state.level)))
///     .child(VolumeControlSlider::new())
///     .child(VolumeControlValue::new())
/// ```
#[derive(IntoElement)]
pub struct VolumeControl {
    id: ElementId,
    value: Option<f32>,
    default_value: f32,
    on_value_change: Option<VolumeHandler>,
    on_value_committed: Option<VolumeHandler>,
    muted: Option<bool>,
    default_muted: bool,
    on_muted_change: Option<MutedHandler>,
    step: f32,
    curve: VolumeCurve,
    orientation: Orientation,
    size: Option<AudioSize>,
    disabled: Option<bool>,
    config: AudioConfig,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl VolumeControl {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            default_value: 1.0,
            on_value_change: None,
            on_value_committed: None,
            muted: None,
            default_muted: false,
            on_muted_change: None,
            step: 0.05,
            curve: VolumeCurve::Perceptual,
            orientation: Orientation::Horizontal,
            size: None,
            disabled: None,
            config: AudioConfig::default(),
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// Volume, 0..1, like `HTMLMediaElement.volume` (controlled).
    pub fn value(mut self, volume: f32) -> Self {
        self.value = Some(volume);
        self
    }

    /// Default 1.
    pub fn default_value(mut self, volume: f32) -> Self {
        self.default_value = volume;
        self
    }

    pub fn on_value_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    pub fn on_value_committed(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_value_committed = Some(Rc::new(f));
        self
    }

    /// Muted (controlled).
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    pub fn default_muted(mut self, muted: bool) -> Self {
        self.default_muted = muted;
        self
    }

    pub fn on_muted_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_muted_change = Some(Rc::new(f));
        self
    }

    /// Slider step, in position. Default 0.05.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// How slider position maps to volume. Default `Perceptual`.
    pub fn curve(mut self, curve: VolumeCurve) -> Self {
        self.curve = curve;
        self
    }

    /// Default horizontal. Vertical stacks the slider above the mute button.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Default: the config's, else `Default`.
    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Size and disabled from a surrounding container (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }
}

impl ParentElement for VolumeControl {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for VolumeControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for VolumeControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size.or(self.config.size).unwrap_or_default();
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let default_volume = clamp(self.default_value, 0.0, 1.0);
        let default_muted = self.default_muted;
        let first_volume = self.value.unwrap_or(default_volume);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| VolumeState {
            volume: default_volume,
            muted: default_muted,
            last_audible: initial_last_audible(first_volume),
            thumb_focus: cx.focus_handle().tab_stop(true),
            mute_focus: cx.focus_handle().tab_stop(true),
            dragging: false,
            drag_offset: 0.0,
            hovered: false,
            ring: ThumbRing::default(),
            track: Rc::new(Cell::new(None)),
        });
        let (volume, muted) = state.update(cx, |s, _| {
            if let Some(v) = self.value {
                s.volume = v;
            }
            if let Some(m) = self.muted {
                s.muted = m;
            }
            if disabled {
                s.dragging = false;
            }
            (s.volume, s.muted)
        });
        let (thumb_size, track_size) = match size {
            AudioSize::Sm => (12.0, 3.0),
            AudioSize::Default => (14.0, 4.0),
            AudioSize::Lg => (16.0, 6.0),
        };
        let ctx = VolumeCtx {
            state,
            volume,
            muted,
            level: volume_level_for(volume, muted),
            position: self.curve.to_position(volume),
            step: self.step,
            curve: self.curve,
            orientation: self.orientation,
            disabled,
            thumb_size,
            track_size,
            volume_controlled: self.value.is_some(),
            muted_controlled: self.muted.is_some(),
            on_value_change: self.on_value_change,
            on_value_committed: self.on_value_committed,
            on_muted_change: self.on_muted_change,
        };
        let children = if self.children.is_empty() {
            vec![
                VolumeControlMute::new().into_any_element(),
                VolumeControlSlider::new().into_any_element(),
            ]
        } else {
            self.children
        };
        let mut root = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .when(self.orientation == Orientation::Vertical, |el| {
                el.flex_col_reverse()
            })
            .when(disabled, |el| el.opacity(0.5))
            .role(gpui::Role::Group)
            .aria_label("Volume")
            .children(children);
        root.style().refine(&self.style);
        provide(ctx, root)
    }
}

type MuteContent = Rc<dyn Fn(VolumeControlMuteState) -> AnyElement>;

/// The mute toggle: "Mute" / "Unmute" text by default, or your icon. Use
/// [`VolumeControlMute::content`] to pick an icon from the level, like the
/// React demo's `group-data-[level=...]` classes.
#[derive(IntoElement)]
pub struct VolumeControlMute {
    content: Option<MuteContent>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl VolumeControlMute {
    pub fn new() -> Self {
        Self {
            content: None,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// Children chosen from the current level and muted state.
    pub fn content(mut self, f: impl Fn(VolumeControlMuteState) -> AnyElement + 'static) -> Self {
        self.content = Some(Rc::new(f));
        self
    }
}

impl Default for VolumeControlMute {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for VolumeControlMute {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for VolumeControlMute {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for VolumeControlMute {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = volume_ctx("VolumeControlMute");
        let theme = cx.theme().clone();
        let label = if ctx.muted { "Unmute" } else { "Mute" };
        let focus = ctx.state.read(cx).mute_focus.clone();
        let focus_visible =
            !ctx.disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let mut children = self.children;
        if let Some(content) = &self.content {
            children.push(content(VolumeControlMuteState {
                level: ctx.level,
                muted: ctx.muted,
            }));
        }
        let text_only = children.is_empty();
        let (muted_bg, foreground) = (theme.muted, theme.foreground);
        let click = ctx.clone();
        let keys = ctx.clone();
        let mut el = div()
            .id("volume-control-mute")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(28.0))
            .rounded(radius_lg(&theme))
            .text_color(theme.muted_foreground)
            .role(gpui::Role::Button)
            .aria_label(label)
            .aria_toggled(if ctx.muted {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .map(|el| {
                if text_only {
                    el.px_2().child(div().text_xs_().child(label))
                } else {
                    el.w(px(28.0)).children(children)
                }
            })
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
                el.track_focus(&focus)
                    .hover(move |s| s.bg(muted_bg).text_color(foreground))
                    .on_click(move |_, window, cx| click.toggle_muted(window, cx))
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            keys.toggle_muted(window, cx);
                        }
                    })
            });
        el.style().refine(&self.style);
        el
    }
}

/// The volume slider (shows 0 while muted; moving it unmutes).
#[derive(IntoElement)]
pub struct VolumeControlSlider {
    style: StyleRefinement,
}

impl VolumeControlSlider {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Default for VolumeControlSlider {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for VolumeControlSlider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for VolumeControlSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = volume_ctx("VolumeControlSlider");
        let theme = cx.theme().clone();
        let horizontal = ctx.horizontal();
        let shown = if ctx.muted { 0.0 } else { ctx.position };
        let thumb = ctx.thumb_size;
        let half = px(thumb / 2.0);
        let (track_cell, dragging, focus) = {
            let s = ctx.state.read(cx);
            (s.track.clone(), s.dragging, s.thumb_focus.clone())
        };
        let focus_visible =
            !ctx.disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let (t, animating) = ctx.state.update(cx, |s, _| {
            s.ring.update(!ctx.disabled && (s.hovered || focus_visible))
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
            .id("volume-control-hit")
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
                    press.start_drag(0.0, window, cx);
                    press.drag_to(e.position, window, cx);
                })
            });

        let text = if ctx.muted {
            "Muted".to_string()
        } else {
            format!("{}%", js_round((ctx.volume * PERCENT) as f64))
        };
        let thumb_el = div()
            .id("volume-control-thumb")
            .absolute()
            .left(-half)
            .top(-half)
            .size(px(thumb))
            .rounded_full()
            .bg(theme.background)
            .shadow(thumb_shadows(&theme, t, focus_visible))
            .role(gpui::Role::Slider)
            .aria_label("Volume")
            .aria_value(text);
        let thumb_el = if ctx.disabled {
            thumb_el
        } else {
            let hover = ctx.state.clone();
            let down = ctx.clone();
            let keys = ctx.clone();
            thumb_el
                .track_focus(&focus)
                .on_hover(move |hovered, _, cx| {
                    hover.update(cx, |s, cx| {
                        s.hovered = *hovered;
                        cx.notify();
                    });
                })
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let offset = down.state.read(cx).track.get().map_or(0.0, |track| {
                        let shown = down.shown(cx);
                        if down.horizontal() {
                            e.position.x.as_f32()
                                - (track.origin.x.as_f32() + shown * track.size.width.as_f32())
                        } else {
                            e.position.y.as_f32()
                                - (track.bottom().as_f32() - shown * track.size.height.as_f32())
                        }
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
            .map(|el| {
                if horizontal {
                    el.left(relative(shown)).top(relative(0.5))
                } else {
                    el.bottom(relative(shown)).left(relative(0.5))
                }
            })
            .child(thumb_el);

        let indicator = div().absolute().rounded_full().bg(theme.primary).map(|el| {
            if horizontal {
                el.left_0().top_0().h_full().w(relative(shown))
            } else {
                el.left_0().bottom_0().w_full().h(relative(shown))
            }
        });

        let track = div()
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
            .child(indicator)
            .child(anchor);

        let control = div()
            .relative()
            .flex()
            .items_center()
            .map(|el| {
                if horizontal {
                    el.h(px(thumb)).w_full().px(half)
                } else {
                    el.w(px(thumb)).h_full().flex_col().py(half)
                }
            })
            .child(hit)
            .child(track);

        let mut el = div()
            .relative()
            .flex()
            .items_center()
            .map(|el| {
                if horizontal {
                    el.w_full().min_w(px(80.0))
                } else {
                    el.h(px(96.0)).flex_col()
                }
            })
            .child(control);
        el.style().refine(&self.style);
        el
    }
}

/// The volume as a percentage of slider travel: `w-9 text-end font-mono text-xs`.
#[derive(IntoElement)]
pub struct VolumeControlValue {
    style: StyleRefinement,
}

impl VolumeControlValue {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Default for VolumeControlValue {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for VolumeControlValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for VolumeControlValue {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = volume_ctx("VolumeControlValue");
        let theme = cx.theme();
        let text: SharedString = volume_value_text(ctx.position, ctx.muted).into();
        let mut el = div()
            .flex()
            .flex_none()
            .justify_end()
            .w(px(36.0))
            .font_family(theme.font_mono.clone())
            .text_xs_()
            .text_color(theme.muted_foreground)
            .child(text);
        el.style().refine(&self.style);
        el
    }
}

/// `VolumeControlValue`'s text.
fn volume_value_text(position: f32, muted: bool) -> String {
    if muted {
        "0%".into()
    } else {
        format!("{}%", js_round((position * PERCENT) as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_control_mutes_and_restores_the_last_audible_volume() {
        // `defaultValue={0}`: muted level; Mute, then Unmute restores 1.
        let last = initial_last_audible(0.0);
        assert_eq!(volume_level_for(0.0, false), VolumeLevel::Muted);
        let (restore, muted) = toggle_muted(0.0, false, last);
        assert_eq!((restore, muted), (None, true));
        let (restore, muted) = toggle_muted(0.0, muted, last);
        assert_eq!((restore, muted), (Some(1.0), false));
        // Unmuting at a non-zero volume keeps it.
        assert_eq!(toggle_muted(0.4, true, 0.7), (None, false));
    }

    #[test]
    fn volume_control_levels() {
        assert_eq!(volume_level_for(0.0, false), VolumeLevel::Muted);
        assert_eq!(volume_level_for(0.8, true), VolumeLevel::Muted);
        assert_eq!(volume_level_for(0.2, false), VolumeLevel::Low);
        assert_eq!(volume_level_for(0.5, false), VolumeLevel::Medium);
        assert_eq!(volume_level_for(0.67, false), VolumeLevel::High);
    }

    #[test]
    fn volume_control_curves_round_trip() {
        let c = VolumeCurve::Perceptual;
        assert!((c.to_position(0.25) - 0.5).abs() < 1e-6);
        assert!((c.to_volume(0.5) - 0.25).abs() < 1e-6);
        assert_eq!(VolumeCurve::Linear.to_position(0.3), 0.3);
    }

    #[test]
    fn volume_control_value_text() {
        assert_eq!(volume_value_text(0.6f32.sqrt(), false), "77%");
        assert_eq!(volume_value_text(0.5, true), "0%");
    }

    #[test]
    fn volume_control_snaps_to_step() {
        assert_eq!(snap(0.62, 0.05), 0.6);
        assert_eq!(snap(0.63, 0.05), 0.65);
        assert_eq!(snap(1.2, 0.05), 1.0);
    }
}
