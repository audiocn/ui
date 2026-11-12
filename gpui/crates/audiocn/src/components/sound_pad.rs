//! Port of `components/ui/sound-pad.tsx`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, ElementId, FocusHandle, FontWeight, Hsla,
    InputPreference, InteractiveElement, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton,
    MouseMoveEvent, ParentElement, PathBuilder, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Window, canvas, div, fill,
    point, prelude::FluentBuilder, px, size,
};

use crate::audio::frame_source::FrameSource;
use crate::theme::{ActiveTheme, mix};
use crate::ui::Kbd;
use crate::ui::icons::Icon;
use crate::ui::styles::{TextSize, provide, radius_xl, ring_overlay, use_context};

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// `SoundPadMode`: how presses map to trigger and stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundPadMode {
    /// Every press triggers.
    #[default]
    OneShot,
    /// A press while playing stops.
    Toggle,
    /// Plays while held; release stops.
    Hold,
    /// Like `Toggle`, for looping sounds.
    Loop,
}

/// What a press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadAction {
    Trigger,
    Stop,
}

/// The action of a press in `mode` (`press`).
pub fn press_action(mode: SoundPadMode, playing: bool) -> PadAction {
    if matches!(mode, SoundPadMode::Toggle | SoundPadMode::Loop) && playing {
        PadAction::Stop
    } else {
        PadAction::Trigger
    }
}

/// The action of a release in `mode` (`release`).
pub fn release_action(mode: SoundPadMode) -> Option<PadAction> {
    (mode == SoundPadMode::Hold).then_some(PadAction::Stop)
}

/// Default `--pad-min-width` (5.5rem).
pub const DEFAULT_PAD_MIN_WIDTH: f32 = 88.0;
/// Default `--pad-gap` (0.5rem).
pub const DEFAULT_PAD_GAP: f32 = 8.0;

/// The column count of a [`SoundPadGrid`] `width` wide: up to `columns`,
/// never narrower than `min_width` (`repeat(auto-fill, minmax(max(min,
/// (100% - (columns - 1) * gap) / columns), 1fr))`).
pub fn grid_columns(width: f32, columns: usize, gap: f32, min_width: f32) -> usize {
    let columns = columns.max(1) as f32;
    let track = min_width.max((width - (columns - 1.0) * gap) / columns);
    let fit = ((width + gap) / (track + gap) + 1e-4).floor();
    fit.max(1.0) as usize
}

#[derive(Clone)]
struct PadHandlers {
    press: Handler,
    release: Handler,
}

struct PadEntry {
    focus: FocusHandle,
    enabled: bool,
}

#[derive(Clone)]
struct SoundPadGridContext {
    pads: Rc<RefCell<HashMap<String, PadHandlers>>>,
    order: Rc<RefCell<Vec<PadEntry>>>,
}

/// `hotkeyScope` of [`SoundPadGrid`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HotkeyScope {
    /// Hotkeys while focus is inside the grid.
    #[default]
    Focus,
    /// Hotkeys anywhere in the window, never while typing.
    Global,
}

struct SoundPadGridState {
    focus: FocusHandle,
    pads: Rc<RefCell<HashMap<String, PadHandlers>>>,
    held: Rc<RefCell<HashSet<String>>>,
    width: Rc<Cell<Option<Pixels>>>,
    intercept: Option<Subscription>,
}

/// Presses the pad for `key`, if any (`handleDown`). Returns whether it did.
fn handle_down(
    pads: &RefCell<HashMap<String, PadHandlers>>,
    held: &RefCell<HashSet<String>>,
    key: &str,
    repeat: bool,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if repeat {
        return false;
    }
    let key = key.to_lowercase();
    let Some(pad) = pads.borrow().get(&key).cloned() else {
        return false;
    };
    held.borrow_mut().insert(key);
    (pad.press)(window, cx);
    true
}

/// Releases the pad held with `key` (`handleUp`).
fn handle_up(
    pads: &RefCell<HashMap<String, PadHandlers>>,
    held: &RefCell<HashSet<String>>,
    key: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let key = key.to_lowercase();
    if !held.borrow_mut().remove(&key) {
        return;
    }
    let pad = pads.borrow().get(&key).cloned();
    if let Some(pad) = pad {
        (pad.release)(window, cx);
    }
}

/// A responsive grid of [`SoundPad`]s with hotkeys and arrow-key focus.
///
/// ```ignore
/// SoundPadGrid::new("pads").columns(4).hotkeys(true)
///     .child(SoundPad::new("airhorn").hotkey("1").child(SoundPadLabel::new().child("Airhorn")))
/// ```
#[derive(IntoElement)]
pub struct SoundPadGrid {
    id: ElementId,
    columns: usize,
    hotkeys: bool,
    hotkey_scope: HotkeyScope,
    pad_min_width: Pixels,
    pad_gap: Pixels,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SoundPadGrid {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            columns: 4,
            hotkeys: false,
            hotkey_scope: HotkeyScope::Focus,
            pad_min_width: px(DEFAULT_PAD_MIN_WIDTH),
            pad_gap: px(DEFAULT_PAD_GAP),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// The most columns. The grid drops columns to keep pads at least
    /// `pad_min_width` wide. Default 4.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns;
        self
    }

    /// Listen for pad hotkeys. Default false.
    pub fn hotkeys(mut self, hotkeys: bool) -> Self {
        self.hotkeys = hotkeys;
        self
    }

    /// `Global` listens in the whole window, never while typing. Default `Focus`.
    pub fn hotkey_scope(mut self, scope: HotkeyScope) -> Self {
        self.hotkey_scope = scope;
        self
    }

    /// `--pad-min-width`. Default 88px (5.5rem).
    pub fn pad_min_width(mut self, width: Pixels) -> Self {
        self.pad_min_width = width;
        self
    }

    /// `--pad-gap`. Default 8px.
    pub fn pad_gap(mut self, gap: Pixels) -> Self {
        self.pad_gap = gap;
        self
    }
}

impl ParentElement for SoundPadGrid {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for SoundPadGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SoundPadGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(
            self.id.clone(),
            cx,
            |_, cx: &mut Context<SoundPadGridState>| SoundPadGridState {
                focus: cx.focus_handle(),
                pads: Rc::new(RefCell::new(HashMap::new())),
                held: Rc::new(RefCell::new(HashSet::new())),
                width: Rc::new(Cell::new(None)),
                intercept: None,
            },
        );
        let (focus, pads, held, width_cell) = {
            let s = state.read(cx);
            (
                s.focus.clone(),
                s.pads.clone(),
                s.held.clone(),
                s.width.clone(),
            )
        };
        // Pads register again while they render; keep the old handlers to
        // release a held pad that went away (disabled, loading, removed).
        let previous: HashMap<String, PadHandlers> = pads.borrow_mut().drain().collect();
        let order = Rc::new(RefCell::new(Vec::new()));
        let context = SoundPadGridContext {
            pads: pads.clone(),
            order: order.clone(),
        };

        let hotkeys = self.hotkeys;
        let scope = self.hotkey_scope;
        let global = hotkeys && scope == HotkeyScope::Global;
        state.update(cx, |s, cx| {
            if !global {
                s.intercept = None;
            } else if s.intercept.is_none() {
                let (pads, held, focus) = (s.pads.clone(), s.held.clone(), s.focus.clone());
                s.intercept = Some(cx.intercept_keystrokes(move |event, window, cx| {
                    let keystroke = &event.keystroke;
                    let modifiers = keystroke.modifiers;
                    // Inside the grid the grid's own listeners see repeats and key-ups.
                    if modifiers.platform
                        || modifiers.control
                        || modifiers.alt
                        || event.input_preference == InputPreference::CharacterInput
                        || focus.contains_focused(window, cx)
                    {
                        return;
                    }
                    if handle_down(&pads, &held, &keystroke.key, false, window, cx) {
                        // Without a window-wide key-up, a hotkey press is a tap.
                        handle_up(&pads, &held, &keystroke.key, window, cx);
                        cx.stop_propagation();
                    }
                }));
            }
        });

        let columns = self.columns;
        let gap = self.pad_gap;
        let min_width = self.pad_min_width;
        let cols = width_cell.get().map_or(columns.max(1), |w| {
            grid_columns(f32::from(w), columns, f32::from(gap), f32::from(min_width))
        });

        let release_gone = {
            let pads = pads.clone();
            let held = held.clone();
            canvas(
                move |bounds, window, cx| {
                    if width_cell.get() != Some(bounds.size.width) {
                        width_cell.set(Some(bounds.size.width));
                        window.refresh();
                    }
                    let gone: Vec<String> = held
                        .borrow()
                        .iter()
                        .filter(|key| !pads.borrow().contains_key(*key))
                        .cloned()
                        .collect();
                    for key in gone {
                        held.borrow_mut().remove(&key);
                        if let Some(pad) = previous.get(&key).cloned() {
                            window.defer(cx, move |window, cx| (pad.release)(window, cx));
                        }
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let (down_pads, down_held) = (pads.clone(), held.clone());
        let (up_pads, up_held) = (pads, held);
        let mut el = div()
            .id(self.id)
            .relative()
            .grid()
            .grid_cols(cols as u16)
            .gap(gap)
            .role(gpui::Role::Group)
            .track_focus(&focus)
            .children(self.children)
            .child(release_gone)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if window.default_prevented() {
                    return;
                }
                let keystroke = &event.keystroke;
                let modifiers = keystroke.modifiers;
                let hotkey_pressed = hotkeys
                    && scope == HotkeyScope::Focus
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt
                    && handle_down(
                        &down_pads,
                        &down_held,
                        &keystroke.key,
                        event.is_held,
                        window,
                        cx,
                    );
                if hotkey_pressed {
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                // moveFocus
                let order = order.borrow();
                let Some(index) = order.iter().position(|pad| pad.focus.is_focused(window)) else {
                    return;
                };
                let per_row = cols.min(order.len()).max(1) as isize;
                let offset = match keystroke.key.as_str() {
                    "down" => per_row,
                    "left" => -1,
                    "right" => 1,
                    "up" => -per_row,
                    _ => return,
                };
                let next = (index as isize + offset).clamp(0, order.len() as isize - 1) as usize;
                window.prevent_default();
                cx.stop_propagation();
                if order[next].enabled {
                    window.focus(&order[next].focus, cx);
                }
            })
            .on_key_up(move |event: &KeyUpEvent, window, cx| {
                if scope == HotkeyScope::Focus || global {
                    handle_up(&up_pads, &up_held, &event.keystroke.key, window, cx);
                }
            });
        el.style().refine(&self.style);
        provide(context, el)
    }
}

// ---------------------------------------------------------------------------
// SoundPad
// ---------------------------------------------------------------------------

/// `variant` of [`SoundPad`] (`soundPadVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundPadVariant {
    /// Tinted with the accent.
    #[default]
    Default,
    Ghost,
    Outline,
}

/// `size` of [`SoundPad`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundPadSize {
    #[default]
    Default,
    Sm,
    Lg,
}

impl SoundPadSize {
    /// (min height, padding)
    fn metrics(self) -> (f32, f32) {
        match self {
            Self::Default => (96.0, 12.0),
            Self::Lg => (128.0, 16.0),
            Self::Sm => (64.0, 8.0),
        }
    }
}

#[derive(Clone)]
struct SoundPadContext {
    hotkey: Option<SharedString>,
    playing: bool,
    accent: Hsla,
    radius: Pixels,
    has_ring: Rc<Cell<bool>>,
}

struct SoundPadState {
    focus: FocusHandle,
    pressed: bool,
    has_ring: Rc<Cell<bool>>,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

/// A trigger pad: click, tap or a hotkey plays a sound; `mode` decides what a
/// second press or a release does. Shows playing, loading and progress.
///
/// ```ignore
/// SoundPad::new("airhorn")
///     .accent(oklch(0.7, 0.2, 30.0))
///     .hotkey("1")
///     .playing(sound.playing)
///     .on_trigger(|_, _| sound.play())
///     .child(SoundPadLabel::new().child("Airhorn"))
///     .child(SoundPadShortcut::new())
/// ```
#[derive(IntoElement)]
pub struct SoundPad {
    id: ElementId,
    on_trigger: Option<Handler>,
    on_stop: Option<Handler>,
    playing: bool,
    mode: SoundPadMode,
    hotkey: Option<SharedString>,
    loading: bool,
    accent: Option<Hsla>,
    variant: SoundPadVariant,
    size: SoundPadSize,
    disabled: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SoundPad {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            on_trigger: None,
            on_stop: None,
            playing: false,
            mode: SoundPadMode::OneShot,
            hotkey: None,
            loading: false,
            accent: None,
            variant: SoundPadVariant::Default,
            size: SoundPadSize::Default,
            disabled: false,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn on_trigger(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_trigger = Some(Rc::new(f));
        self
    }

    pub fn on_stop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(f));
        self
    }

    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    /// How presses map to trigger and stop. Default `OneShot`.
    pub fn mode(mut self, mode: SoundPadMode) -> Self {
        self.mode = mode;
        self
    }

    /// Active when the grid enables hotkeys.
    pub fn hotkey(mut self, hotkey: impl Into<SharedString>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    /// The sound is not ready yet.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// The pad colour (`--pad-accent`). Default `primary`.
    pub fn accent(mut self, accent: Hsla) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn variant(mut self, variant: SoundPadVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: SoundPadSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl ParentElement for SoundPad {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for SoundPad {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn run(handler: &Option<Handler>, window: &mut Window, cx: &mut App) {
    if let Some(handler) = handler {
        handler(window, cx);
    }
}

impl RenderOnce for SoundPad {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let grid = use_context::<SoundPadGridContext>();
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<SoundPadState>| {
                SoundPadState {
                    focus: cx.focus_handle().tab_stop(true),
                    pressed: false,
                    has_ring: Rc::new(Cell::new(false)),
                    bounds: Rc::new(Cell::new(None)),
                }
            });
        let (focus, pressed, has_ring, bounds_cell) = {
            let s = state.read(cx);
            (
                s.focus.clone(),
                s.pressed,
                s.has_ring.clone(),
                s.bounds.clone(),
            )
        };
        has_ring.set(false);
        let inactive = self.disabled || self.loading;
        let mode = self.mode;
        let playing = self.playing;

        let press: Handler = {
            let state = state.clone();
            let (on_trigger, on_stop) = (self.on_trigger.clone(), self.on_stop.clone());
            Rc::new(move |window, cx| {
                state.update(cx, |s, cx| {
                    s.pressed = true;
                    cx.notify();
                });
                match press_action(mode, playing) {
                    PadAction::Stop => run(&on_stop, window, cx),
                    PadAction::Trigger => run(&on_trigger, window, cx),
                }
            })
        };
        let release: Handler = {
            let state = state.clone();
            let on_stop = self.on_stop.clone();
            Rc::new(move |window, cx| {
                state.update(cx, |s, cx| {
                    s.pressed = false;
                    cx.notify();
                });
                if release_action(mode).is_some() {
                    run(&on_stop, window, cx);
                }
            })
        };

        if let Some(grid) = &grid {
            grid.order.borrow_mut().push(PadEntry {
                focus: focus.clone(),
                enabled: !inactive,
            });
            if let Some(hotkey) = &self.hotkey
                && !inactive
            {
                grid.pads.borrow_mut().insert(
                    hotkey.to_lowercase(),
                    PadHandlers {
                        press: press.clone(),
                        release: release.clone(),
                    },
                );
            }
        }

        let t = cx.theme();
        let accent = self.accent.unwrap_or(t.primary);
        let radius = radius_xl(t);
        let (min_h, padding) = self.size.metrics();
        let (bg, hover_bg, border) = match self.variant {
            SoundPadVariant::Default => (
                Some(mix(t.muted, accent, 0.14)),
                mix(t.muted, accent, 0.22),
                None,
            ),
            SoundPadVariant::Ghost => (None, t.muted, None),
            SoundPadVariant::Outline => (None, t.muted, Some(t.border)),
        };
        let focus_visible =
            !inactive && focus.is_focused(window) && window.last_input_was_keyboard();
        let ring = if playing {
            Some((2.0, accent))
        } else if focus_visible {
            Some((3.0, t.ring.opacity(0.4)))
        } else {
            None
        };
        let opacity = if self.loading {
            Some(0.6)
        } else if self.disabled {
            Some(0.5)
        } else {
            None
        };
        let context = SoundPadContext {
            hotkey: self.hotkey.clone(),
            playing,
            accent,
            radius,
            has_ring,
        };
        let group: SharedString = format!("sound-pad-{}", self.id).into();

        let inner = div()
            .relative()
            .flex()
            .flex_1()
            .w_full()
            .flex_col()
            .items_start()
            .justify_between()
            .gap(px(8.0))
            .overflow_hidden()
            .rounded(radius)
            .p(px(padding))
            .text_left()
            .when_some(bg, |el, bg| el.bg(bg))
            .when_some(border, |el, border| el.border_1().border_color(border))
            .when(!inactive, |el| {
                el.group_hover(group.clone(), move |s| s.bg(hover_bg))
            })
            .children(self.children);

        let mut el = div()
            .id(self.id)
            .group(group)
            .relative()
            .flex()
            .flex_col()
            .min_h(px(min_h))
            .role(gpui::Role::Button)
            .when(
                matches!(mode, SoundPadMode::Toggle | SoundPadMode::Loop),
                |el| {
                    el.aria_toggled(if playing {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                },
            )
            .when_some(opacity, |el, o| el.opacity(o))
            .child(
                canvas(
                    move |bounds, _, _| bounds_cell.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(inner)
            .when_some(ring, |el, (width, color)| {
                el.child(ring_overlay(radius, width, color))
            });
        el.style().refine(&self.style);
        if inactive {
            return provide(context, el);
        }

        let holding = mode == SoundPadMode::Hold;
        // While pressed, sliding off the pad releases it (`onPointerLeave`).
        if pressed {
            let release = release.clone();
            let bounds = state.read(cx).bounds.clone();
            el = el.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase.bubble()
                                && bounds.get().is_some_and(|b| !b.contains(&event.position))
                            {
                                release(window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
        }

        let (down, up, key_down, key_up, click) = (
            press.clone(),
            release.clone(),
            press.clone(),
            release.clone(),
            press,
        );
        let mouse_focus = focus.clone();
        let click_state = state.clone();
        let el = el
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&mouse_focus, cx);
                down(window, cx);
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| up(window, cx))
            .on_click(move |event: &ClickEvent, window, cx| {
                // `event.detail === 0`: a keyboard click.
                if !holding && event.is_keyboard() {
                    click(window, cx);
                    click_state.update(cx, |s, _| s.pressed = false);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if holding && (key == "space" || key == "enter") && !event.is_held {
                    window.prevent_default();
                    key_down(window, cx);
                }
            })
            .on_key_up(move |event: &KeyUpEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if holding && (key == "space" || key == "enter") {
                    key_up(window, cx);
                }
            });
        provide(context, el)
    }
}

// ---------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------

/// The pad icon, in the accent colour (`size-5` icons).
///
/// ```ignore
/// SoundPadIcon::new().icon(IconName::Megaphone)
/// ```
#[derive(IntoElement, Default)]
pub struct SoundPadIcon {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SoundPadIcon {
    pub fn new() -> Self {
        Self::default()
    }

    /// An icon at `size-5` (20px).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.children
            .push(icon.into().size(px(20.0)).into_any_element());
        self
    }
}

impl ParentElement for SoundPadIcon {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for SoundPadIcon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SoundPadIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let accent = use_context::<SoundPadContext>().map_or(cx.theme().primary, |c| c.accent);
        let mut el = div()
            .relative()
            .flex()
            .text_color(accent)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The pad name (`mt-auto line-clamp-2 text-sm font-medium`).
///
/// ```ignore
/// SoundPadLabel::new().child("Airhorn")
/// ```
#[derive(IntoElement, Default)]
pub struct SoundPadLabel {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl SoundPadLabel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ParentElement for SoundPadLabel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for SoundPadLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SoundPadLabel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // `group-has-data-[variant=ring]:pe-6`: room for a ring progress.
        let ring = use_context::<SoundPadContext>().is_some_and(|c| c.has_ring.get());
        let mut label = div()
            .relative()
            .line_clamp(2)
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .when(ring, |el| el.pr(px(24.0)))
            .children(self.children);
        label.style().refine(&self.style);
        // `mt-auto`, as a box that takes the free space and bottom-aligns the
        // text (Taffy also applies the pad's `justify-between` after an auto
        // margin, which pushes the label out of the pad).
        div().flex().flex_1().flex_col().justify_end().child(label)
    }
}

/// The hotkey as a key cap in the top-right corner. Shows the pad's hotkey
/// unless children are given; nothing without either.
///
/// ```ignore
/// SoundPadShortcut::new()
/// ```
#[derive(IntoElement, Default)]
pub struct SoundPadShortcut {
    label: Option<SharedString>,
    style: StyleRefinement,
}

impl SoundPadShortcut {
    pub fn new() -> Self {
        Self::default()
    }

    /// Text instead of the pad's hotkey.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Styled for SoundPadShortcut {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SoundPadShortcut {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let hotkey = use_context::<SoundPadContext>().and_then(|c| c.hotkey);
        let Some(text) = self.label.or(hotkey) else {
            return div().into_any_element();
        };
        let mut kbd = Kbd::new(text.to_uppercase())
            .absolute()
            .top(px(8.0))
            .right(px(8.0));
        kbd.style().refine(&self.style);
        kbd.into_any_element()
    }
}

/// `variant` of [`SoundPadProgress`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundPadProgressVariant {
    /// A 4px bar along the bottom.
    #[default]
    Bar,
    /// The whole pad fills, at 20% opacity.
    Fill,
    /// A 20px ring in the bottom-right corner.
    Ring,
}

/// Playback progress: a bar, a fill or a ring in the accent colour. Reads a
/// smooth `source` every frame while the pad plays, or a declarative `value`.
///
/// ```ignore
/// SoundPadProgress::new().source(sound.progress_source()).variant(SoundPadProgressVariant::Ring)
/// ```
#[derive(IntoElement, Default)]
pub struct SoundPadProgress {
    value: Option<f32>,
    source: Option<FrameSource<f32>>,
    variant: SoundPadProgressVariant,
    style: StyleRefinement,
}

impl SoundPadProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// 0..1, for declarative use.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// A smooth progress source.
    pub fn source(mut self, source: FrameSource<f32>) -> Self {
        self.source = Some(source);
        self
    }

    /// Default `Bar`.
    pub fn variant(mut self, variant: SoundPadProgressVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl Styled for SoundPadProgress {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How far a rounded-rect bottom edge rises at `x` (corner radius `r`, width `w`).
fn corner_rise(x: f32, w: f32, r: f32) -> f32 {
    let d = if x < r {
        r - x
    } else if x > w - r {
        x - (w - r)
    } else {
        return 0.0;
    };
    r - (r * r - d * d).max(0.0).sqrt()
}

/// The bottom strip of a rounded pad (height `h`), filled to `fraction`,
/// clipped to the pad's rounded corners.
fn paint_bar(bounds: Bounds<Pixels>, fraction: f32, radius: f32, color: Hsla, window: &mut Window) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let r = radius.min(w / 2.0);
    let end = w * fraction;
    if end <= 0.0 || h <= 0.0 {
        return;
    }
    // The strip is visible where the corner rises less than its height.
    let start = if r > h {
        r - (r * r - (r - h) * (r - h)).sqrt()
    } else {
        0.0
    };
    let end = end.min(w - start);
    if end <= start {
        return;
    }
    let origin = bounds.origin;
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    let mut points = vec![at(start, 0.0), at(end, 0.0)];
    let steps = 16;
    for i in 0..=steps {
        let x = end - (end - start) * i as f32 / steps as f32;
        points.push(at(x, h - corner_rise(x, w, r).min(h)));
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// An annular sector (ring progress), clockwise from 12 o'clock.
fn paint_ring_sector(
    bounds: Bounds<Pixels>,
    thickness: f32,
    from: f32,
    to: f32,
    color: Hsla,
    window: &mut Window,
) {
    if to <= from {
        return;
    }
    let center = bounds.center();
    let outer = f32::from(bounds.size.width.min(bounds.size.height)) / 2.0;
    let inner = (outer - thickness).max(0.0);
    let steps = ((to - from) * 64.0).ceil().max(2.0) as usize;
    let angle = |t: f32| (from + (to - from) * t) * std::f32::consts::TAU;
    let at = |radius: f32, a: f32| {
        point(
            center.x + px(radius * a.sin()),
            center.y - px(radius * a.cos()),
        )
    };
    let mut points = Vec::with_capacity(steps * 2 + 2);
    for i in 0..=steps {
        points.push(at(outer, angle(i as f32 / steps as f32)));
    }
    for i in (0..=steps).rev() {
        points.push(at(inner, angle(i as f32 / steps as f32)));
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

impl RenderOnce for SoundPadProgress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pad = use_context::<SoundPadContext>();
        let playing = pad.as_ref().is_some_and(|p| p.playing);
        let accent = pad.as_ref().map_or(cx.theme().primary, |p| p.accent);
        let radius = pad.as_ref().map_or(px(14.0), |p| p.radius);
        let progress = match self.value {
            Some(value) => value.clamp(0.0, 1.0),
            None if !playing => 0.0,
            None => self
                .source
                .as_ref()
                .and_then(|s| s.latest())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
        };
        if playing && self.value.is_none() && self.source.is_some() {
            window.request_animation_frame();
        }
        let variant = self.variant;
        if variant == SoundPadProgressVariant::Ring
            && let Some(pad) = &pad
            && !pad.has_ring.replace(true)
        {
            // The label renders before us: lay it out again with room for the ring.
            window.refresh();
        }

        let painter = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| match variant {
                SoundPadProgressVariant::Ring => {
                    paint_ring_sector(
                        bounds,
                        3.0,
                        0.0,
                        1.0,
                        accent.opacity(accent.a * 0.2),
                        window,
                    );
                    paint_ring_sector(bounds, 3.0, 0.0, progress, accent, window);
                }
                SoundPadProgressVariant::Bar => {
                    paint_bar(bounds, progress, f32::from(radius), accent, window);
                }
                SoundPadProgressVariant::Fill => {
                    if progress > 0.0 {
                        let w = bounds.size.width * progress;
                        let right = (radius - (bounds.size.width - w)).max(px(0.0));
                        window.paint_quad(
                            fill(
                                Bounds::new(bounds.origin, size(w, bounds.size.height)),
                                accent.opacity(accent.a * 0.2),
                            )
                            .corner_radii(gpui::Corners {
                                top_left: radius,
                                bottom_left: radius,
                                top_right: right,
                                bottom_right: right,
                            }),
                        );
                    }
                }
            },
        )
        .size_full();

        let mut el = div().absolute().child(painter);
        el = match variant {
            SoundPadProgressVariant::Ring => el.right(px(8.0)).bottom(px(8.0)).size(px(20.0)),
            SoundPadProgressVariant::Bar => el.left_0().right_0().bottom_0().h(px(4.0)),
            SoundPadProgressVariant::Fill => el.left_0().right_0().top_0().bottom_0(),
        };
        el.style().refine(&self.style);
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_in_toggle_mode() {
        assert_eq!(
            press_action(SoundPadMode::Toggle, false),
            PadAction::Trigger
        );
        assert_eq!(press_action(SoundPadMode::Toggle, true), PadAction::Stop);
        assert_eq!(press_action(SoundPadMode::Loop, true), PadAction::Stop);
        assert_eq!(
            press_action(SoundPadMode::OneShot, true),
            PadAction::Trigger
        );
        assert_eq!(press_action(SoundPadMode::Hold, true), PadAction::Trigger);
    }

    #[test]
    fn stops_a_hold_pad_on_release() {
        assert_eq!(release_action(SoundPadMode::Hold), Some(PadAction::Stop));
        assert_eq!(release_action(SoundPadMode::OneShot), None);
        assert_eq!(release_action(SoundPadMode::Toggle), None);
    }

    #[test]
    fn caps_grid_columns_and_keeps_a_minimum_pad_width() {
        // Wide: never more than `columns`.
        assert_eq!(grid_columns(512.0, 4, 8.0, 88.0), 4);
        assert_eq!(grid_columns(2000.0, 4, 8.0, 88.0), 4);
        assert_eq!(grid_columns(500.0, 3, 8.0, 88.0), 3);
        // Narrow: drop columns to keep pads at least 88px.
        assert_eq!(grid_columns(300.0, 4, 8.0, 88.0), 3);
        assert_eq!(grid_columns(190.0, 4, 8.0, 88.0), 2);
        assert_eq!(grid_columns(50.0, 4, 8.0, 88.0), 1);
    }

    #[test]
    fn clips_the_bar_to_the_corners() {
        assert_eq!(corner_rise(50.0, 100.0, 14.0), 0.0);
        assert!((corner_rise(0.0, 100.0, 14.0) - 14.0).abs() < 1e-4);
        assert!((corner_rise(100.0, 100.0, 14.0) - 14.0).abs() < 1e-4);
    }
}
