//! Port of `components/ui/live-waveform.tsx`.
//!
//! The canvas drawing is split from GPUI: [`LiveWaveformPainter`] runs the
//! React `tick` (history playback, idle line, bars / mirror / line) and keeps
//! the shapes it drew; the element paints those shapes every frame. The
//! shared shape painter (`CanvasShape`, edge fades) also serves
//! `SmoothWaveform` and `Waveform`.

use std::sync::Arc;

use gpui::{
    App, Background, Bounds, ContentMask, ElementId, Hsla, IntoElement, ParentElement, Path,
    PathBuilder, PathStyle, Pixels, Refineable, RenderOnce, StyleRefinement, Styled, Window,
    canvas, div, fill, linear_color_stop, linear_gradient, point, px, size,
};
use parking_lot::Mutex;

use crate::audio::bands::resample_levels;
use crate::audio::decibels::clamp;
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::history_playback::{HistoryPlayback, create_history_playback};
use crate::audio::types::VisualFrame;
use crate::signal::reduced_motion::prefers_reduced_motion;

const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
const IDLE_ALPHA: f32 = 0.35;

// ---------------------------------------------------------------------------
// Shared canvas shapes (used by the three waveform components)
// ---------------------------------------------------------------------------

/// One shape of a waveform canvas, in pixels from the canvas's top-left.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasShape {
    /// `roundRect` + `fill` (or `fillRect` with radius 0) at `globalAlpha`
    /// `alpha`, optionally clipped to the x range `clip`.
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        alpha: f32,
        clip: Option<(f32, f32)>,
    },
    /// A polyline. When `fill_alpha` is set it is closed and filled at that
    /// alpha first; when `stroke` is set it is then stroked at full alpha with
    /// that width (closed if `closed`).
    Line {
        points: Vec<(f32, f32)>,
        closed: bool,
        fill_alpha: Option<f32>,
        stroke: Option<f32>,
    },
}

/// Canvas `destination-out` edge fades: the left `edge` pixels fade in from
/// transparent and the right `edge` pixels fade out, linearly.
#[derive(Clone, Copy, Debug)]
struct EdgeFade {
    width: f32,
    edge: f32,
}

impl EdgeFade {
    fn new(width: f32, fade_px: Option<f32>) -> Self {
        let edge = fade_px.map_or(0.0, |fade| fade.min(width / 2.0).max(0.0));
        Self { width, edge }
    }

    /// A background for a primitive spanning `x0..x1` (local pixels) that
    /// lies in one fade segment. Exact for each ramp (the gradient stops are
    /// placed on the canvas's ramp, relative to the primitive).
    /// `mid` (local px) picks the segment: the middle of the visible part.
    fn background(&self, color: Hsla, x0: f32, x1: f32, mid: f32) -> Background {
        let span = x1 - x0;
        if self.edge <= 0.0 || span <= f32::EPSILON {
            return color.into();
        }
        let clear = color.opacity(0.0);
        let stop = |at: f32| (at - x0) / span;
        if mid < self.edge {
            linear_gradient(
                90.0,
                linear_color_stop(clear, stop(0.0)),
                linear_color_stop(color, stop(self.edge)),
            )
        } else if mid > self.width - self.edge {
            linear_gradient(
                90.0,
                linear_color_stop(color, stop(self.width - self.edge)),
                linear_color_stop(clear, stop(self.width)),
            )
        } else {
            color.into()
        }
    }

    /// Paints a path in up to three clipped passes (left ramp, middle, right
    /// ramp) so each pass can carry its own gradient.
    fn paint_path(
        &self,
        path: Path<Pixels>,
        color: Hsla,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) {
        if self.edge <= 0.0 {
            window.paint_path(path, color);
            return;
        }
        let origin = bounds.origin.x.as_f32();
        let path_x0 = path.bounds.origin.x.as_f32() - origin;
        let path_x1 = path_x0 + path.bounds.size.width.as_f32();
        let segments = [
            (0.0, self.edge),
            (self.edge, self.width - self.edge),
            (self.width - self.edge, self.width),
        ];
        for (from, to) in segments {
            let x0 = path_x0.max(from);
            let x1 = path_x1.min(to);
            if x1 <= x0 {
                continue;
            }
            let background = self.background(color, x0, x1, (x0 + x1) / 2.0);
            let mask = clip_x(bounds, from, to);
            let path = path.clone();
            window.with_content_mask(Some(mask), |window| window.paint_path(path, background));
        }
    }
}

/// A content mask over the canvas, limited to the x range `from..to` (local px).
fn clip_x(bounds: Bounds<Pixels>, from: f32, to: f32) -> ContentMask<Pixels> {
    ContentMask {
        bounds: Bounds::new(
            point(bounds.origin.x + px(from), bounds.origin.y),
            size(px((to - from).max(0.0)), bounds.size.height),
        ),
    }
}

fn build_line(
    bounds: Bounds<Pixels>,
    points: &[(f32, f32)],
    closed: bool,
    builder: PathBuilder,
) -> Option<Path<Pixels>> {
    if points.len() < 2 {
        return None;
    }
    let mut builder = builder;
    let at = |(x, y): (f32, f32)| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    builder.move_to(at(points[0]));
    for &p in &points[1..] {
        builder.line_to(at(p));
    }
    if closed {
        builder.close();
    }
    builder.build().ok()
}

/// Paints canvas shapes in `color`, clipped to the canvas (and to the x range
/// `clip`, local px, when given), with the ends faded over `fade_px`.
///
/// Canvas line joins/caps are approximated: GPUI does not expose lyon's
/// round joins, so strokes use its default miter joins and butt caps.
pub(crate) fn paint_canvas_shapes(
    bounds: Bounds<Pixels>,
    shapes: &[CanvasShape],
    color: Hsla,
    fade_px: Option<f32>,
    clip: Option<(f32, f32)>,
    window: &mut Window,
) {
    let width = bounds.size.width.as_f32();
    let fade = EdgeFade::new(width, fade_px);
    let mask = match clip {
        Some((from, to)) => clip_x(bounds, from, to),
        None => ContentMask { bounds },
    };
    window.with_content_mask(Some(mask), |window| {
        for shape in shapes {
            match shape {
                CanvasShape::Rect {
                    x,
                    y,
                    width,
                    height,
                    radius,
                    alpha,
                    clip,
                } => {
                    if *width <= 0.0 || *height <= 0.0 {
                        continue;
                    }
                    let (vx0, vx1) = match clip {
                        Some((from, to)) => (x.max(*from), (x + width).min(*to)),
                        None => (*x, x + width),
                    };
                    if vx1 <= vx0 {
                        continue;
                    }
                    let background =
                        fade.background(color.opacity(*alpha), *x, x + width, (vx0 + vx1) / 2.0);
                    let quad = fill(
                        Bounds::new(
                            point(bounds.origin.x + px(*x), bounds.origin.y + px(*y)),
                            size(px(*width), px(*height)),
                        ),
                        background,
                    )
                    .corner_radii(px(radius.min(width / 2.0).min(height / 2.0).max(0.0)));
                    match clip {
                        Some((from, to)) => window
                            .with_content_mask(Some(clip_x(bounds, *from, *to)), |window| {
                                window.paint_quad(quad)
                            }),
                        None => window.paint_quad(quad),
                    }
                }
                CanvasShape::Line {
                    points,
                    closed,
                    fill_alpha,
                    stroke,
                } => {
                    if let Some(alpha) = fill_alpha {
                        let builder = PathBuilder::fill().with_style(PathStyle::Fill(
                            gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
                        ));
                        if let Some(path) = build_line(bounds, points, true, builder) {
                            fade.paint_path(path, color.opacity(*alpha), bounds, window);
                        }
                    }
                    if let Some(line_width) = stroke
                        && *line_width > 0.0
                        && let Some(path) = build_line(
                            bounds,
                            points,
                            *closed,
                            PathBuilder::stroke(px(*line_width)),
                        )
                    {
                        fade.paint_path(path, color, bounds, window);
                    }
                }
            }
        }
    });
}

/// Ordered paint/clear calls from an actions handle, drained by the component.
#[derive(Clone, Debug)]
pub(crate) enum ActionOp {
    Paint(VisualFrame),
    Clear,
}

#[derive(Clone, Default)]
pub(crate) struct ActionQueue(Arc<Mutex<Vec<ActionOp>>>);

impl ActionQueue {
    pub(crate) fn push(&self, op: ActionOp) {
        self.0.lock().push(op);
    }

    pub(crate) fn drain(&self) -> Vec<ActionOp> {
        std::mem::take(&mut *self.0.lock())
    }
}

impl PartialEq for ActionQueue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Tracks a frame source: returns `Some(frame)` when it emitted (or was
/// swapped) since the last call.
#[derive(Default)]
pub(crate) struct SourceCursor {
    source: Option<FrameSource<VisualFrame>>,
    version: Option<u64>,
}

impl SourceCursor {
    /// Follows `source`. Returns `(changed_source, new_frame)`: whether the
    /// source itself changed, and the latest frame when it has a new one.
    pub(crate) fn poll(
        &mut self,
        source: Option<&FrameSource<VisualFrame>>,
    ) -> (bool, Option<Option<VisualFrame>>) {
        let changed = self.source.as_ref() != source;
        if changed {
            self.source = source.cloned();
            self.version = None;
        }
        let Some(source) = source else {
            return (changed, None);
        };
        let version = source.version();
        if self.version == Some(version) {
            return (changed, None);
        }
        self.version = Some(version);
        if version == 0 {
            return (changed, None);
        }
        (changed, Some(source.latest()))
    }
}

// ---------------------------------------------------------------------------
// LiveWaveform
// ---------------------------------------------------------------------------

/// `mode` of [`LiveWaveform`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiveWaveformMode {
    /// The level history, scrolling in from the right.
    Scrolling,
    /// The current frame (default).
    #[default]
    Static,
}

/// `variant` of [`LiveWaveform`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiveWaveformVariant {
    /// Vertical bars (default).
    #[default]
    Bars,
    /// The waveform trace.
    Line,
    /// Bars mirrored around the centre.
    Mirror,
}

/// The imperative handle of a [`LiveWaveform`] (`actionsRef`): paint frames
/// directly, or clear. Create one, pass it with [`LiveWaveform::actions`],
/// and keep a clone. Calls apply in order on the next frame.
///
/// ```ignore
/// let actions = LiveWaveformActions::new();
/// LiveWaveform::new("trace").actions(&actions);
/// actions.paint(frame);
/// ```
#[derive(Clone, Default, PartialEq)]
pub struct LiveWaveformActions {
    queue: ActionQueue,
}

impl LiveWaveformActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint a frame directly.
    pub fn paint(&self, frame: VisualFrame) {
        self.queue.push(ActionOp::Paint(frame));
    }

    /// Clear the canvas and forget the last frame.
    pub fn clear(&self) {
        self.queue.push(ActionOp::Clear);
    }
}

/// Drawing options of a [`LiveWaveform`] (its props).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrawOptions {
    mode: LiveWaveformMode,
    variant: LiveWaveformVariant,
    bar_width: f32,
    bar_gap: f32,
    bar_radius: f32,
    min_bar_height: f32,
    line_width: f32,
    fade_edges: bool,
    fade_width: f32,
    active: bool,
    sensitivity: f32,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self {
            mode: LiveWaveformMode::Static,
            variant: LiveWaveformVariant::Bars,
            bar_width: 3.0,
            bar_gap: 1.0,
            bar_radius: 1.5,
            min_bar_height: 4.0,
            line_width: 1.5,
            fade_edges: true,
            fade_width: 24.0,
            active: true,
            sensitivity: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Size {
    width: f32,
    height: f32,
}

fn history_levels(frame: &VisualFrame, count: usize) -> Vec<f32> {
    let available = frame.history_length.min(count);
    let size = frame.history.len();
    let include_previous =
        available == size && available < count && frame.history_previous_level.is_some();
    let offset = usize::from(include_previous);
    let mut out = vec![0.0; available + offset];
    if include_previous {
        out[0] = frame.history_previous_level.unwrap_or(0.0);
    }
    if size == 0 {
        return out;
    }
    let first = frame.history_start + frame.history_length - available;
    for index in 0..available {
        out[index + offset] = frame.history[(first + index) % size];
    }
    out
}

fn levels_for(frame: &VisualFrame, count: usize, options: &DrawOptions) -> Vec<f32> {
    if options.mode == LiveWaveformMode::Scrolling {
        return history_levels(frame, count + 1);
    }
    if options.variant != LiveWaveformVariant::Mirror {
        let mut out = vec![0.0; count];
        resample_levels(&frame.bands, 0, frame.bands.len(), &mut out);
        return out;
    }
    let mut half = vec![0.0; count.div_ceil(2)];
    resample_levels(&frame.bands, 0, frame.bands.len(), &mut half);
    let center = (count as f32 - 1.0) / 2.0;
    (0..count)
        .map(|index| {
            let at =
                ((index as f32 - center).abs().floor() as usize).min(half.len().saturating_sub(1));
            half.get(at).copied().unwrap_or(0.0)
        })
        .collect()
}

fn draw_idle(out: &mut Vec<CanvasShape>, size: Size, options: &DrawOptions) {
    let step = (options.bar_width + options.bar_gap) * 2.0;
    let y = size.height / 2.0 - 0.5;
    if step <= 0.0 {
        return;
    }
    let mut x = 0.0;
    while x < size.width {
        out.push(CanvasShape::Rect {
            x,
            y,
            width: options.bar_width,
            height: 1.0,
            radius: 0.0,
            alpha: IDLE_ALPHA,
            clip: None,
        });
        x += step;
    }
}

fn draw_line(
    out: &mut Vec<CanvasShape>,
    size: Size,
    frame: &VisualFrame,
    options: &DrawOptions,
    scroll_progress: f32,
) {
    let middle = size.height / 2.0;
    if options.mode == LiveWaveformMode::Static
        && let Some(samples) = frame.time_domain.as_ref().filter(|s| s.len() > 1)
    {
        let last = (samples.len() - 1) as f32;
        let points = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let x = (index as f32 / last) * size.width;
                let value = clamp(sample * options.sensitivity, -1.0, 1.0);
                (x, middle - value * (middle - options.line_width))
            })
            .collect();
        out.push(CanvasShape::Line {
            points,
            closed: false,
            fill_alpha: None,
            stroke: Some(options.line_width),
        });
        return;
    }

    let count = 2usize.max((size.width / (options.bar_width + options.bar_gap)).floor() as usize);
    let bars = DrawOptions {
        variant: LiveWaveformVariant::Bars,
        ..*options
    };
    let levels = levels_for(frame, count, &bars);
    let offset = count as f32 - levels.len() as f32 + 1.0 - scroll_progress;
    let x_at = |index: usize| ((offset + index as f32) / (count as f32 - 1.0)) * size.width;
    let reach = middle - options.line_width;
    let mut points = Vec::with_capacity(levels.len() * 2);
    for (index, level) in levels.iter().enumerate() {
        let value = clamp(level * options.sensitivity, 0.0, 1.0);
        points.push((x_at(index), middle - value * reach));
    }
    for (index, level) in levels.iter().enumerate().rev() {
        let value = clamp(level * options.sensitivity, 0.0, 1.0);
        points.push((x_at(index), middle + value * reach));
    }
    out.push(CanvasShape::Line {
        points,
        closed: true,
        fill_alpha: Some(0.25),
        stroke: Some(options.line_width),
    });
}

fn draw_level_bar(
    out: &mut Vec<CanvasShape>,
    size: Size,
    options: &DrawOptions,
    x: f32,
    level: f32,
    clip: Option<(f32, f32)>,
) {
    let value = clamp(level * options.sensitivity, 0.0, 1.0);
    let height = options.min_bar_height.max(value * size.height);
    out.push(CanvasShape::Rect {
        x,
        y: (size.height - height) / 2.0,
        width: options.bar_width,
        height,
        radius: options.bar_radius,
        alpha: 0.4 + 0.6 * value,
        clip,
    });
}

fn draw_scrolling_mirror(
    out: &mut Vec<CanvasShape>,
    size: Size,
    frame: &VisualFrame,
    options: &DrawOptions,
    scroll_progress: f32,
    count: usize,
) {
    let levels = history_levels(frame, count.div_ceil(2) + 1);
    let center = (size.width - options.bar_width) / 2.0;
    let pitch = options.bar_width + options.bar_gap;
    let left = Some((0.0, center));
    let right = Some((center + options.bar_width, size.width));
    for age in (1..levels.len()).rev() {
        let value = levels[levels.len() - 1 - age];
        let distance = (age as f32 - 1.0 + scroll_progress) * pitch;
        draw_level_bar(out, size, options, center - distance, value, left);
        draw_level_bar(out, size, options, center + distance, value, right);
    }
    if let Some(&newest) = levels.last() {
        let previous = if levels.len() >= 2 {
            levels[levels.len() - 2]
        } else {
            newest
        };
        draw_level_bar(
            out,
            size,
            options,
            center,
            previous + (newest - previous) * scroll_progress,
            None,
        );
    }
}

fn draw_bars(
    out: &mut Vec<CanvasShape>,
    size: Size,
    frame: &VisualFrame,
    options: &DrawOptions,
    scroll_progress: f32,
) {
    let pitch = options.bar_width + options.bar_gap;
    let count = 1usize.max(((size.width + options.bar_gap) / pitch).floor() as usize);
    if options.mode == LiveWaveformMode::Scrolling && options.variant == LiveWaveformVariant::Mirror
    {
        draw_scrolling_mirror(out, size, frame, options, scroll_progress, count);
        return;
    }
    let levels = levels_for(frame, count, options);
    let offset = count as f32 - levels.len() as f32 + 1.0 - scroll_progress;
    let used = count as f32 * pitch - options.bar_gap;
    let left = (size.width - used) / 2.0;
    for (index, level) in levels.iter().enumerate() {
        let x = left + (offset + index as f32) * pitch;
        draw_level_bar(out, size, options, x, *level, None);
    }
}

/// What the painter last drew (the canvas contents).
#[derive(Clone, Debug, Default)]
pub(crate) struct LiveScene {
    pub(crate) shapes: Vec<CanvasShape>,
    /// Fade width for the ends; `None` for no fade (and for the idle line).
    pub(crate) fade: Option<f32>,
    /// The dotted idle line rather than a signal.
    pub(crate) idle: bool,
}

/// The `tick` of `LiveWaveform`'s canvas effect: playback, dirty tracking and
/// drawing. GPUI keeps one per instance in keyed state.
pub(crate) struct LiveWaveformPainter {
    frame: Option<VisualFrame>,
    dirty: bool,
    playback: HistoryPlayback,
    options: Option<(DrawOptions, bool)>,
    size: Option<(f32, f32)>,
    last_paint_ms: f64,
    last_scroll_progress: f32,
    scene: LiveScene,
    paints: u64,
}

impl Default for LiveWaveformPainter {
    fn default() -> Self {
        Self {
            frame: None,
            dirty: true,
            playback: create_history_playback(),
            options: None,
            size: None,
            last_paint_ms: 0.0,
            last_scroll_progress: 1.0,
            scene: LiveScene::default(),
            paints: 0,
        }
    }
}

impl LiveWaveformPainter {
    /// `paint(frame)`: a new frame from the source or the handle.
    pub(crate) fn paint(&mut self, frame: VisualFrame) {
        self.frame = Some(frame);
        self.dirty = true;
    }

    /// `clear()`, and what a source change does.
    pub(crate) fn clear(&mut self) {
        self.playback.clear();
        self.frame = None;
        self.dirty = true;
    }

    pub(crate) fn scene(&self) -> &LiveScene {
        &self.scene
    }

    /// One frame at `now_ms`. Returns whether it needs another frame.
    pub(crate) fn tick(
        &mut self,
        now_ms: f64,
        width: f32,
        height: f32,
        options: &DrawOptions,
        reduced_motion: bool,
    ) -> bool {
        // The React effect re-runs when the options change: fresh locals.
        if self.options != Some((*options, reduced_motion)) {
            self.options = Some((*options, reduced_motion));
            self.last_paint_ms = 0.0;
            self.last_scroll_progress = 1.0;
            self.dirty = true;
        }
        if self.size != Some((width, height)) {
            self.size = Some((width, height));
            self.dirty = true;
        }
        let Self {
            frame,
            dirty,
            playback,
            last_paint_ms,
            last_scroll_progress,
            scene,
            paints,
            ..
        } = self;

        let mut scroll_progress = 1.0;
        let mut drawn = frame.as_ref();
        if let Some(current) = frame.as_ref()
            && options.active
            && options.mode == LiveWaveformMode::Scrolling
            && !reduced_motion
        {
            let read = playback.read(current, now_ms);
            drawn = Some(read.frame);
            scroll_progress = read.progress;
        } else {
            playback.clear();
        }
        if scroll_progress != *last_scroll_progress {
            *dirty = true;
        }
        if width <= 0.0 {
            return false;
        }
        if !*dirty {
            return scroll_progress < 1.0;
        }
        if reduced_motion && now_ms - *last_paint_ms < REDUCED_MOTION_INTERVAL_MS {
            return true;
        }
        *last_paint_ms = now_ms;
        *last_scroll_progress = scroll_progress;
        *dirty = false;
        *paints += 1;

        let size = Size { width, height };
        scene.shapes.clear();
        scene.fade = None;
        scene.idle = false;
        let Some(drawn) = drawn.filter(|_| options.active) else {
            scene.idle = true;
            draw_idle(&mut scene.shapes, size, options);
            return false;
        };
        if options.variant == LiveWaveformVariant::Line {
            draw_line(&mut scene.shapes, size, drawn, options, scroll_progress);
        } else {
            draw_bars(&mut scene.shapes, size, drawn, options, scroll_progress);
        }
        if options.fade_edges {
            scene.fade = Some(options.fade_width);
        }
        scroll_progress < 1.0
    }
}

struct LiveWaveformState {
    painter: LiveWaveformPainter,
    source: SourceCursor,
}

/// A canvas waveform of a live signal, as scrolling history or the current
/// frame. Draws in the current text colour (`text-*` classes →
/// [`Styled::text_color`]).
///
/// ```ignore
/// let signal = use_demo_signal("speech", DemoSignalOptions::kind(Speech).with_history_size(120), window, cx);
/// LiveWaveform::new("mic").mode(LiveWaveformMode::Scrolling).source(signal.visual).h(px(80.))
/// ```
#[derive(IntoElement)]
pub struct LiveWaveform {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    options: DrawOptions,
    actions: Option<LiveWaveformActions>,
    style: StyleRefinement,
}

impl LiveWaveform {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            options: DrawOptions::default(),
            actions: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.source = Some(source);
        self
    }

    /// `scrolling` shows the level history; `static` shows the current frame. Default `Static`.
    pub fn mode(mut self, mode: LiveWaveformMode) -> Self {
        self.options.mode = mode;
        self
    }

    /// Default `Bars`. `Line` draws the waveform trace.
    pub fn variant(mut self, variant: LiveWaveformVariant) -> Self {
        self.options.variant = variant;
        self
    }

    /// Bar width in pixels. Default 3.
    pub fn bar_width(mut self, bar_width: f32) -> Self {
        self.options.bar_width = bar_width;
        self
    }

    /// Gap between bars in pixels. Default 1.
    pub fn bar_gap(mut self, bar_gap: f32) -> Self {
        self.options.bar_gap = bar_gap;
        self
    }

    /// Bar corner radius in pixels. Default 1.5.
    pub fn bar_radius(mut self, bar_radius: f32) -> Self {
        self.options.bar_radius = bar_radius;
        self
    }

    /// Smallest bar height in pixels. Default 4.
    pub fn min_bar_height(mut self, min_bar_height: f32) -> Self {
        self.options.min_bar_height = min_bar_height;
        self
    }

    /// Line width for the `Line` variant. Default 1.5.
    pub fn line_width(mut self, line_width: f32) -> Self {
        self.options.line_width = line_width;
        self
    }

    /// Fade the left and right edges. Default true.
    pub fn fade_edges(mut self, fade_edges: bool) -> Self {
        self.options.fade_edges = fade_edges;
        self
    }

    /// Width of the fade in pixels. Default 24.
    pub fn fade_width(mut self, fade_width: f32) -> Self {
        self.options.fade_width = fade_width;
        self
    }

    /// When false, shows an idle dotted line. Default true.
    pub fn active(mut self, active: bool) -> Self {
        self.options.active = active;
        self
    }

    /// Visual gain. Default 1.
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.options.sensitivity = sensitivity;
        self
    }

    /// `actionsRef`: paint frames directly or clear.
    pub fn actions(mut self, actions: &LiveWaveformActions) -> Self {
        self.actions = Some(actions.clone());
        self
    }
}

impl Styled for LiveWaveform {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LiveWaveform {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| LiveWaveformState {
            painter: LiveWaveformPainter::default(),
            source: SourceCursor::default(),
        });
        let source = self.source.clone();
        let ops = self
            .actions
            .as_ref()
            .map(|a| a.queue.drain())
            .unwrap_or_default();
        state.update(cx, |s, _| {
            let (changed, frame) = s.source.poll(source.as_ref());
            if changed {
                s.painter.clear();
            }
            if let Some(frame) = frame {
                match frame {
                    Some(frame) => s.painter.paint(frame),
                    None => s.painter.clear(),
                }
            }
            for op in ops {
                match op {
                    ActionOp::Paint(frame) => s.painter.paint(frame),
                    ActionOp::Clear => s.painter.clear(),
                }
            }
        });

        let options = self.options;
        let reduced = prefers_reduced_motion(cx);
        // A source can't wake the painter, so watch it while it can matter.
        let watching = options.active && (self.source.is_some() || self.actions.is_some());
        let painter_state = state.clone();

        let mut root = div().relative().h(px(64.0)).w_full();
        root.style().refine(&self.style);
        root.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let width = bounds.size.width.as_f32();
                    let height = bounds.size.height.as_f32();
                    let more = painter_state.update(cx, |s, _| {
                        s.painter.tick(now_ms(), width, height, &options, reduced)
                    });
                    if more || watching {
                        window.request_animation_frame();
                    }
                    let color = window.text_style().color;
                    let scene = painter_state.read(cx).painter.scene().clone();
                    paint_canvas_shapes(bounds, &scene.shapes, color, scene.fade, None, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::demo_signal::{DemoSignal, DemoSignalKind, DemoSignalOptions};

    fn frame_of() -> VisualFrame {
        VisualFrame {
            bands: vec![0.5],
            history: vec![0.25, 0.5, 0.75, 1.0, 0.0, 0.0, 0.0, 0.0],
            history_interval_ms: Some(50.0),
            history_length: 4,
            history_start: 0,
            history_updated_at: Some(0.0),
            peak_db: -12.0,
            ..Default::default()
        }
    }

    fn close(a: f32, b: f32) -> bool {
        // `toBeCloseTo` default precision: 2 digits.
        (a - b).abs() < 0.005
    }

    /// A drawn bar: x, height, alpha and clip.
    type Bar = (f32, f32, f32, Option<(f32, f32)>);

    /// Drives a painter like `useFakeFrames`: one tick every 16 ms.
    struct Harness {
        painter: LiveWaveformPainter,
        options: DrawOptions,
        width: f32,
        now: f64,
        reduced: bool,
        source: Option<DemoSignal>,
    }

    impl Harness {
        fn new(width: f32, options: DrawOptions) -> Self {
            Self {
                painter: LiveWaveformPainter::default(),
                options,
                width,
                now: 0.0,
                reduced: false,
                source: None,
            }
        }

        fn advance(&mut self, ms: f64) -> bool {
            let mut more = false;
            let end = self.now + ms;
            while self.now + 16.0 <= end + 1e-9 {
                self.now += 16.0;
                if let Some(signal) = self.source.as_mut() {
                    signal.tick(self.now);
                    let frame = signal.visual_frame().clone();
                    self.painter.paint(frame);
                }
                more = self
                    .painter
                    .tick(self.now, self.width, 80.0, &self.options, self.reduced);
            }
            more
        }

        /// Bars (rounded rects; the idle line doesn't count).
        fn bars(&self) -> Vec<Bar> {
            let scene = self.painter.scene();
            if scene.idle {
                return Vec::new();
            }
            scene
                .shapes
                .iter()
                .filter_map(|shape| match shape {
                    CanvasShape::Rect {
                        x,
                        height,
                        alpha,
                        clip,
                        ..
                    } => Some((*x, *height, *alpha, *clip)),
                    CanvasShape::Line { .. } => None,
                })
                .collect()
        }

        fn bar_x(&self, height: f32) -> f32 {
            self.bars()
                .iter()
                .find(|bar| bar.1 == height)
                .map_or(f32::NAN, |bar| bar.0)
        }

        fn first_point_x(&self) -> f32 {
            match self.painter.scene().shapes.first() {
                Some(CanvasShape::Line { points, .. }) => points[0].0,
                _ => f32::NAN,
            }
        }
    }

    fn scrolling() -> DrawOptions {
        DrawOptions {
            fade_edges: false,
            mode: LiveWaveformMode::Scrolling,
            ..DrawOptions::default()
        }
    }

    #[test]
    fn moves_bars_between_source_frames_at_one_pitch_per_history_interval() {
        let mut h = Harness::new(24.0, scrolling());
        h.painter.paint(frame_of());
        h.advance(64.0);
        let first = h.bar_x(40.0);
        h.advance(16.0);
        assert!(close(first - h.bar_x(40.0), (4.0 * 16.0) / 50.0));
    }

    #[test]
    fn does_not_jump_when_the_next_sample_arrives_late() {
        let mut h = Harness::new(24.0, scrolling());
        let mut frame = frame_of();
        h.painter.paint(frame.clone());
        h.advance(48.0);
        frame.history[4] = 0.125;
        frame.history_length = 5;
        frame.history_updated_at = Some(50.0);
        h.painter.paint(frame);
        h.advance(48.0);
        let first = h.bar_x(40.0);
        h.advance(16.0);
        assert!(close(first - h.bar_x(40.0), (4.0 * 16.0) / 50.0));
    }

    #[test]
    fn keeps_the_extra_bar_at_the_left_edge_when_the_window_is_full() {
        let mut h = Harness::new(12.0, scrolling());
        h.painter.paint(frame_of());
        h.advance(64.0);
        let bars = h.bars();
        assert_eq!(bars.len(), 4);
        assert!(close(bars[0].0, 0.5 - (4.0 * 14.0) / 50.0));
    }

    #[test]
    fn keeps_the_outgoing_bar_visible_after_the_ring_wraps() {
        let mut h = Harness::new(24.0, scrolling());
        let mut frame = frame_of();
        frame.history = vec![0.25, 0.5, 0.75, 1.0];
        h.painter.paint(frame.clone());
        h.advance(48.0);
        frame.history[0] = 0.125;
        frame.history_start = 1;
        frame.history_updated_at = Some(50.0);
        frame.history_previous_level = Some(0.25);
        h.painter.paint(frame);
        h.advance(48.0);
        let first = h.bar_x(20.0);
        h.advance(16.0);
        assert!(close(first - h.bar_x(20.0), (4.0 * 16.0) / 50.0));
    }

    #[test]
    fn moves_a_scrolling_line_between_source_frames() {
        let mut h = Harness::new(
            24.0,
            DrawOptions {
                variant: LiveWaveformVariant::Line,
                ..scrolling()
            },
        );
        h.painter.paint(frame_of());
        h.advance(64.0);
        let first = h.first_point_x();
        h.advance(16.0);
        assert!(close(
            first - h.first_point_x(),
            ((24.0 / 5.0) * 16.0) / 50.0
        ));
    }

    #[test]
    fn moves_mirrored_history_out_from_the_center() {
        for width in [20.0, 24.0] {
            let mut h = Harness::new(
                width,
                DrawOptions {
                    variant: LiveWaveformVariant::Mirror,
                    ..scrolling()
                },
            );
            h.painter.paint(frame_of());
            h.advance(64.0);
            let xs = |h: &Harness| {
                let mut xs: Vec<f32> = h
                    .bars()
                    .iter()
                    .filter(|bar| bar.1 == 60.0)
                    .map(|bar| bar.0)
                    .collect();
                xs.sort_by(f32::total_cmp);
                xs
            };
            let first = xs(&h);
            h.advance(16.0);
            let next = xs(&h);
            assert_eq!(first.len(), 2);
            assert!(close(first[0] - next[0], (4.0 * 16.0) / 50.0));
            assert!(close(next[1] - first[1], (4.0 * 16.0) / 50.0));
            assert!(close(next[0] + next[1] + 3.0, width));
        }
    }

    #[test]
    fn keeps_mirrored_center_opacity_steady_across_sample_boundaries() {
        let width = 24.0;
        let mut h = Harness::new(
            width,
            DrawOptions {
                variant: LiveWaveformVariant::Mirror,
                ..scrolling()
            },
        );
        let mut frame = frame_of();
        frame.history.fill(0.25);
        frame.history_updated_at = Some(16.0);
        h.painter.paint(frame.clone());
        let center = width / 2.0;
        for index in 0..8 {
            h.advance(16.0);
            let opacity = h
                .bars()
                .iter()
                .filter(|(x, _, _, clip)| {
                    let visible = clip.is_none_or(|(from, to)| from <= center && center < to);
                    visible && *x <= center && center < x + 3.0
                })
                .fold(0.0, |alpha, bar| alpha + (1.0 - alpha) * bar.2);
            assert!(close(opacity, 0.55), "frame {index}: {opacity}");
            if index == 3 {
                frame.history_updated_at = Some(66.0);
                frame.history_length += 1;
                h.painter.paint(frame.clone());
            }
        }
    }

    #[test]
    fn stops_moving_after_one_interval_without_new_history_and_clears() {
        let mut h = Harness::new(24.0, scrolling());
        h.painter.paint(frame_of());
        h.advance(160.0);
        let paints = h.painter.paints;
        let more = h.advance(160.0);
        assert_eq!(h.painter.paints, paints);
        assert!(!more);
        h.painter.clear();
        h.advance(16.0);
        assert!(h.bars().is_empty());
    }

    #[test]
    fn clears_a_sleeping_waveform_when_its_source_is_disconnected() {
        let emitter = crate::audio::frame_source::create_frame_emitter::<VisualFrame>();
        let mut cursor = SourceCursor::default();
        let mut h = Harness::new(24.0, scrolling());
        let source = emitter.source();
        let _ = cursor.poll(Some(&source));
        emitter.emit(frame_of());
        if let (_, Some(Some(frame))) = cursor.poll(Some(&source)) {
            h.painter.paint(frame);
        }
        let more = h.advance(160.0);
        assert!(!h.bars().is_empty());
        assert!(!more);
        let (changed, _) = cursor.poll(None);
        assert!(changed);
        h.painter.clear();
        let more = h.advance(16.0);
        assert!(h.bars().is_empty());
        assert!(!more);
    }

    #[test]
    fn keeps_scrolling_continuous_when_drawing_options_change() {
        let mut h = Harness::new(240.0, scrolling());
        h.source = Some(DemoSignal::new(DemoSignalOptions::kind(
            DemoSignalKind::Tone,
        )));
        h.advance(272.0);
        let x = h.bars()[0].0;
        h.options.sensitivity = 2.0;
        h.advance(16.0);
        assert!(close(x - h.bars()[0].0, 1.0), "{}", x - h.bars()[0].0);
    }

    #[test]
    fn replaces_same_timestamp_history_after_a_source_change() {
        for delay in [0.0, 16.0] {
            let mut h = Harness::new(24.0, scrolling());
            let mut frame = frame_of();
            frame.history.fill(0.25);
            h.painter.paint(frame);
            h.advance(256.0);
            assert_eq!(h.bars()[0].1, 20.0);
            // A new source: the component clears on the swap.
            h.painter.clear();
            h.advance(delay);
            let mut replacement = frame_of();
            replacement.history.fill(0.75);
            h.painter.paint(replacement);
            h.advance(16.0);
            assert_eq!(h.bars()[0].1, 60.0);
        }
    }

    #[test]
    fn clears_buffered_history_before_painting_within_the_same_frame() {
        let mut h = Harness::new(24.0, scrolling());
        let mut frame = frame_of();
        frame.history.fill(0.25);
        h.painter.paint(frame.clone());
        h.advance(256.0);
        assert_eq!(h.bars()[0].1, 20.0);
        h.painter.clear();
        frame.history.fill(0.75);
        h.painter.paint(frame);
        h.advance(16.0);
        assert_eq!(h.bars()[0].1, 60.0);
    }

    #[test]
    fn keeps_untimed_custom_sources_and_static_mode_still_between_frames() {
        let mut h = Harness::new(24.0, scrolling());
        let mut frame = frame_of();
        frame.history_updated_at = None;
        frame.history_interval_ms = None;
        h.painter.paint(frame);
        h.advance(16.0);
        let paints = h.painter.paints;
        h.advance(32.0);
        assert_eq!(h.painter.paints, paints);
        h.options = DrawOptions {
            fade_edges: false,
            ..DrawOptions::default()
        };
        h.painter.paint(frame_of());
        h.advance(16.0);
        let paints = h.painter.paints;
        h.advance(32.0);
        assert_eq!(h.painter.paints, paints);
    }

    #[test]
    fn keeps_the_reduced_motion_paint_limit() {
        let mut h = Harness::new(24.0, scrolling());
        h.reduced = true;
        h.source = Some(DemoSignal::new(DemoSignalOptions::default()));
        h.advance(1024.0);
        assert_eq!(h.painter.paints, 4);
    }

    #[test]
    fn moves_on_each_paint_with_the_default_demo_source() {
        let mut h = Harness::new(240.0, scrolling());
        h.source = Some(DemoSignal::new(DemoSignalOptions::kind(
            DemoSignalKind::Tone,
        )));
        h.advance(192.0);
        let first = h.bars()[0].0;
        h.advance(16.0);
        assert!(close(first - h.bars()[0].0, (4.0 * 16.0) / 64.0));
    }

    #[test]
    fn moves_steadily_when_a_source_connects_after_the_painter() {
        let mut h = Harness::new(240.0, scrolling());
        h.advance(16.0);
        h.source = Some(DemoSignal::new(DemoSignalOptions::kind(
            DemoSignalKind::Tone,
        )));
        h.advance(160.0);
        let mut previous = h.bars()[0].0;
        for _ in 0..8 {
            h.advance(16.0);
            let x = h.bars()[0].0;
            assert!(close(previous - x, (4.0 * 16.0) / 64.0), "{}", previous - x);
            previous = x;
        }
    }

    #[test]
    fn draws_the_idle_line_when_inactive() {
        let mut h = Harness::new(
            24.0,
            DrawOptions {
                active: false,
                ..DrawOptions::default()
            },
        );
        h.painter.paint(frame_of());
        assert!(!h.advance(16.0));
        let scene = h.painter.scene();
        assert!(scene.idle);
        // step = (3 + 1) * 2 = 8: x = 0, 8, 16.
        assert_eq!(scene.shapes.len(), 3);
        assert!(scene.fade.is_none());
    }

    #[test]
    fn static_mirror_mirrors_half_the_bands() {
        let frame = VisualFrame {
            bands: vec![0.1, 0.2, 0.3, 0.4],
            ..Default::default()
        };
        let options = DrawOptions {
            variant: LiveWaveformVariant::Mirror,
            ..DrawOptions::default()
        };
        // Half: 4 bands into 3 (loudest per span) = [0.1, 0.2, 0.4], mirrored
        // out from the centre.
        assert_eq!(
            levels_for(&frame, 5, &options),
            vec![0.4, 0.2, 0.1, 0.2, 0.4]
        );
    }
}
