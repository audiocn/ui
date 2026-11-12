//! Port of `components/ui/bar-visualizer.tsx`.
//!
//! A row of rounded bars driven by frequency bands, with idle, loading and
//! mirrored modes. Bars are painted in one canvas in the inherited text colour
//! (`bg-current`), so colour them with `.text_color(..)` like the React
//! `text-primary` class.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, Pixels,
    Refineable, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    WeakEntity, Window, canvas, div, fill, point, prelude::FluentBuilder, px, size,
};

use crate::audio::bar_levels::{BarIdle, BarLevels, BarLevelsOptions};
use crate::audio::frame_loop::{FrameTask, PainterClock, now_ms};
use crate::audio::frame_source::FrameSource;
use crate::audio::types::{Orientation, VisualFrame};
use crate::config::AudioConfig;
use crate::signal::reduced_motion::prefers_reduced_motion;

const DEFAULT_BAR_COUNT: usize = 24;
const DEFAULT_MIN_LEVEL: f32 = 0.08;
const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
/// `--bar-gap: 0.1875rem`.
const DEFAULT_BAR_GAP: f32 = 3.0;
/// `--bar-width: 0.375rem`.
const DEFAULT_BAR_WIDTH: f32 = 6.0;
/// `--bar-radius: 9999px` (fully rounded).
const DEFAULT_BAR_RADIUS: f32 = 9999.0;
/// `h-16` / `w-16`.
const DEFAULT_CROSS_SIZE: f32 = 64.0;
/// Painted levels change only by more than this (the DOM style-write threshold).
const SHOWN_EPSILON: f32 = 0.002;

/// Where bars grow from (`align`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BarVisualizerAlign {
    /// Grow both ways from the middle. The default.
    #[default]
    Center,
    /// Grow from the top (horizontal) or the left (vertical).
    Start,
    /// Grow from the bottom (horizontal) or the right (vertical).
    End,
}

/// The painter behind a [`BarVisualizer`] (`createBarPainter` plus the
/// frame task that drives it): bar levels with release smoothing and idle
/// animations, sleeping once settled until new levels wake it.
#[derive(Clone, Debug)]
struct BarPainter {
    options: BarLevelsOptions,
    bar_levels: BarLevels,
    clock: PainterClock,
    task: FrameTask,
    /// Last painted level per bar; negative before the first paint.
    shown: Vec<f32>,
    last_paint_ms: f64,
    /// The signal is above the floor (`data-active`).
    active: bool,
    input: Option<Vec<f32>>,
}

impl BarPainter {
    fn new(options: BarLevelsOptions) -> Self {
        Self {
            options,
            bar_levels: BarLevels::new(options),
            clock: PainterClock::default(),
            task: FrameTask::default(),
            shown: vec![-1.0; options.bar_count],
            last_paint_ms: f64::NEG_INFINITY,
            active: false,
            input: None,
        }
    }

    /// New options recreate the painter (the React effect re-runs), keeping the input.
    fn set_options(&mut self, options: BarLevelsOptions) {
        if options != self.options {
            let input = self.input.take();
            *self = Self::new(options);
            self.input = input;
        }
    }

    /// New levels (or none) wake the painter.
    fn set_input(&mut self, input: Option<Vec<f32>>) {
        self.input = input;
        self.task.wake();
    }

    fn step(&mut self, frame_ms: f64) -> bool {
        let now_ms = self.clock.tick(frame_ms);
        if self.options.reduced_motion && now_ms - self.last_paint_ms < REDUCED_MOTION_INTERVAL_MS {
            return true;
        }
        self.last_paint_ms = now_ms;
        self.active = self.bar_levels.step(now_ms, self.input.as_deref());
        for (shown, &value) in self.shown.iter_mut().zip(self.bar_levels.levels()) {
            if (value - *shown).abs() > SHOWN_EPSILON {
                *shown = value;
            }
        }
        !self.bar_levels.settled()
    }

    /// Runs one animation frame. Returns whether another frame is needed.
    fn frame(&mut self, frame_ms: f64) -> bool {
        let mut task = self.task;
        let awake = task.run(frame_ms, |now| self.step(now));
        self.task = task;
        awake
    }

    /// The levels to draw, 0..1 per bar.
    fn shown(&self) -> Vec<f32> {
        let min_level = self.options.min_level;
        self.shown
            .iter()
            .map(|&level| if level < 0.0 { min_level } else { level })
            .collect()
    }
}

/// Keyed state of a [`BarVisualizer`].
pub struct BarVisualizerState {
    painter: BarPainter,
    /// The `levels` prop last seen, so only changes wake the painter.
    levels_prop: Option<Vec<f32>>,
    source_version: Option<u64>,
}

impl BarVisualizerState {
    /// The signal is above the floor (the React `data-active` attribute).
    pub fn active(&self) -> bool {
        self.painter.active
    }
}

/// `BarVisualizerActions`: paint levels into a [`BarVisualizer`] from your
/// own frame loop (`actionsRef`). Create one, pass it with
/// [`BarVisualizer::actions`], then call [`paint`](Self::paint).
///
/// ```ignore
/// let actions = BarVisualizerActions::new();
/// div().child(BarVisualizer::new("bars").actions(&actions));
/// // later, e.g. from a timer:
/// actions.paint(&[0.2, 0.8, 0.5], cx);
/// ```
#[derive(Clone, Default)]
pub struct BarVisualizerActions {
    state: Rc<RefCell<Option<WeakEntity<BarVisualizerState>>>>,
}

impl BarVisualizerActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint levels (0..1) directly. They are resampled to the bar count.
    /// Does nothing until the visualizer has rendered once.
    pub fn paint(&self, levels: &[f32], cx: &mut App) {
        let Some(state) = self.state.borrow().as_ref().and_then(WeakEntity::upgrade) else {
            return;
        };
        state.update(cx, |state, cx| {
            state.painter.set_input(Some(levels.to_vec()));
            cx.notify();
        });
    }

    /// Whether the signal is above the floor (`data-active`).
    pub fn is_active(&self, cx: &App) -> bool {
        self.state
            .borrow()
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_some_and(|state| state.read(cx).active())
    }

    fn attach(&self, state: &Entity<BarVisualizerState>) {
        *self.state.borrow_mut() = Some(state.downgrade());
    }
}

/// A row of bars driven by frequency bands, with idle, loading and mirrored modes.
///
/// Bars use the inherited text colour; set it with `.text_color(..)`.
///
/// ```ignore
/// let signal = use_demo_signal("voice", DemoSignalOptions::kind(DemoSignalKind::Speech), window, cx);
/// BarVisualizer::new("voice")
///     .source(signal.visual)
///     .text_color(cx.theme().primary)
///     .h(px(96.0))
///     .max_w(px(384.0))
/// ```
#[derive(IntoElement)]
pub struct BarVisualizer {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    levels: Option<Vec<f32>>,
    bar_count: usize,
    align: BarVisualizerAlign,
    mirrored: bool,
    min_level: f32,
    idle: BarIdle,
    loading: bool,
    orientation: Option<Orientation>,
    config: AudioConfig,
    actions: Option<BarVisualizerActions>,
    bar_gap: Pixels,
    bar_width: Pixels,
    bar_radius: Pixels,
    aria_label: SharedString,
    aria_hidden: bool,
    style: StyleRefinement,
}

impl BarVisualizer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            levels: None,
            bar_count: DEFAULT_BAR_COUNT,
            align: BarVisualizerAlign::Center,
            mirrored: false,
            min_level: DEFAULT_MIN_LEVEL,
            idle: BarIdle::Static,
            loading: false,
            orientation: None,
            config: AudioConfig::default(),
            actions: None,
            bar_gap: px(DEFAULT_BAR_GAP),
            bar_width: px(DEFAULT_BAR_WIDTH),
            bar_radius: px(DEFAULT_BAR_RADIUS),
            aria_label: "Audio visualizer".into(),
            aria_hidden: false,
            style: StyleRefinement::default(),
        }
    }

    /// A visual source; the bars follow its frequency bands.
    pub fn source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.source = Some(source);
        self
    }

    /// Like [`source`](Self::source), `None` detaches.
    pub fn source_opt(mut self, source: Option<FrameSource<VisualFrame>>) -> Self {
        self.source = source;
        self
    }

    /// Levels, 0..1, for declarative use. Removing them lets the bars fall.
    pub fn levels(mut self, levels: impl Into<Vec<f32>>) -> Self {
        self.levels = Some(levels.into());
        self
    }

    /// Number of bars. Bands are resampled to fit. Default 24.
    pub fn bar_count(mut self, bar_count: usize) -> Self {
        self.bar_count = bar_count;
        self
    }

    /// Where bars grow from. Default `Center`.
    pub fn align(mut self, align: BarVisualizerAlign) -> Self {
        self.align = align;
        self
    }

    /// Symmetric around the middle bar, lows in the centre. Default false.
    pub fn mirrored(mut self, mirrored: bool) -> Self {
        self.mirrored = mirrored;
        self
    }

    /// Resting bar size, 0..1. Default 0.08.
    pub fn min_level(mut self, min_level: f32) -> Self {
        self.min_level = min_level;
        self
    }

    /// What the bars do with no signal. Default `Static`.
    pub fn idle(mut self, idle: BarIdle) -> Self {
        self.idle = idle;
        self
    }

    /// Runs a sweep animation, for connecting or thinking states. Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// `Vertical` grows bars sideways. Default: the config's, else horizontal.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Settings from a container (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    /// Connects a [`BarVisualizerActions`] handle (`actionsRef`).
    pub fn actions(mut self, actions: &BarVisualizerActions) -> Self {
        self.actions = Some(actions.clone());
        self
    }

    /// `--bar-gap`: gap between bars. Default 3px (`0.1875rem`).
    pub fn bar_gap(mut self, gap: Pixels) -> Self {
        self.bar_gap = gap;
        self
    }

    /// `--bar-width`: maximum bar width. Default 6px (`0.375rem`).
    pub fn bar_width(mut self, width: Pixels) -> Self {
        self.bar_width = width;
        self
    }

    /// `--bar-radius`: bar corner radius. Default fully rounded.
    pub fn bar_radius(mut self, radius: Pixels) -> Self {
        self.bar_radius = radius;
        self
    }

    /// `aria-label`. Default "Audio visualizer".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }

    /// `aria-hidden`, when it is decorative next to a labelled meter.
    pub fn aria_hidden(mut self, hidden: bool) -> Self {
        self.aria_hidden = hidden;
        self
    }
}

impl Styled for BarVisualizer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Bar geometry inside `bounds`, like the flex layout: `flex-1` bars capped
/// at `bar_width`, `bar_gap` apart, centred on the main axis, sized by level
/// on the cross axis and placed by `align`.
fn bar_bounds(
    bounds: Bounds<Pixels>,
    levels: &[f32],
    horizontal: bool,
    align: BarVisualizerAlign,
    bar_gap: f32,
    bar_width: f32,
) -> Vec<Bounds<Pixels>> {
    let count = levels.len();
    if count == 0 {
        return Vec::new();
    }
    let (main, cross) = if horizontal {
        (bounds.size.width.as_f32(), bounds.size.height.as_f32())
    } else {
        (bounds.size.height.as_f32(), bounds.size.width.as_f32())
    };
    let gaps = bar_gap * (count as f32 - 1.0);
    let thickness = ((main - gaps) / count as f32).clamp(0.0, bar_width);
    let total = thickness * count as f32 + gaps;
    let start = (main - total) / 2.0;
    levels
        .iter()
        .enumerate()
        .map(|(index, &level)| {
            let along = start + index as f32 * (thickness + bar_gap);
            let length = level * cross;
            let offset = match align {
                BarVisualizerAlign::Center => (cross - length) / 2.0,
                BarVisualizerAlign::Start => 0.0,
                BarVisualizerAlign::End => cross - length,
            };
            let (x, y, w, h) = if horizontal {
                (along, offset, thickness, length)
            } else {
                (offset, along, length, thickness)
            };
            Bounds::new(
                point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
                size(px(w), px(h)),
            )
        })
        .collect()
}

impl RenderOnce for BarVisualizer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let orientation = self
            .orientation
            .or(self.config.orientation)
            .unwrap_or_default();
        let horizontal = orientation == Orientation::Horizontal;
        let options = BarLevelsOptions {
            bar_count: self.bar_count,
            idle: self.idle,
            loading: self.loading,
            min_level: self.min_level,
            mirrored: self.mirrored,
            reduced_motion: prefers_reduced_motion(cx),
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| BarVisualizerState {
            painter: BarPainter::new(options),
            levels_prop: None,
            source_version: None,
        });
        if let Some(actions) = &self.actions {
            actions.attach(&state);
        }

        let now = now_ms();
        let source = self.source.clone();
        let levels_prop = self.levels;
        let (levels, needs_frame) = state.update(cx, |state, _| {
            let painter = &mut state.painter;
            painter.set_options(options);
            // Clear only when levels go from set to unset, so the bars fall
            // instead of freezing, without wiping levels painted through the handle.
            match levels_prop {
                Some(levels) => {
                    if state.levels_prop.as_ref() != Some(&levels) {
                        painter.set_input(Some(levels.clone()));
                        state.levels_prop = Some(levels);
                    }
                }
                None => {
                    if state.levels_prop.take().is_some() {
                        painter.set_input(None);
                    }
                }
            }
            match &source {
                Some(source) => {
                    let version = source.version();
                    if state.source_version != Some(version) {
                        state.source_version = Some(version);
                        if let Some(bands) = source.with_latest(|f| f.map(|f| f.bands.clone())) {
                            painter.set_input(Some(bands));
                        }
                    }
                }
                None => state.source_version = None,
            }
            let needs_frame = painter.frame(now);
            (painter.shown(), needs_frame)
        });
        // A source can deliver a frame at any time, so keep looking for one.
        if needs_frame || source.is_some() {
            window.request_animation_frame();
        }

        let (bar_gap, bar_width, bar_radius) = (
            self.bar_gap.as_f32(),
            self.bar_width.as_f32(),
            self.bar_radius,
        );
        let align = self.align;
        let mut root = div()
            .id(self.id)
            .relative()
            .when(!self.aria_hidden, |el| {
                el.role(gpui::Role::Image).aria_label(self.aria_label)
            })
            .when(horizontal, |el| el.h(px(DEFAULT_CROSS_SIZE)).w_full())
            .when(!horizontal, |el| el.h_full().w(px(DEFAULT_CROSS_SIZE)))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let color = window.text_style().color;
                        for bar in
                            bar_bounds(bounds, &levels, horizontal, align, bar_gap, bar_width)
                        {
                            let radius = bar_radius
                                .min(bar.size.width / 2.0)
                                .min(bar.size.height / 2.0);
                            window.paint_quad(fill(bar, color).corner_radii(radius));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        root.style().refine(&self.style);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: f64 = 1000.0 / 60.0;

    fn options(bar_count: usize) -> BarLevelsOptions {
        BarLevelsOptions {
            bar_count,
            idle: BarIdle::Static,
            loading: false,
            min_level: DEFAULT_MIN_LEVEL,
            mirrored: false,
            reduced_motion: false,
        }
    }

    /// Drives the painter like the frame loop: one frame per tick while it
    /// asks for frames. Returns the time reached and whether it still wants frames.
    fn advance(painter: &mut BarPainter, now: &mut f64, ms: f64) -> bool {
        let end = *now + ms;
        let mut wants = painter.task.is_awake();
        while *now + FRAME_MS <= end {
            *now += FRAME_MS;
            if wants {
                wants = painter.frame(*now);
            }
        }
        *now = end;
        wants
    }

    #[test]
    fn bars_fall_when_levels_go_away_and_active_follows() {
        let mut painter = BarPainter::new(options(5));
        let mut now = 0.0;
        painter.set_input(Some(vec![1.0; 5]));
        advance(&mut painter, &mut now, 500.0);
        assert!(painter.shown()[2] > 0.8);
        assert!(painter.active);

        painter.set_input(None);
        advance(&mut painter, &mut now, 3000.0);
        assert!(painter.shown()[2] < 0.3);
        assert!(!painter.active);
    }

    #[test]
    fn static_bars_settle_and_new_levels_wake_them() {
        let mut painter = BarPainter::new(options(3));
        let mut now = 0.0;
        painter.set_input(Some(vec![0.6; 3]));
        let wants = advance(&mut painter, &mut now, 100.0);
        assert!((painter.shown()[0] - 0.6).abs() < 0.0005);
        assert!(!wants);

        painter.set_input(Some(vec![0.1; 3]));
        advance(&mut painter, &mut now, 50.0);
        assert!(painter.shown()[0] > 0.1);
        let wants = advance(&mut painter, &mut now, 10_000.0);
        assert!((painter.shown()[0] - 0.1).abs() < 0.005);
        assert!(!wants);
    }

    #[test]
    fn an_idle_animation_keeps_running() {
        let mut painter = BarPainter::new(BarLevelsOptions {
            idle: BarIdle::Pulse,
            ..options(3)
        });
        let mut now = 0.0;
        assert!(advance(&mut painter, &mut now, 10_000.0));
    }

    #[test]
    fn a_source_frame_wakes_sleeping_bars() {
        let mut painter = BarPainter::new(options(3));
        let mut now = 0.0;
        assert!(!advance(&mut painter, &mut now, 200.0));
        painter.set_input(Some(vec![0.7; 3]));
        let wants = advance(&mut painter, &mut now, 100.0);
        assert!((painter.shown()[0] - 0.7).abs() < 0.005);
        assert!(!wants);
    }

    #[test]
    fn rests_at_min_level_before_the_first_paint() {
        let painter = BarPainter::new(options(4));
        assert_eq!(painter.shown(), vec![DEFAULT_MIN_LEVEL; 4]);
    }

    #[test]
    fn reduced_motion_paints_four_times_a_second() {
        let mut painter = BarPainter::new(BarLevelsOptions {
            reduced_motion: true,
            ..options(1)
        });
        let mut now = 0.0;
        painter.set_input(Some(vec![0.5]));
        painter.frame(now);
        assert_eq!(painter.shown(), vec![0.5]);
        painter.set_input(Some(vec![0.2]));
        for _ in 0..4 {
            now += 50.0;
            assert!(painter.frame(now));
            assert_eq!(painter.shown(), vec![0.5]);
        }
        now += 50.0;
        painter.frame(now);
        assert_eq!(painter.shown(), vec![0.2]);
    }

    #[test]
    fn new_options_keep_the_input() {
        let mut painter = BarPainter::new(options(3));
        painter.set_input(Some(vec![0.6; 3]));
        painter.set_options(BarLevelsOptions {
            mirrored: true,
            ..options(3)
        });
        painter.frame(16.0);
        assert!((painter.shown()[1] - 0.6).abs() < 0.0005);
    }

    #[test]
    fn layout_caps_bar_width_and_centres_the_row() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(64.0)));
        let bars = bar_bounds(
            bounds,
            &[0.5, 1.0],
            true,
            BarVisualizerAlign::Center,
            DEFAULT_BAR_GAP,
            DEFAULT_BAR_WIDTH,
        );
        // 2 bars of 6px with a 3px gap = 15px, centred in 100px.
        assert_eq!(bars[0].origin.x, px(42.5));
        assert_eq!(bars[1].origin.x, px(51.5));
        assert_eq!(bars[0].size, size(px(6.0), px(32.0)));
        assert_eq!(bars[0].origin.y, px(16.0));
        assert_eq!(bars[1].origin.y, px(0.0));
    }

    #[test]
    fn layout_shrinks_bars_to_fit_and_aligns() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(28.0), px(14.0)));
        let bars = bar_bounds(bounds, &[0.5; 5], true, BarVisualizerAlign::End, 2.0, 3.0);
        // 5 × 3px + 4 × 2px = 23px, centred in 28px.
        assert_eq!(bars[0].origin.x, px(2.5));
        assert_eq!(bars[0].origin.y, px(7.0));
        let narrow = Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let bars = bar_bounds(narrow, &[1.0; 3], true, BarVisualizerAlign::Start, 2.0, 6.0);
        assert_eq!(bars[0].size.width, px(2.0));
        assert_eq!(bars[0].origin.y, px(0.0));
    }

    #[test]
    fn vertical_layout_grows_sideways() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(64.0), px(100.0)));
        let bars = bar_bounds(
            bounds,
            &[0.5],
            false,
            BarVisualizerAlign::End,
            DEFAULT_BAR_GAP,
            DEFAULT_BAR_WIDTH,
        );
        assert_eq!(bars[0].size, size(px(32.0), px(6.0)));
        assert_eq!(bars[0].origin, point(px(32.0), px(47.0)));
    }
}
