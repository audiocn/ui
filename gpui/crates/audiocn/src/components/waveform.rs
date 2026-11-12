//! Port of `components/ui/waveform.tsx`.
//!
//! `Waveform` is the root (seeking, hover, keyboard); `WaveformCanvas`,
//! `WaveformCursor`, `WaveformHover`, `WaveformRegion` and `WaveformMarker`
//! are its parts and read the root through context, exactly like the React
//! `WaveformContext`. Without children the root shows a canvas and a cursor.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, DispatchPhase, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, canvas, div,
    prelude::FluentBuilder, px, relative,
};

use super::live_waveform::{CanvasShape, paint_canvas_shapes};
use crate::audio::bands::resample_levels;
use crate::audio::decibels::clamp64;
use crate::audio::frame_source::FrameSource;
use crate::audio::time::{FormatTimeOptions, format_time};
use crate::theme::ActiveTheme;
use crate::ui::Skeleton;
use crate::ui::styles::{provide, ring_overlay, use_context};

type SeekHandler = Rc<dyn Fn(f64, &mut Window, &mut App)>;
type RegionHandler = Rc<dyn Fn(WaveformRegionValue, &mut Window, &mut App)>;
type FormatFn = Rc<dyn Fn(f64) -> String>;

/// `variant` of [`Waveform`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WaveformVariant {
    /// Bars from the bottom (default).
    #[default]
    Bars,
    /// A filled outline, mirrored around the middle.
    Line,
    /// Bars centred on the middle.
    Mirror,
}

/// What the parts read from the root (`WaveformContextValue`).
#[derive(Clone)]
struct WaveformContext {
    peaks: Option<Arc<[f32]>>,
    duration: f64,
    variant: WaveformVariant,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
    loading: bool,
    /// Current progress, 0..1.
    progress: f32,
    /// The hovered time, or `None`.
    hover: Option<f64>,
    /// Whether pointer seeking (and so the hover line) is on.
    interactive: bool,
    /// The root's bounds, from the last paint.
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// `--waveform`, `--waveform-progress`, `--waveform-cursor`.
    color: Hsla,
    progress_color: Hsla,
    cursor_color: Hsla,
}

impl WaveformContext {
    fn time_to_position(&self, value: f64) -> f32 {
        time_to_position(value, self.duration)
    }
}

fn time_to_position(value: f64, duration: f64) -> f32 {
    if duration > 0.0 {
        clamp64(value / duration, 0.0, 1.0) as f32
    } else {
        0.0
    }
}

fn use_waveform(part: &str) -> WaveformContext {
    use_context::<WaveformContext>()
        .unwrap_or_else(|| panic!("{part} must be used inside Waveform."))
}

/// `seekTarget`: where a key moves the playhead, if anywhere.
fn seek_target(key: &str, current: f64, amount: f64, duration: f64) -> Option<f64> {
    match key {
        "down" | "left" => Some(current - amount),
        "up" | "right" => Some(current + amount),
        "end" => Some(duration),
        "home" => Some(0.0),
        _ => None,
    }
}

/// `isInteractive`.
fn is_interactive(interactive: bool, disabled: bool, loading: bool, duration: f64) -> bool {
    interactive && !disabled && !loading && duration > 0.0
}

/// The canvas shapes for `levels` in a `width` x `height` canvas (`drawPass`).
fn waveform_shapes(
    levels: &[f32],
    height: f32,
    variant: WaveformVariant,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
) -> Vec<CanvasShape> {
    let pitch = bar_width + bar_gap;
    let middle = height / 2.0;
    if variant == WaveformVariant::Line {
        let x_at = |index: usize| index as f32 * pitch + bar_width / 2.0;
        let mut points = Vec::with_capacity(levels.len() * 2);
        for (index, level) in levels.iter().enumerate() {
            points.push((x_at(index), middle - level * (middle - 1.0)));
        }
        for (index, level) in levels.iter().enumerate().rev() {
            points.push((x_at(index), middle + level * (middle - 1.0)));
        }
        return vec![CanvasShape::Line {
            points,
            closed: true,
            fill_alpha: Some(1.0),
            stroke: None,
        }];
    }
    levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let bar_height = (level * height).max(2.0);
            let y = if variant == WaveformVariant::Mirror {
                middle - bar_height / 2.0
            } else {
                height - bar_height
            };
            CanvasShape::Rect {
                x: index as f32 * pitch,
                y,
                width: bar_width,
                height: bar_height,
                radius: bar_radius,
                alpha: 1.0,
                clip: None,
            }
        })
        .collect()
}

/// The peaks fitted to the bars that fit in `width` (the canvas `resize`).
fn waveform_levels(peaks: Option<&[f32]>, width: f32, bar_width: f32, bar_gap: f32) -> Vec<f32> {
    let count = 1usize.max(((width + bar_gap) / (bar_width + bar_gap)).floor() as usize);
    let mut levels = vec![0.0; count];
    if let Some(peaks) = peaks.filter(|p| !p.is_empty()) {
        resample_levels(peaks, 0, peaks.len(), &mut levels);
    }
    levels
}

// ---------------------------------------------------------------------------
// WaveformCanvas
// ---------------------------------------------------------------------------

/// The peaks, drawn twice: unplayed in `--waveform`, then played (up to the
/// playhead) in `--waveform-progress`. Shows a pulsing skeleton while loading.
#[derive(IntoElement, Default)]
pub struct WaveformCanvas {
    style: StyleRefinement,
}

impl WaveformCanvas {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for WaveformCanvas {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for WaveformCanvas {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_waveform("WaveformCanvas");
        let radius = cx.theme().radius_lg();
        let mut layer = div().absolute().top_0().left_0().size_full();
        layer.style().refine(&self.style);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .when(ctx.loading, |el| {
                el.child(
                    Skeleton::new()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(radius),
                )
            })
            .child(
                layer.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            if ctx.loading {
                                return;
                            }
                            let Some(peaks) = ctx.peaks.as_deref() else {
                                return;
                            };
                            let width = bounds.size.width.as_f32();
                            let levels =
                                waveform_levels(Some(peaks), width, ctx.bar_width, ctx.bar_gap);
                            let shapes = waveform_shapes(
                                &levels,
                                bounds.size.height.as_f32(),
                                ctx.variant,
                                ctx.bar_width,
                                ctx.bar_gap,
                                ctx.bar_radius,
                            );
                            paint_canvas_shapes(
                                bounds,
                                &shapes,
                                ctx.color,
                                None,
                                Some((0.0, width)),
                                window,
                            );
                            paint_canvas_shapes(
                                bounds,
                                &shapes,
                                ctx.progress_color,
                                None,
                                Some((0.0, ctx.progress * width)),
                                window,
                            );
                        },
                    )
                    .size_full(),
                ),
            )
    }
}

// ---------------------------------------------------------------------------
// WaveformCursor
// ---------------------------------------------------------------------------

/// The playhead: a 2px rounded line at the current position, in
/// `--waveform-cursor`. Hidden while loading.
#[derive(IntoElement, Default)]
pub struct WaveformCursor {
    style: StyleRefinement,
}

impl WaveformCursor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for WaveformCursor {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for WaveformCursor {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ctx = use_waveform("WaveformCursor");
        let mut el = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(ctx.progress))
            .ml(px(-1.0))
            .w(px(2.0))
            .rounded_full()
            .bg(ctx.cursor_color)
            .when(ctx.loading, |el| el.invisible());
        el.style().refine(&self.style);
        el
    }
}

// ---------------------------------------------------------------------------
// WaveformHover
// ---------------------------------------------------------------------------

/// A thin line under the pointer with the time above it. Only while the
/// waveform is interactive and hovered.
#[derive(IntoElement)]
pub struct WaveformHover {
    format: FormatFn,
    style: StyleRefinement,
}

impl Default for WaveformHover {
    fn default() -> Self {
        Self {
            format: Rc::new(|value| format_time(value, FormatTimeOptions::default())),
            style: StyleRefinement::default(),
        }
    }
}

impl WaveformHover {
    pub fn new() -> Self {
        Self::default()
    }

    /// Formats the hovered time. Default `format_time` (`m:ss`).
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
}

impl Styled for WaveformHover {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for WaveformHover {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_waveform("WaveformHover");
        let Some(hover) = visible_hover(ctx.hover, ctx.interactive) else {
            return div().into_any_element();
        };
        let t = cx.theme();
        let mut line = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(ctx.time_to_position(hover)))
            .w(px(1.0))
            .bg(t.foreground.opacity(0.4));
        line.style().refine(&self.style);
        line.child(
            // `absolute -top-6 left-1/2 -translate-x-1/2`: centred on the line.
            div()
                .absolute()
                .top(px(-24.0))
                .left(px(0.5))
                .w(px(0.0))
                .flex()
                .justify_center()
                .child(
                    div()
                        .flex_none()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(t.radius_md())
                        .bg(t.foreground)
                        .text_color(t.background)
                        .font_family(t.font_mono.clone())
                        .text_size(px(10.0))
                        .line_height(px(15.0))
                        .whitespace_nowrap()
                        .child((self.format)(hover)),
                ),
        )
        .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// WaveformRegion
// ---------------------------------------------------------------------------

/// A region's start and end, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveformRegionValue {
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegionDragKind {
    Start,
    End,
    Move,
}

#[derive(Clone, Copy, Debug)]
struct RegionDrag {
    kind: RegionDragKind,
    x: f32,
    start: f64,
    end: f64,
}

/// `move` during a region drag: the next value for a pointer at `x`.
fn region_drag_value(
    drag: &RegionDrag,
    x: f32,
    seconds_per_pixel: f64,
    duration: f64,
    min_length: f64,
) -> WaveformRegionValue {
    let delta = f64::from(x - drag.x) * seconds_per_pixel;
    match drag.kind {
        RegionDragKind::Move => {
            let length = drag.end - drag.start;
            let start = clamp64(drag.start + delta, 0.0, duration - length);
            WaveformRegionValue {
                start,
                end: start + length,
            }
        }
        RegionDragKind::Start => WaveformRegionValue {
            start: clamp64(drag.start + delta, 0.0, drag.end - min_length),
            end: drag.end,
        },
        RegionDragKind::End => WaveformRegionValue {
            start: drag.start,
            end: clamp64(drag.end + delta, drag.start + min_length, duration),
        },
    }
}

/// `nudge`: arrow keys move one edge by 0.1 s (1 s with Shift).
fn region_nudge(
    edge: RegionDragKind,
    key: &str,
    shift: bool,
    value: WaveformRegionValue,
    duration: f64,
    min_length: f64,
) -> Option<WaveformRegionValue> {
    let amount = if shift { 1.0 } else { 0.1 };
    let delta = match key {
        "right" | "up" => amount,
        "left" | "down" => -amount,
        _ => return None,
    };
    Some(if edge == RegionDragKind::Start {
        WaveformRegionValue {
            start: clamp64(value.start + delta, 0.0, value.end - min_length),
            end: value.end,
        }
    } else {
        WaveformRegionValue {
            start: value.start,
            end: clamp64(value.end + delta, value.start + min_length, duration),
        }
    })
}

struct RegionState {
    drag: Option<RegionDrag>,
    start_focus: FocusHandle,
    end_focus: FocusHandle,
}

/// A selected span (a loop, a clip) over the waveform. Drag it to move it,
/// drag (or arrow-key) its edges to resize it. Controlled: report changes with
/// [`on_value_change`](Self::on_value_change) and pass the new `start`/`end`.
///
/// ```ignore
/// WaveformRegion::new("clip", region.start, region.end)
///     .on_value_change(move |value, _, cx| state.update(cx, |s, cx| { *s = value; cx.notify() }))
/// ```
#[derive(IntoElement)]
pub struct WaveformRegion {
    id: ElementId,
    start: f64,
    end: f64,
    on_value_change: Option<RegionHandler>,
    resizable: bool,
    draggable: bool,
    min_length: f64,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl WaveformRegion {
    pub fn new(id: impl Into<ElementId>, start: f64, end: f64) -> Self {
        Self {
            id: id.into(),
            start,
            end,
            on_value_change: None,
            resizable: true,
            draggable: true,
            min_length: 0.1,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn on_value_change(
        mut self,
        handler: impl Fn(WaveformRegionValue, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(handler));
        self
    }

    /// Drag the edges. Default true.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Drag the whole region. Default true.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Shortest region in seconds. Default 0.1.
    pub fn min_length(mut self, min_length: f64) -> Self {
        self.min_length = min_length;
        self
    }
}

impl ParentElement for WaveformRegion {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for WaveformRegion {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn begin_region_drag(
    state: &Entity<RegionState>,
    kind: RegionDragKind,
    start: f64,
    end: f64,
    x: f32,
    cx: &mut App,
) {
    state.update(cx, |s, cx| {
        s.drag = Some(RegionDrag {
            kind,
            x,
            start,
            end,
        });
        cx.notify();
    });
}

impl RenderOnce for WaveformRegion {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_waveform("WaveformRegion");
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<RegionState>| {
                RegionState {
                    drag: None,
                    start_focus: cx.focus_handle().tab_stop(true),
                    end_focus: cx.focus_handle().tab_stop(true),
                }
            });
        let t = cx.theme().clone();
        let (drag, start_focus, end_focus) = {
            let s = state.read(cx);
            (s.drag, s.start_focus.clone(), s.end_focus.clone())
        };
        let (start, end) = (self.start, self.end);
        let duration = ctx.duration;
        let min_length = self.min_length;
        let value = WaveformRegionValue { start, end };
        let left = ctx.time_to_position(start);
        let width = ctx.time_to_position(end) - left;
        let on_change = self.on_value_change.clone();
        let keyboard = window.last_input_was_keyboard();

        let handle = |kind: RegionDragKind, focus: FocusHandle, window: &Window| {
            let focus_visible = keyboard && focus.is_focused(window);
            let down_state = state.clone();
            let down_focus = focus.clone();
            let key_change = on_change.clone();
            let hit = div()
                .id((
                    self.id.clone(),
                    if kind == RegionDragKind::Start {
                        "start"
                    } else {
                        "end"
                    },
                ))
                .absolute()
                .top_0()
                .bottom_0()
                .w(px(20.0))
                .cursor(CursorStyle::ResizeLeftRight)
                .track_focus(&focus)
                .on_mouse_down(
                    MouseButton::Left,
                    move |event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        window.focus(&down_focus, cx);
                        begin_region_drag(
                            &down_state,
                            kind,
                            start,
                            end,
                            event.position.x.as_f32(),
                            cx,
                        );
                    },
                )
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let next = region_nudge(
                        kind,
                        event.keystroke.key.as_str(),
                        event.keystroke.modifiers.shift,
                        value,
                        duration,
                        min_length,
                    );
                    if let Some(next) = next {
                        cx.stop_propagation();
                        if let Some(handler) = key_change.as_ref() {
                            handler(next, window, cx);
                        }
                    }
                })
                .child(
                    // `w-2 rounded-sm bg-primary/60`, centred in the 20px hit area
                    // (`after:-inset-x-1.5`).
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(6.0))
                        .w(px(8.0))
                        .rounded(t.radius_sm())
                        .bg(if focus_visible {
                            t.primary
                        } else {
                            t.primary.opacity(0.6)
                        })
                        .when(focus_visible, |el| {
                            el.child(ring_overlay(t.radius_sm(), 3.0, t.ring.opacity(0.3)))
                        }),
                );
            if kind == RegionDragKind::Start {
                hit.left(px(-10.0))
            } else {
                hit.right(px(-10.0))
            }
        };
        let handles = self.resizable.then(|| {
            [
                handle(RegionDragKind::Start, start_focus.clone(), window),
                handle(RegionDragKind::End, end_focus.clone(), window),
            ]
        });

        let capture_state = state.clone();
        let capture_change = self.on_value_change.clone();
        let root_bounds = ctx.bounds.clone();
        let down_state = state.clone();
        let draggable = self.draggable;
        let moving = drag.is_some_and(|d| d.kind == RegionDragKind::Move);
        let mut el = div()
            .id(self.id.clone())
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(left))
            .w(relative(width))
            .rounded(t.radius_sm())
            .bg(t.primary.opacity(0.15))
            .child(ring_overlay(t.radius_sm(), 1.0, t.primary.opacity(0.4)))
            .when(draggable, |el| {
                el.cursor(if moving {
                    CursorStyle::ClosedHand
                } else {
                    CursorStyle::OpenHand
                })
            })
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                if draggable {
                    let x = event.position.x.as_f32();
                    begin_region_drag(&down_state, RegionDragKind::Move, start, end, x, cx);
                }
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let Some(drag) = drag else {
                            return;
                        };
                        let moved = capture_change.clone();
                        let bounds = root_bounds.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            cx.stop_propagation();
                            let width = bounds.get().map_or(1.0, |b| b.size.width.as_f32());
                            let seconds_per_pixel = duration / f64::from(width.max(1.0));
                            let next = region_drag_value(
                                &drag,
                                event.position.x.as_f32(),
                                seconds_per_pixel,
                                duration,
                                min_length,
                            );
                            if let Some(handler) = moved.as_ref() {
                                handler(next, window, cx);
                            }
                        });
                        let released = capture_state.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                                return;
                            }
                            cx.stop_propagation();
                            released.update(cx, |s, cx| {
                                s.drag = None;
                                cx.notify();
                            });
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .children(self.children)
            .children(handles.into_iter().flatten());
        el.style().refine(&self.style);
        el
    }
}

// ---------------------------------------------------------------------------
// WaveformMarker
// ---------------------------------------------------------------------------

/// A 1px line at `time` in `--meter-warn`, with an optional label (children).
#[derive(IntoElement)]
pub struct WaveformMarker {
    time: f64,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl WaveformMarker {
    pub fn new(time: f64) -> Self {
        Self {
            time,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl ParentElement for WaveformMarker {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for WaveformMarker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for WaveformMarker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_waveform("WaveformMarker");
        let t = cx.theme();
        let has_label = !self.children.is_empty();
        let mut el = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(ctx.time_to_position(self.time)))
            .w(px(1.0))
            .bg(t.meter_warn);
        el.style().refine(&self.style);
        el.when(has_label, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(4.0))
                    .flex_none()
                    .rounded(t.radius_sm())
                    .px(px(4.0))
                    .bg(t.meter_warn.opacity(0.2))
                    .text_color(t.foreground)
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .children(self.children),
            )
        })
    }
}

// ---------------------------------------------------------------------------
// Waveform
// ---------------------------------------------------------------------------

/// `timeAtPointer`: the time under window x `x` in a waveform at `bounds`.
fn time_at_x(bounds: Option<Bounds<Pixels>>, x: Pixels, duration: f64) -> f64 {
    let Some(bounds) = bounds else {
        return 0.0;
    };
    let width = bounds.size.width.as_f32();
    if width <= 0.0 {
        return 0.0;
    }
    let position = clamp64(f64::from((x - bounds.origin.x).as_f32() / width), 0.0, 1.0);
    position * duration
}

/// The hover line shows only while seeking is on.
fn visible_hover(hover: Option<f64>, interactive: bool) -> Option<f64> {
    hover.filter(|_| interactive)
}

struct WaveformState {
    internal_time: f64,
    /// The last `shownTime` (for the effect that writes it to the playhead).
    shown_time: Option<f64>,
    /// `latestTimeRef`: what the playhead shows.
    latest_time: f64,
    time_source: Option<FrameSource<f64>>,
    time_version: Option<u64>,
    hover: Option<f64>,
    dragging: bool,
    focus: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl WaveformState {
    /// `timeAtPointer`.
    fn time_at(&self, x: Pixels, duration: f64) -> f64 {
        time_at_x(self.bounds.get(), x, duration)
    }

    /// Follows `shownTime` and the `time` source into `latest_time`.
    fn sync(&mut self, shown_time: f64, time: Option<&FrameSource<f64>>) {
        if self.shown_time != Some(shown_time) {
            self.shown_time = Some(shown_time);
            self.latest_time = shown_time;
        }
        if self.time_source.as_ref() != time {
            self.time_source = time.cloned();
            self.time_version = None;
        }
        if let Some(source) = time {
            let version = source.version();
            if self.time_version != Some(version) {
                self.time_version = Some(version);
                if let Some(value) = source.latest() {
                    self.latest_time = value;
                }
            }
        }
    }
}

/// Props the event handlers need.
#[derive(Clone)]
struct SeekProps {
    duration: f64,
    controlled: bool,
    on_seek: Option<SeekHandler>,
    on_seek_committed: Option<SeekHandler>,
}

/// `seekTo`.
fn seek_to(
    state: &Entity<WaveformState>,
    props: &SeekProps,
    value: f64,
    commit: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let next = clamp64(value, 0.0, props.duration);
    state.update(cx, |s, cx| {
        s.latest_time = next;
        if !props.controlled {
            s.internal_time = next;
        }
        cx.notify();
    });
    if let Some(on_seek) = props.on_seek.as_ref() {
        on_seek(next, window, cx);
    }
    if commit && let Some(on_commit) = props.on_seek_committed.as_ref() {
        on_commit(next, window, cx);
    }
}

fn set_hover(state: &Entity<WaveformState>, hover: Option<f64>, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.hover != hover {
            s.hover = hover;
            cx.notify();
        }
    });
}

/// A static waveform of a clip with a smooth playhead, seeking (click, drag,
/// arrow keys), hover time, regions and markers.
///
/// ```ignore
/// Waveform::new("track")
///     .peaks(data.peaks.clone())
///     .duration(data.duration)
///     .time(player.time.clone())
///     .on_seek_committed(|t, _, _| player.seek(t))
///     .child(WaveformCanvas::new())
///     .child(WaveformCursor::new())
///     .child(WaveformHover::new())
/// ```
#[derive(IntoElement)]
pub struct Waveform {
    id: ElementId,
    peaks: Option<Arc<[f32]>>,
    duration: f64,
    current_time: Option<f64>,
    default_current_time: f64,
    time: Option<FrameSource<f64>>,
    on_seek: Option<SeekHandler>,
    on_seek_committed: Option<SeekHandler>,
    step: f64,
    large_step: f64,
    variant: WaveformVariant,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
    interactive: bool,
    loading: bool,
    disabled: bool,
    color: Option<Hsla>,
    progress_color: Option<Hsla>,
    cursor_color: Option<Hsla>,
    aria_label: Option<SharedString>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Waveform {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            peaks: None,
            duration: 0.0,
            current_time: None,
            default_current_time: 0.0,
            time: None,
            on_seek: None,
            on_seek_committed: None,
            step: 5.0,
            large_step: 15.0,
            variant: WaveformVariant::Bars,
            bar_width: 2.0,
            bar_gap: 1.0,
            bar_radius: 1.0,
            interactive: true,
            loading: false,
            disabled: false,
            color: None,
            progress_color: None,
            cursor_color: None,
            aria_label: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Peaks, 0..1, from `waveform_data` (`useWaveformData`). Unset is `null`.
    pub fn peaks(mut self, peaks: impl Into<Arc<[f32]>>) -> Self {
        self.peaks = Some(peaks.into());
        self
    }

    /// Peaks that may not be there yet (`peaks: ... | null`).
    pub fn peaks_option(mut self, peaks: Option<Arc<[f32]>>) -> Self {
        self.peaks = peaks;
        self
    }

    /// Length in seconds.
    pub fn duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    /// Playhead position in seconds (controlled).
    pub fn current_time(mut self, current_time: f64) -> Self {
        self.current_time = Some(current_time);
        self
    }

    /// Initial playhead position when uncontrolled. Default 0.
    pub fn default_current_time(mut self, default_current_time: f64) -> Self {
        self.default_current_time = default_current_time;
        self
    }

    /// A smooth playhead: the position in seconds, read every frame.
    pub fn time(mut self, time: FrameSource<f64>) -> Self {
        self.time = Some(time);
        self
    }

    /// Fires while seeking.
    pub fn on_seek(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_seek = Some(Rc::new(handler));
        self
    }

    /// Fires when the pointer is released or after keyboard seeking.
    pub fn on_seek_committed(
        mut self,
        handler: impl Fn(f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_seek_committed = Some(Rc::new(handler));
        self
    }

    /// Seconds per arrow key. Default 5.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Seconds per Shift+arrow. Default 15.
    pub fn large_step(mut self, large_step: f64) -> Self {
        self.large_step = large_step;
        self
    }

    /// Default `Bars`.
    pub fn variant(mut self, variant: WaveformVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Bar width in pixels. Default 2.
    pub fn bar_width(mut self, bar_width: f32) -> Self {
        self.bar_width = bar_width;
        self
    }

    /// Gap in pixels. Default 1.
    pub fn bar_gap(mut self, bar_gap: f32) -> Self {
        self.bar_gap = bar_gap;
        self
    }

    /// Corner radius in pixels. Default 1.
    pub fn bar_radius(mut self, bar_radius: f32) -> Self {
        self.bar_radius = bar_radius;
        self
    }

    /// When false, the waveform is display only. Default true.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `--waveform`: the unplayed colour. Default `muted-foreground`.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// `--waveform-progress`: the played colour. Default `primary`.
    pub fn progress_color(mut self, color: impl Into<Hsla>) -> Self {
        self.progress_color = Some(color.into());
        self
    }

    /// `--waveform-cursor`: the playhead colour. Default `foreground`.
    pub fn cursor_color(mut self, color: impl Into<Hsla>) -> Self {
        self.cursor_color = Some(color.into());
        self
    }

    /// `aria-label` (kept for parity; GPUI has no accessibility tree yet).
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl ParentElement for Waveform {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Waveform {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Waveform {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default_time = self.default_current_time;
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<WaveformState>| {
                WaveformState {
                    internal_time: default_time,
                    shown_time: None,
                    latest_time: default_time,
                    time_source: None,
                    time_version: None,
                    hover: None,
                    dragging: false,
                    focus: cx.focus_handle().tab_stop(true),
                    bounds: Rc::new(Cell::new(None)),
                }
            });
        let time = self.time.clone();
        let current_time = self.current_time;
        state.update(cx, |s, _| {
            let shown = current_time.unwrap_or(s.internal_time);
            s.sync(shown, time.as_ref());
        });
        // The time source can't wake us: follow it every frame.
        if self.time.is_some() {
            window.request_animation_frame();
        }

        let duration = self.duration;
        let active = is_interactive(self.interactive, self.disabled, self.loading, duration);
        let (latest_time, hover, dragging, focus, bounds) = {
            let s = state.read(cx);
            (
                s.latest_time,
                s.hover,
                s.dragging,
                s.focus.clone(),
                s.bounds.clone(),
            )
        };
        let t = cx.theme().clone();
        let ctx = WaveformContext {
            peaks: self.peaks.clone(),
            duration,
            variant: self.variant,
            bar_width: self.bar_width,
            bar_gap: self.bar_gap,
            bar_radius: self.bar_radius,
            loading: self.loading,
            progress: time_to_position(latest_time, duration),
            hover,
            interactive: active,
            bounds: bounds.clone(),
            color: self.color.unwrap_or(t.muted_foreground),
            progress_color: self.progress_color.unwrap_or(t.primary),
            cursor_color: self.cursor_color.unwrap_or(t.foreground),
        };
        let props = SeekProps {
            duration,
            controlled: self.current_time.is_some(),
            on_seek: self.on_seek.clone(),
            on_seek_committed: self.on_seek_committed.clone(),
        };

        let children: Vec<AnyElement> = if self.children.is_empty() {
            vec![
                WaveformCanvas::new().into_any_element(),
                WaveformCursor::new().into_any_element(),
            ]
        } else {
            self.children
        };

        let focus_visible = active && window.last_input_was_keyboard() && focus.is_focused(window);
        let capture_state = state.clone();
        let capture_props = props.clone();
        let leave_state = state.clone();
        let mut root = div()
            .id(self.id.clone())
            .relative()
            .h(px(80.0))
            .w_full()
            .rounded(t.radius_lg())
            .when(self.disabled, |el| el.opacity(0.5));
        root.style().refine(&self.style);
        let root = root
            .child(
                canvas(
                    {
                        let bounds = bounds.clone();
                        move |b, _, _| bounds.set(Some(b))
                    },
                    move |_, _, window, _| {
                        if !(dragging && active) {
                            return;
                        }
                        // Pointer capture: follow the pointer anywhere until release.
                        let moved = capture_state.clone();
                        let move_props = capture_props.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let at = moved
                                .read(cx)
                                .time_at(event.position.x, move_props.duration);
                            set_hover(&moved, Some(at), cx);
                            seek_to(&moved, &move_props, at, false, window, cx);
                        });
                        let released = capture_state.clone();
                        let up_props = capture_props.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                                return;
                            }
                            let (at, inside) = released.update(cx, |s, cx| {
                                s.dragging = false;
                                cx.notify();
                                let inside =
                                    s.bounds.get().is_some_and(|b| b.contains(&event.position));
                                (s.time_at(event.position.x, up_props.duration), inside)
                            });
                            seek_to(&released, &up_props, at, true, window, cx);
                            if !inside {
                                // The pointer left while captured.
                                set_hover(&released, None, cx);
                            }
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(provide(
                ctx,
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .children(children),
            ))
            .when(focus_visible, |el| {
                el.child(ring_overlay(t.radius_lg(), 3.0, t.ring.opacity(0.3)))
            })
            // Leaving clears the hover even while seeking is off, so turning
            // interactivity off mid-hover doesn't leave a stale line.
            .on_hover(move |hovered, _, cx| {
                if !hovered {
                    set_hover(&leave_state, None, cx);
                }
            });
        if !active {
            return root;
        }

        let step = self.step;
        let large_step = self.large_step;
        let key_state = state.clone();
        let key_props = props.clone();
        let down_state = state.clone();
        let down_props = props.clone();
        let move_state = state.clone();
        root.track_focus(&focus)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let current = key_state.read(cx).latest_time;
                let amount = if event.keystroke.modifiers.shift {
                    large_step
                } else {
                    step
                };
                if let Some(next) = seek_target(
                    event.keystroke.key.as_str(),
                    current,
                    amount,
                    key_props.duration,
                ) {
                    cx.stop_propagation();
                    seek_to(&key_state, &key_props, next, true, window, cx);
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    let focus = down_state.read(cx).focus.clone();
                    window.focus(&focus, cx);
                    let at = down_state.update(cx, |s, _| {
                        s.dragging = true;
                        s.time_at(event.position.x, down_props.duration)
                    });
                    seek_to(&down_state, &down_props, at, false, window, cx);
                },
            )
            .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                let s = move_state.read(cx);
                if s.dragging {
                    return;
                }
                let at = s.time_at(event.position.x, duration);
                set_hover(&move_state, Some(at), cx);
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEAKS: [f32; 5] = [0.2, 0.6, 1.0, 0.6, 0.2];

    #[test]
    fn leaving_clears_the_hover_line_even_while_seeking_is_off() {
        let bounds = Some(Bounds::new(
            gpui::point(px(0.0), px(0.0)),
            gpui::size(px(200.0), px(80.0)),
        ));
        let interactive = |loading| is_interactive(true, false, loading, 100.0);
        // The pointer moves to the middle of a 200px, 100 s waveform.
        let mut hover = Some(time_at_x(bounds, px(100.0), 100.0));
        assert_eq!(visible_hover(hover, interactive(false)), Some(50.0));
        // A new track starts loading under the pointer: seeking turns off.
        assert_eq!(visible_hover(hover, interactive(true)), None);
        // Leaving clears the hover even while seeking is off.
        hover = None;
        assert_eq!(visible_hover(hover, interactive(false)), None);
        // Pointer positions clamp to the waveform.
        assert_eq!(time_at_x(bounds, px(-20.0), 100.0), 0.0);
        assert_eq!(time_at_x(bounds, px(400.0), 100.0), 100.0);
    }

    #[test]
    fn interactive_needs_a_duration_and_no_loading_or_disabled() {
        assert!(is_interactive(true, false, false, 10.0));
        assert!(!is_interactive(false, false, false, 10.0));
        assert!(!is_interactive(true, true, false, 10.0));
        assert!(!is_interactive(true, false, true, 10.0));
        assert!(!is_interactive(true, false, false, 0.0));
    }

    #[test]
    fn keys_move_the_playhead() {
        assert_eq!(seek_target("right", 10.0, 5.0, 60.0), Some(15.0));
        assert_eq!(seek_target("up", 10.0, 15.0, 60.0), Some(25.0));
        assert_eq!(seek_target("left", 10.0, 5.0, 60.0), Some(5.0));
        assert_eq!(seek_target("down", 10.0, 5.0, 60.0), Some(5.0));
        assert_eq!(seek_target("home", 10.0, 5.0, 60.0), Some(0.0));
        assert_eq!(seek_target("end", 10.0, 5.0, 60.0), Some(60.0));
        assert_eq!(seek_target("a", 10.0, 5.0, 60.0), None);
    }

    #[test]
    fn time_to_position_clamps_and_handles_no_duration() {
        assert_eq!(time_to_position(15.0, 60.0), 0.25);
        assert_eq!(time_to_position(-1.0, 60.0), 0.0);
        assert_eq!(time_to_position(90.0, 60.0), 1.0);
        assert_eq!(time_to_position(5.0, 0.0), 0.0);
    }

    #[test]
    fn fits_peaks_into_the_bars_that_fit() {
        // 14px with 2px bars and 1px gaps: floor(15 / 3) = 5 bars.
        let levels = waveform_levels(Some(&PEAKS), 14.0, 2.0, 1.0);
        assert_eq!(levels, PEAKS.to_vec());
        // No peaks: zeros, still at least one bar.
        assert_eq!(waveform_levels(None, 0.0, 2.0, 1.0), vec![0.0]);
    }

    #[test]
    fn draws_bars_from_the_bottom_or_the_middle() {
        let shapes = waveform_shapes(&[0.5, 0.0], 80.0, WaveformVariant::Bars, 2.0, 1.0, 1.0);
        assert_eq!(
            shapes[0],
            CanvasShape::Rect {
                x: 0.0,
                y: 40.0,
                width: 2.0,
                height: 40.0,
                radius: 1.0,
                alpha: 1.0,
                clip: None
            }
        );
        // Silent bars keep 2px.
        match &shapes[1] {
            CanvasShape::Rect { x, y, height, .. } => {
                assert_eq!((*x, *y, *height), (3.0, 78.0, 2.0));
            }
            CanvasShape::Line { .. } => panic!("expected a bar"),
        }
        let mirror = waveform_shapes(&[0.5], 80.0, WaveformVariant::Mirror, 2.0, 1.0, 1.0);
        match &mirror[0] {
            CanvasShape::Rect { y, height, .. } => assert_eq!((*y, *height), (20.0, 40.0)),
            CanvasShape::Line { .. } => panic!("expected a bar"),
        }
    }

    #[test]
    fn draws_the_line_variant_as_a_filled_outline() {
        let shapes = waveform_shapes(&[1.0, 0.5], 80.0, WaveformVariant::Line, 2.0, 1.0, 1.0);
        let CanvasShape::Line {
            points,
            fill_alpha,
            stroke,
            ..
        } = &shapes[0]
        else {
            panic!("expected a line");
        };
        assert_eq!(*fill_alpha, Some(1.0));
        assert_eq!(*stroke, None);
        assert_eq!(
            points,
            &vec![(1.0, 1.0), (4.0, 20.5), (4.0, 59.5), (1.0, 79.0)]
        );
    }

    #[test]
    fn region_drags_move_and_resize_within_limits() {
        let drag = |kind| RegionDrag {
            kind,
            x: 100.0,
            start: 9.0,
            end: 22.0,
        };
        // 60 s over 200 px: 0.3 s per px.
        let moved = region_drag_value(&drag(RegionDragKind::Move), 110.0, 0.3, 60.0, 0.1);
        assert!((moved.start - 12.0).abs() < 1e-9 && (moved.end - 25.0).abs() < 1e-9);
        let pinned = region_drag_value(&drag(RegionDragKind::Move), 400.0, 0.3, 60.0, 0.1);
        assert_eq!((pinned.start, pinned.end), (47.0, 60.0));
        let start = region_drag_value(&drag(RegionDragKind::Start), 400.0, 0.3, 60.0, 0.1);
        assert!((start.start - 21.9).abs() < 1e-9);
        assert_eq!(start.end, 22.0);
        let end = region_drag_value(&drag(RegionDragKind::End), 0.0, 0.3, 60.0, 0.1);
        assert!((end.end - 9.1).abs() < 1e-9);
    }

    #[test]
    fn region_edges_nudge_with_arrow_keys() {
        let value = WaveformRegionValue {
            start: 9.0,
            end: 22.0,
        };
        let next = region_nudge(RegionDragKind::Start, "right", false, value, 60.0, 0.1).unwrap();
        assert!((next.start - 9.1).abs() < 1e-9);
        let next = region_nudge(RegionDragKind::End, "down", true, value, 60.0, 0.1).unwrap();
        assert!((next.end - 21.0).abs() < 1e-9);
        assert!(region_nudge(RegionDragKind::End, "home", false, value, 60.0, 0.1).is_none());
    }
}
