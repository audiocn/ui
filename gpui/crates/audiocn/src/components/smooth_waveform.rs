//! Port of `components/ui/smooth-waveform.tsx`.

use gpui::{
    App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce, StyleRefinement, Styled,
    Window, canvas, div, px,
};

use super::live_waveform::{ActionOp, ActionQueue, CanvasShape, SourceCursor, paint_canvas_shapes};
use crate::audio::decibels::clamp;
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::types::VisualFrame;
use crate::audio::wave_line::{WaveLine, WaveLineMode, WaveLineOptions, create_wave_line};
use crate::signal::reduced_motion::prefers_reduced_motion;

const DEFAULT_LINE_WIDTH: f32 = 2.0;
const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
/// One point every this many pixels along the line.
const POINT_SPACING_PX: f32 = 3.0;
/// Width of the faded ends, in pixels.
const FADE_PX: f32 = 24.0;

/// `mode` of [`SmoothWaveform`]: `Wave` (bands) or `Scope` (the signal).
pub type SmoothWaveformMode = WaveLineMode;

/// The imperative handle of a [`SmoothWaveform`] (`actionsRef`). Create one,
/// pass it with [`SmoothWaveform::actions`] and keep a clone.
///
/// ```ignore
/// let actions = SmoothWaveformActions::new();
/// SmoothWaveform::new("voice").actions(&actions);
/// actions.paint(frame);
/// ```
#[derive(Clone, Default, PartialEq)]
pub struct SmoothWaveformActions {
    queue: ActionQueue,
}

impl SmoothWaveformActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint a frame directly.
    pub fn paint(&self, frame: VisualFrame) {
        self.queue.push(ActionOp::Paint(frame));
    }

    /// Forget the last frame, so the line falls flat.
    pub fn clear(&self) {
        self.queue.push(ActionOp::Clear);
    }
}

/// The canvas effect of `SmoothWaveform`: the wave line, rebuilt when the
/// line options change, stepped every frame.
pub(crate) struct SmoothWaveformPainter {
    line: WaveLine,
    frame: Option<VisualFrame>,
    last_paint_ms: f64,
    /// `data-active`: the line follows a signal.
    active: bool,
    shapes: Vec<CanvasShape>,
    paints: u64,
}

impl SmoothWaveformPainter {
    pub(crate) fn new(options: WaveLineOptions) -> Self {
        Self {
            line: create_wave_line(options),
            frame: None,
            last_paint_ms: 0.0,
            active: false,
            shapes: Vec::new(),
            paints: 0,
        }
    }

    /// Rebuilds the line when `loading`, `mode`, reduced motion or
    /// `sensitivity` change (the effect's dependencies); stroke options don't.
    pub(crate) fn configure(&mut self, options: WaveLineOptions) {
        if self.line.options() != options {
            self.line = create_wave_line(options);
            self.last_paint_ms = 0.0;
            self.active = false;
        }
    }

    pub(crate) fn paint(&mut self, frame: VisualFrame) {
        self.frame = Some(frame);
    }

    pub(crate) fn clear(&mut self) {
        self.frame = None;
    }

    /// One frame. Returns whether it painted.
    pub(crate) fn tick(&mut self, now_ms: f64, width: f32, height: f32, line_width: f32) -> bool {
        if width <= 0.0 {
            return false;
        }
        if self.line.options().reduced_motion
            && now_ms - self.last_paint_ms < REDUCED_MOTION_INTERVAL_MS
        {
            return false;
        }
        self.last_paint_ms = now_ms;
        let count = (width / POINT_SPACING_PX).round() + 1.0;
        self.active = self.line.step(now_ms, self.frame.as_ref(), count);
        let line_width = clamp(line_width, 0.5, height / 2.0);
        let middle = height / 2.0;
        let reach = (middle - line_width).max(0.0);
        let points = self.line.points();
        let last = (points.len().max(2) - 1) as f32;
        let points = points
            .iter()
            .enumerate()
            .map(|(point, h)| ((point as f32 / last) * width, middle - h * reach))
            .collect();
        self.shapes.clear();
        self.shapes.push(CanvasShape::Line {
            points,
            closed: false,
            fill_alpha: None,
            stroke: Some(line_width),
        });
        self.paints += 1;
        true
    }

    /// `data-active` (a DOM attribute in React; only the tests read it here).
    #[allow(dead_code)]
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }
}

struct SmoothWaveformState {
    painter: SmoothWaveformPainter,
    source: SourceCursor,
}

/// One clean line across the width that follows the sound, as a flowing wave
/// or an oscilloscope trace. Draws in the current text colour.
///
/// ```ignore
/// SmoothWaveform::new("voice").source(signal.visual).text_color(cx.theme().primary).h(px(128.))
/// SmoothWaveform::new("connecting").loading(true)
/// ```
#[derive(IntoElement)]
pub struct SmoothWaveform {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    mode: SmoothWaveformMode,
    loading: bool,
    sensitivity: f32,
    line_width: f32,
    fade_edges: bool,
    actions: Option<SmoothWaveformActions>,
    style: StyleRefinement,
}

impl SmoothWaveform {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            mode: SmoothWaveformMode::Wave,
            loading: false,
            sensitivity: 1.0,
            line_width: DEFAULT_LINE_WIDTH,
            fade_edges: true,
            actions: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.source = Some(source);
        self
    }

    /// `Wave` draws a smooth wave shaped by the frequency bands. `Scope` draws
    /// the signal itself, like an oscilloscope. Default `Wave`.
    pub fn mode(mut self, mode: SmoothWaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Runs a pulse along the line, for connecting or thinking states. Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Visual gain. Default 1.
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Line width in pixels. Default 2.
    pub fn line_width(mut self, line_width: f32) -> Self {
        self.line_width = line_width;
        self
    }

    /// Fade the left and right ends. Default true.
    pub fn fade_edges(mut self, fade_edges: bool) -> Self {
        self.fade_edges = fade_edges;
        self
    }

    /// `actionsRef`: paint frames directly or clear.
    pub fn actions(mut self, actions: &SmoothWaveformActions) -> Self {
        self.actions = Some(actions.clone());
        self
    }
}

impl Styled for SmoothWaveform {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SmoothWaveform {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let options = WaveLineOptions {
            mode: self.mode,
            loading: self.loading,
            sensitivity: self.sensitivity,
            reduced_motion: prefers_reduced_motion(cx),
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| SmoothWaveformState {
            painter: SmoothWaveformPainter::new(options),
            source: SourceCursor::default(),
        });
        let ops = self
            .actions
            .as_ref()
            .map(|a| a.queue.drain())
            .unwrap_or_default();
        let source = self.source.clone();
        state.update(cx, |s, _| {
            s.painter.configure(options);
            if let (_, Some(frame)) = s.source.poll(source.as_ref()) {
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

        let line_width = self.line_width;
        let fade = self.fade_edges.then_some(FADE_PX);
        let painter_state = state.clone();
        let mut root = div().relative().h(px(96.0)).w_full();
        root.style().refine(&self.style);
        root.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    // The line drifts and eases every frame, like `subscribeFrame`.
                    window.request_animation_frame();
                    let width = bounds.size.width.as_f32();
                    let height = bounds.size.height.as_f32();
                    painter_state.update(cx, |s, _| {
                        s.painter.tick(now_ms(), width, height, line_width);
                    });
                    let color = window.text_style().color;
                    let shapes = painter_state.read(cx).painter.shapes.clone();
                    paint_canvas_shapes(bounds, &shapes, color, fade, None, window);
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

    fn frame_of(bands: &[f32]) -> VisualFrame {
        VisualFrame {
            bands: bands.to_vec(),
            history: vec![0.0],
            history_length: 0,
            history_start: 0,
            peak_db: -12.0,
            ..Default::default()
        }
    }

    fn options() -> WaveLineOptions {
        WaveLineOptions {
            mode: WaveLineMode::Wave,
            loading: false,
            sensitivity: 1.0,
            reduced_motion: false,
        }
    }

    /// Ticks every 16 ms from `*now` for `ms`, at 240 x 80.
    fn advance(painter: &mut SmoothWaveformPainter, now: &mut f64, ms: f64, line_width: f32) {
        let end = *now + ms;
        while *now + 16.0 <= end + 1e-9 {
            *now += 16.0;
            painter.tick(*now, 240.0, 80.0, line_width);
        }
    }

    fn points(painter: &SmoothWaveformPainter) -> Vec<(f32, f32)> {
        match painter.shapes.first() {
            Some(CanvasShape::Line { points, .. }) => points.clone(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn draws_a_flat_line_across_the_width_with_no_signal() {
        let mut painter = SmoothWaveformPainter::new(options());
        let mut now = 0.0;
        advance(&mut painter, &mut now, 100.0, 2.0);
        let points = points(&painter);
        let min = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let max = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(min, 0.0);
        assert_eq!(max, 240.0);
        assert!(points.iter().all(|p| p.1 == points[0].1));
        // 240 / 3 + 1 points.
        assert_eq!(points.len(), 81);
    }

    #[test]
    fn follows_frames_from_its_handle_and_marks_an_active_signal() {
        let mut painter = SmoothWaveformPainter::new(options());
        let mut now = 0.0;
        advance(&mut painter, &mut now, 100.0, 2.0);
        assert!(!painter.is_active());
        painter.paint(frame_of(&[1.0, 1.0, 1.0, 1.0]));
        advance(&mut painter, &mut now, 300.0, 2.0);
        assert!(painter.is_active());
        let points = points(&painter);
        assert!(points.iter().any(|p| p.1 != points[0].1));
        painter.clear();
        advance(&mut painter, &mut now, 1000.0, 2.0);
        assert!(!painter.is_active());
    }

    #[test]
    fn keeps_its_line_when_only_the_stroke_changes() {
        let mut painter = SmoothWaveformPainter::new(options());
        let mut now = 0.0;
        painter.paint(frame_of(&[1.0, 1.0, 1.0, 1.0]));
        advance(&mut painter, &mut now, 300.0, 2.0);
        assert!(painter.is_active());
        // A new line width (and fade) is a stroke option: the line stays.
        painter.configure(options());
        advance(&mut painter, &mut now, 16.0, 4.0);
        assert!(painter.is_active());
    }

    #[test]
    fn rebuilds_the_line_when_the_mode_changes() {
        let mut painter = SmoothWaveformPainter::new(options());
        let mut now = 0.0;
        painter.paint(frame_of(&[1.0, 1.0, 1.0, 1.0]));
        advance(&mut painter, &mut now, 300.0, 2.0);
        painter.configure(WaveLineOptions {
            loading: true,
            ..options()
        });
        assert!(!painter.is_active());
        advance(&mut painter, &mut now, 16.0, 2.0);
        // Loading never reports a signal.
        assert!(!painter.is_active());
    }

    #[test]
    fn keeps_the_reduced_motion_paint_limit() {
        let mut painter = SmoothWaveformPainter::new(WaveLineOptions {
            reduced_motion: true,
            ..options()
        });
        let mut now = 0.0;
        advance(&mut painter, &mut now, 1024.0, 2.0);
        assert_eq!(painter.paints, 4);
    }

    #[test]
    fn clamps_the_line_width_to_half_the_height() {
        let mut painter = SmoothWaveformPainter::new(options());
        painter.tick(16.0, 240.0, 6.0, 10.0);
        match painter.shapes.first() {
            Some(CanvasShape::Line { stroke, .. }) => assert_eq!(*stroke, Some(3.0)),
            _ => panic!("no line"),
        }
    }
}
