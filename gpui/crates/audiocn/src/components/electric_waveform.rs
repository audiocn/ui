//! Port of `components/ui/electric-waveform.tsx`.
//!
//! One electric line across the width: a crackling white-hot core, a tall
//! glow, forks of lightning off the peaks and sparks on sudden rises.
//!
//! **What changed for GPUI.** The React component draws on two canvases: a
//! glow canvas blurred with CSS (`blur(var(--electric-glow-size))`) under a
//! sharp one, and cuts the ends away with a `destination-out` gradient. GPUI
//! has neither filters nor composite modes, so:
//!
//! - The sharp layer is stroked with `PathBuilder` exactly as the canvas
//!   strokes it (same widths, alphas, joins; round caps added as half discs).
//! - The blurred layer is rebuilt from Gaussian-blurred boxes
//!   (`paint_drop_shadows`, whose blur *is* a Gaussian of the given sigma): the
//!   glow ribbons become one blurred column per point, clipped to the box like
//!   the canvas clips them, and the wide glow strokes become a chain of blurred
//!   boxes with the same area and spread as each stroke piece. Overlapping
//!   pieces are composited in optical density (`-ln(1 - alpha)`), so stacked
//!   pieces add up to the canvas's alpha.
//! - The faded ends paint each sharp path through three clip strips, with a
//!   linear gradient over the two end strips; glow pieces are scaled by the
//!   same ramp.
//!
//! The moving parts ([`ElectricTrace`]) are a straight port and are unit-tested.

use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{
    App, BoxShadow, ContentMask, Corners, ElementId, Hsla, IntoElement, ParentElement, Path,
    PathBuilder, PathStyle, Pixels, Point, Refineable, RenderOnce, SharedString, StrokeOptions,
    StyleRefinement, Styled, Window, canvas, div, linear_color_stop, linear_gradient, point, px,
    size,
};

use crate::audio::decibels::clamp;
use crate::audio::electric::{
    ELECTRIC_GLOW_OPACITY_DARK, ELECTRIC_GLOW_OPACITY_LIGHT, ELECTRIC_GLOW_SIZE,
    ELECTRIC_HEAT_DARK, ELECTRIC_HEAT_LIGHT, ElectricColors, ElectricSparks, SeededRandom,
    SparkLaunch, create_electric_sparks, create_random, displace, electric_colors, emit_spark,
    move_sparks, spark_streaks,
};
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::types::VisualFrame;
use crate::audio::wave_line::{WAVE_LINE_MAX_POINTS, WaveLine, WaveLineMode, WaveLineOptions};
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::ActiveTheme;

const DEFAULT_INTENSITY: f32 = 0.6;
const DEFAULT_LINE_WIDTH: f32 = 3.0;
pub(crate) const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
const MS_PER_SECOND: f64 = 1000.0;
const FRAME_MS: f64 = 16.67;
const MAX_STEP_SECONDS: f64 = 0.05;
/// Width of the faded ends, in pixels.
const FADE_PX: f32 = 24.0;

/// One point every this many pixels along the line.
const POINT_SPACING_PX: f32 = 4.0;
/// Crackle is built in short chunks of 2^2 segments that share their ends.
const CHUNK: usize = 4;
const MIN_POINTS: usize = CHUNK * 8 + 1;
const MAX_POINTS: usize = WAVE_LINE_MAX_POINTS;
/// How much each finer split of the crackle bends compared with the one before.
const ROUGHNESS: f32 = 0.8;
/// Each jagged shape holds at least this long, so the crackle never strobes.
const JITTER_MS: f64 = 42.0;
/// Brightness flicker, at most 12%.
const FLICKER: f64 = 0.12;
/// Crackle at rest, in pixels at full intensity.
const HUM_PX: f32 = 1.5;
/// Extra crackle at full level, in pixels at full intensity.
const CRACKLE_PX: f32 = 6.0;

const MAX_BRANCHES: usize = 3;
/// Points along a fork (2^3 segments).
pub const BRANCH_POINTS: usize = 9;
/// A point must be this far from the middle to throw a fork.
const BRANCH_THRESHOLD: f32 = 0.3;
const BRANCH_CHANCE: f64 = 0.6;
const BRANCH_CANDIDATES: usize = 3;
const BRANCH_MIN_PX: f64 = 10.0;
const BRANCH_MAX_PX: f64 = 30.0;
const BRANCH_MIN_MS: f64 = 60.0;
const BRANCH_MAX_MS: f64 = 140.0;
/// How far a fork bends, relative to its length.
const BRANCH_BOW: f32 = 0.35;

const MAX_SPARKS: usize = 48;
/// A rise in loudness this big in one frame throws sparks.
const SPARK_RISE: f32 = 0.1;
const SPARK_MIN_LEVEL: f32 = 0.3;
const SPARK_COOLDOWN_MS: f64 = 90.0;
/// Pixels per second squared.
const SPARK_GRAVITY: f32 = 520.0;
/// Pixels per second.
const SPARK_SPEED: f64 = 170.0;
/// Pixels per second, sideways.
const SPARK_SPREAD: f64 = 90.0;
const SPARK_MIN_MS: f64 = 260.0;
const SPARK_MAX_MS: f64 = 560.0;
pub(crate) const SPARK_TRAIL_SECONDS: f32 = 0.03;

/// `wave` draws a smooth wave shaped by the frequency bands; `scope` draws
/// the signal itself, like an oscilloscope.
pub type ElectricWaveformMode = WaveLineMode;

/// A fork of lightning off the line.
#[derive(Clone, Debug, PartialEq)]
pub struct ElectricBranch {
    /// The point on the line it leaves from.
    pub point: usize,
    /// 1 up, −1 down: away from the middle.
    pub direction: f32,
    /// Pixels.
    pub length: f32,
    /// Sideways drift of the far end, relative to the length.
    pub lean: f32,
    pub born_ms: f64,
    /// 0 when the fork is gone.
    pub life_ms: f64,
    /// Bend at each point, in units of the fork's bow.
    pub jitter: [f32; BRANCH_POINTS],
}

/// The box the line is drawn in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricTraceGeometry {
    /// Logical pixels.
    pub width: f32,
    pub height: f32,
    pub line_width: f32,
}

/// Options for [`create_electric_trace`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricTraceOptions {
    pub mode: ElectricWaveformMode,
    pub loading: bool,
    pub intensity: f32,
    pub arcs: bool,
    pub sparks: bool,
    pub sensitivity: f32,
    /// A still line: no crackle, forks, sparks or drift.
    pub reduced_motion: bool,
    /// Makes the crackle repeatable. Each trace gets its own by default.
    pub seed: Option<f64>,
}

impl Default for ElectricTraceOptions {
    fn default() -> Self {
        Self {
            mode: WaveLineMode::Wave,
            loading: false,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            sensitivity: 1.0,
            reduced_motion: false,
            seed: None,
        }
    }
}

/// Points for a width: one every few pixels, in whole crackle chunks.
fn point_count_for(width: f32) -> usize {
    let chunks = ((width / POINT_SPACING_PX / CHUNK as f32) as f64 + 0.5).floor();
    (chunks * CHUNK as f64 + 1.0).clamp(MIN_POINTS as f64, MAX_POINTS as f64) as usize
}

fn middle_of(geometry: &ElectricTraceGeometry) -> f32 {
    geometry.height / 2.0
}

/// Pixels from the middle to a height of 1, leaving room for the line.
fn reach_of(geometry: &ElectricTraceGeometry) -> f32 {
    (geometry.height / 2.0 - (geometry.line_width * 2.0).max(6.0)).max(0.0)
}

fn x_of(geometry: &ElectricTraceGeometry, count: usize, point: usize) -> f32 {
    (point as f32 / (count as f32 - 1.0)) * geometry.width
}

static TRACE_COUNT: AtomicU64 = AtomicU64::new(0);

/// The moving parts of the electric line, apart from any drawing: its shape,
/// crackle, forks and sparks. With reduced motion it is a still, smooth line.
///
/// ```ignore
/// let mut trace = create_electric_trace(ElectricTraceOptions::default());
/// let active = trace.step(now_ms(), frame.as_ref(), geometry);
/// ```
#[derive(Clone, Debug)]
pub struct ElectricTrace {
    /// Points in use along the width.
    pub count: usize,
    /// Crackle at each point, in units of its amplitude.
    pub crackle: Vec<f32>,
    /// How loud the line is, 0..1.
    pub loudness: f32,
    /// Brightness, 0.88..1.
    pub flicker: f32,
    pub branches: Vec<ElectricBranch>,
    pub sparks: ElectricSparks,
    options: ElectricTraceOptions,
    random: SeededRandom,
    line: WaveLine,
    last_ms: f64,
    last_jitter_ms: f64,
    last_spark_ms: f64,
    last_step_seconds: f32,
    previous_peak: f32,
    primed: bool,
}

/// Creates an [`ElectricTrace`] (`createElectricTrace`).
pub fn create_electric_trace(options: ElectricTraceOptions) -> ElectricTrace {
    let count = TRACE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    ElectricTrace {
        count: MIN_POINTS,
        crackle: vec![0.0; MAX_POINTS],
        loudness: 0.0,
        flicker: 1.0,
        branches: (0..MAX_BRANCHES)
            .map(|_| ElectricBranch {
                point: 0,
                direction: 1.0,
                length: 0.0,
                lean: 0.0,
                born_ms: 0.0,
                life_ms: 0.0,
                jitter: [0.0; BRANCH_POINTS],
            })
            .collect(),
        sparks: create_electric_sparks(MAX_SPARKS),
        options,
        random: create_random(options.seed.unwrap_or(count as f64)),
        line: WaveLine::new(WaveLineOptions {
            mode: options.mode,
            loading: options.loading,
            sensitivity: options.sensitivity,
            reduced_motion: options.reduced_motion,
        }),
        last_ms: 0.0,
        last_jitter_ms: f64::NEG_INFINITY,
        last_spark_ms: f64::NEG_INFINITY,
        last_step_seconds: 0.0,
        previous_peak: 0.0,
        primed: false,
    }
}

impl ElectricTrace {
    /// Height of the line at each point, −1..1, up positive (the first
    /// [`count`](Self::count) are in use).
    pub fn heights(&self) -> &[f32] {
        &self.line.heights
    }

    /// The options this trace was created with.
    pub fn options(&self) -> ElectricTraceOptions {
        self.options
    }

    fn height(&self, point: usize) -> f32 {
        self.line.heights.get(point).copied().unwrap_or(0.0)
    }

    fn signed(&mut self) -> f64 {
        self.random.next() * 2.0 - 1.0
    }

    /// Crackle in pixels at a point: louder lines and higher peaks crackle more.
    fn crackle_at(&self, intensity: f32, point: usize) -> f32 {
        intensity * (HUM_PX + CRACKLE_PX * (0.4 * self.loudness + 0.6 * self.height(point).abs()))
    }

    /// Where a point of the line is drawn, in pixels down from the top.
    pub fn y_of(
        &self,
        geometry: &ElectricTraceGeometry,
        intensity: f32,
        point: usize,
        scale: f32,
        crackle_share: f32,
    ) -> f32 {
        middle_of(geometry)
            - self.height(point) * scale * reach_of(geometry)
            - self.crackle.get(point).copied().unwrap_or(0.0)
                * self.crackle_at(intensity, point)
                * crackle_share
    }

    fn reroll_crackle(&mut self) {
        self.crackle[0] = (self.signed() * 0.5) as f32;
        let mut start = 0;
        while start + CHUNK < self.count {
            let from = self.crackle[start];
            let to = (self.signed() * 0.5) as f32;
            let random = &mut self.random;
            displace(
                &mut self.crackle,
                start,
                CHUNK + 1,
                [from, to],
                || random.next(),
                ROUGHNESS,
            );
            start += CHUNK;
        }
        self.flicker = (1.0 - FLICKER * self.random.next()) as f32;
    }

    /// Picks the highest of a few random points, so forks favour the peaks.
    fn pick_peak(&mut self) -> usize {
        let count = self.count as f64;
        let mut best = (self.random.next() * count).floor() as usize;
        for _ in 1..BRANCH_CANDIDATES {
            let point = (self.random.next() * count).floor() as usize;
            if self.height(point).abs() > self.height(best).abs() {
                best = point;
            }
        }
        best
    }

    fn update_branches(&mut self, now_ms: f64) {
        let point = self.pick_peak();
        let height = self.height(point);
        let intensity = self.options.intensity as f64;
        let charged = height.abs() >= BRANCH_THRESHOLD
            && self.random.next() < BRANCH_CHANCE * intensity * height.abs() as f64;
        let free = self.branches.iter().position(|b| b.life_ms == 0.0);
        if charged && let Some(free) = free {
            let length = (BRANCH_MIN_PX + self.random.next() * (BRANCH_MAX_PX - BRANCH_MIN_PX))
                * (0.6 + 0.8 * intensity);
            let lean = self.signed() * 0.6;
            let life = BRANCH_MIN_MS + self.random.next() * (BRANCH_MAX_MS - BRANCH_MIN_MS);
            let branch = &mut self.branches[free];
            branch.point = point;
            branch.direction = if height >= 0.0 { 1.0 } else { -1.0 };
            branch.length = length as f32;
            branch.lean = lean as f32;
            branch.born_ms = now_ms;
            branch.life_ms = life;
        }
        for index in 0..self.branches.len() {
            if self.branches[index].life_ms > 0.0 {
                let end = (self.signed() * 0.5) as f32;
                let random = &mut self.random;
                displace(
                    &mut self.branches[index].jitter,
                    0,
                    BRANCH_POINTS,
                    [0.0, end],
                    || random.next(),
                    ROUGHNESS,
                );
            }
        }
    }

    fn expire_branches(&mut self, now_ms: f64) {
        for branch in &mut self.branches {
            if branch.life_ms > 0.0 && now_ms - branch.born_ms >= branch.life_ms {
                branch.life_ms = 0.0;
            }
        }
    }

    fn throw_sparks(&mut self, now_ms: f64, geometry: &ElectricTraceGeometry) {
        let rise = self.line.peak - self.previous_peak;
        let rested = now_ms - self.last_spark_ms >= SPARK_COOLDOWN_MS;
        if rise < SPARK_RISE || self.line.peak < SPARK_MIN_LEVEL || !rested {
            return;
        }
        self.last_spark_ms = now_ms;
        let mut peak = 0;
        for point in 1..self.count {
            if self.height(point).abs() > self.height(peak).abs() {
                peak = point;
            }
        }
        let upward = if self.height(peak) >= 0.0 { -1.0 } else { 1.0 };
        let intensity = self.options.intensity;
        let x = x_of(geometry, self.count, peak);
        let y = self.y_of(geometry, intensity, peak, 1.0, 1.0);
        let count = 3 + (self.random.next() * 3.0).floor() as usize;
        for _ in 0..count {
            let life_ms = SPARK_MIN_MS + self.random.next() * (SPARK_MAX_MS - SPARK_MIN_MS);
            let vx = self.signed() * SPARK_SPREAD;
            let vy = upward * SPARK_SPEED * (0.5 + self.random.next()) * (0.5 + intensity as f64);
            emit_spark(
                &mut self.sparks,
                SparkLaunch {
                    x,
                    y,
                    vx: vx as f32,
                    vy: vy as f32,
                    life_ms: life_ms as f32,
                },
            );
        }
    }

    fn animate(&mut self, now_ms: f64, geometry: &ElectricTraceGeometry) {
        if now_ms - self.last_jitter_ms >= JITTER_MS {
            self.last_jitter_ms = now_ms;
            self.reroll_crackle();
            if self.options.arcs {
                self.update_branches(now_ms);
            }
        }
        self.expire_branches(now_ms);
        if self.options.sparks {
            if self.primed {
                self.throw_sparks(now_ms, geometry);
            }
            move_sparks(&mut self.sparks, self.last_step_seconds, SPARK_GRAVITY);
        }
    }

    /// Advances to `now_ms` with the latest frame. Returns true when there is a signal.
    pub fn step(
        &mut self,
        now_ms: f64,
        frame: Option<&VisualFrame>,
        geometry: ElectricTraceGeometry,
    ) -> bool {
        let elapsed_ms = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            now_ms - self.last_ms
        };
        self.last_ms = now_ms;
        self.last_step_seconds = (elapsed_ms / MS_PER_SECOND).clamp(0.0, MAX_STEP_SECONDS) as f32;
        let active = self
            .line
            .step(now_ms, frame, point_count_for(geometry.width) as f32);
        self.count = self.line.count;
        self.loudness = self.line.loudness;
        if !self.options.reduced_motion {
            self.animate(now_ms, &geometry);
        }
        self.previous_peak = self.line.peak;
        self.primed = true;
        active
    }

    fn line_points(
        &self,
        geometry: &ElectricTraceGeometry,
        origin: Point<Pixels>,
        intensity: f32,
        scale: f32,
        crackle_share: f32,
    ) -> Vec<Point<Pixels>> {
        (0..self.count)
            .map(|p| {
                origin
                    + point(
                        px(x_of(geometry, self.count, p)),
                        px(self.y_of(geometry, intensity, p, scale, crackle_share)),
                    )
            })
            .collect()
    }

    fn branch_points(
        &self,
        geometry: &ElectricTraceGeometry,
        origin: Point<Pixels>,
        intensity: f32,
        branch: &ElectricBranch,
    ) -> Vec<Point<Pixels>> {
        let at = branch.point.min(self.count - 1);
        let x = x_of(geometry, self.count, at);
        let y = self.y_of(geometry, intensity, at, 1.0, 1.0);
        let dx = branch.lean * branch.length;
        let dy = -branch.direction * branch.length;
        let length = match dx.hypot(dy) {
            0.0 => 1.0,
            length => length,
        };
        let bow = length * BRANCH_BOW;
        (0..BRANCH_POINTS)
            .map(|index| {
                let progress = index as f32 / (BRANCH_POINTS - 1) as f32;
                let bend = branch.jitter[index] * bow;
                origin
                    + point(
                        px(x + dx * progress - (dy / length) * bend),
                        px(y + dy * progress + (dx / length) * bend),
                    )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Drawing helpers shared with the electric bar visualizer.
// ---------------------------------------------------------------------------

/// The canvas `lineJoin = "round"`: lyon has no round join without naming its
/// types, so the miter limit of 1 turns every corner into a bevel.
pub(crate) const ROUND_JOIN: f32 = 1.0;
/// The canvas `lineJoin = "miter"` with its default `miterLimit` of 10.
pub(crate) const MITER_JOIN: f32 = 10.0;

/// The blurred layer: colour, blur sigma (CSS `blur()` radius) and opacity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlowLayer {
    pub color: Hsla,
    pub sigma: f32,
    pub opacity: f32,
}

/// Optical density of an alpha: stacked densities add up like stacked alphas.
pub(crate) fn density(alpha: f32) -> f32 {
    -(1.0 - alpha.clamp(0.0, 0.995)).ln()
}

/// A Gaussian-blurred box (`radius` rounds its corners).
pub(crate) fn glow_box(
    window: &mut Window,
    origin: Point<f32>,
    extent: Point<f32>,
    radius: f32,
    glow: &GlowLayer,
    alpha: f32,
) {
    let alpha = alpha * glow.opacity;
    if alpha <= 0.002 || extent.x <= 0.0 || extent.y <= 0.0 {
        return;
    }
    window.paint_drop_shadows(
        gpui::Bounds {
            origin: point(px(origin.x), px(origin.y)),
            size: size(px(extent.x), px(extent.y)),
        },
        Corners::all(px(radius.min(extent.x / 2.0).min(extent.y / 2.0))),
        &[BoxShadow {
            color: glow.color.opacity(alpha.min(1.0)),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(glow.sigma.max(0.0)),
            spread_radius: px(0.0),
            inset: false,
        }],
    );
}

/// A blurred stroke along `points`: each piece of the stroke becomes a
/// blurred box with the same area and spread. `alpha` is the stroke's alpha;
/// `fade` scales each piece by its x position.
pub(crate) fn glow_polyline(
    window: &mut Window,
    points: &[Point<Pixels>],
    width: f32,
    alpha: f32,
    glow: &GlowLayer,
    fade: &dyn Fn(f32) -> f32,
) {
    let piece_density = density(alpha);
    if piece_density <= 0.0 || width <= 0.0 {
        return;
    }
    for pair in points.windows(2) {
        let (x0, y0) = (pair[0].x.as_f32(), pair[0].y.as_f32());
        let (x1, y1) = (pair[1].x.as_f32(), pair[1].y.as_f32());
        let (dx, dy) = (x1 - x0, y1 - y0);
        let length = dx.hypot(dy);
        if length < 0.01 {
            continue;
        }
        let pieces = (length / width.max(2.0)).ceil().max(1.0);
        let piece = length / pieces;
        let (cos, sin) = (dx / length, dy / length);
        // An axis-aligned box with the spread of the rotated piece.
        let box_w = (piece * piece * cos * cos + width * width * sin * sin).sqrt();
        let box_h = (piece * piece * sin * sin + width * width * cos * cos).sqrt();
        let mass = (piece * width) / (box_w * box_h);
        for index in 0..pieces as usize {
            let t = (index as f32 + 0.5) / pieces;
            let (cx, cy) = (x0 + dx * t, y0 + dy * t);
            glow_box(
                window,
                point(cx - box_w / 2.0, cy - box_h / 2.0),
                point(box_w, box_h),
                0.0,
                glow,
                (piece_density * mass).min(1.0) * fade(cx),
            );
        }
    }
}

/// A stroke along `points` (no caps).
pub(crate) fn stroke_path(
    points: &[Point<Pixels>],
    width: f32,
    miter_limit: f32,
) -> Option<Path<Pixels>> {
    if points.len() < 2 || width <= 0.0 {
        return None;
    }
    let options = StrokeOptions::default()
        .with_line_width(width)
        .with_miter_limit(miter_limit.max(1.0));
    let mut builder = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
    builder.move_to(points[0]);
    for p in &points[1..] {
        builder.line_to(*p);
    }
    builder.build().ok()
}

/// The canvas `lineCap = "round"`: a half disc past each end of the polyline.
pub(crate) fn round_caps(points: &[Point<Pixels>], width: f32) -> Option<Path<Pixels>> {
    const STEPS: usize = 8;
    if points.len() < 2 || width < 1.0 {
        return None;
    }
    let radius = width / 2.0;
    let mut builder = PathBuilder::fill();
    let mut any = false;
    let ends = [
        (points[0], points[1]),
        (points[points.len() - 1], points[points.len() - 2]),
    ];
    for (end, before) in ends {
        let (ex, ey) = (end.x.as_f32(), end.y.as_f32());
        let (dx, dy) = (ex - before.x.as_f32(), ey - before.y.as_f32());
        let length = dx.hypot(dy);
        if length < 0.01 {
            continue;
        }
        let (ux, uy) = (dx / length, dy / length);
        let (nx, ny) = (-uy, ux);
        let cap: Vec<Point<Pixels>> = (0..=STEPS)
            .map(|step| {
                let angle = PI * step as f32 / STEPS as f32;
                let (c, s) = (angle.cos(), angle.sin());
                point(
                    px(ex + radius * (nx * c + ux * s)),
                    px(ey + radius * (ny * c + uy * s)),
                )
            })
            .collect();
        builder.add_polygon(&cap, true);
        any = true;
    }
    if any { builder.build().ok() } else { None }
}

/// A filled polygon.
pub(crate) fn fill_polygon(points: &[Point<Pixels>]) -> Option<Path<Pixels>> {
    if points.len() < 3 {
        return None;
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(points, true);
    builder.build().ok()
}

/// The ends cut away by a `destination-out` gradient, as a multiplier by x.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeFade {
    pub left: f32,
    pub right: f32,
    pub edge: f32,
}

impl EdgeFade {
    pub fn at(&self, x: f32) -> f32 {
        if self.edge <= 0.0 {
            return 1.0;
        }
        ((x - self.left) / self.edge)
            .min((self.right - x) / self.edge)
            .clamp(0.0, 1.0)
    }
}

/// Paints a path, faded toward the ends when `fade` is set: the solid middle
/// as is, each end strip through a clip with a gradient over the path's part
/// of the strip.
pub(crate) fn paint_faded(
    window: &mut Window,
    path: Path<Pixels>,
    color: Hsla,
    fade: Option<&EdgeFade>,
) {
    if color.a <= 0.0 {
        return;
    }
    let Some(fade) = fade.filter(|f| f.edge > 0.0) else {
        window.paint_path(path, color);
        return;
    };
    let bounds = path.bounds;
    let (x0, x1) = (
        bounds.origin.x.as_f32(),
        (bounds.origin.x + bounds.size.width).as_f32(),
    );
    let (top, height) = (bounds.origin.y, bounds.size.height);
    let strips = [
        (fade.left, fade.left + fade.edge, true),
        (fade.left + fade.edge, fade.right - fade.edge, false),
        (fade.right - fade.edge, fade.right, true),
    ];
    for (from, to, ramp) in strips {
        let (clip_from, clip_to) = (x0.max(from), x1.min(to));
        if clip_to <= clip_from {
            continue;
        }
        let mask = ContentMask {
            bounds: gpui::Bounds {
                origin: point(px(clip_from), top - px(1.0)),
                size: size(px(clip_to - clip_from), height + px(2.0)),
            },
        };
        let path = path.clone();
        window.with_content_mask(Some(mask), |window| {
            if ramp {
                window.paint_path(
                    path,
                    linear_gradient(
                        90.0,
                        linear_color_stop(color.opacity(fade.at(clip_from)), 0.0),
                        linear_color_stop(color.opacity(fade.at(clip_to)), 1.0),
                    ),
                );
            } else {
                window.paint_path(path, color);
            }
        });
    }
}

/// Strokes a polyline with round caps, faded toward the ends when asked.
pub(crate) fn paint_stroke(
    window: &mut Window,
    points: &[Point<Pixels>],
    width: f32,
    miter_limit: f32,
    color: Hsla,
    fade: Option<&EdgeFade>,
) {
    if color.a <= 0.002 {
        return;
    }
    if let Some(path) = stroke_path(points, width, miter_limit) {
        paint_faded(window, path, color, fade);
    }
    if let Some(caps) = round_caps(points, width) {
        paint_faded(window, caps, color, fade);
    }
}

/// Each live spark as a short streak that fades with age (`strokeSparks`).
pub(crate) fn paint_sparks(
    window: &mut Window,
    origin: Point<Pixels>,
    sparks: &ElectricSparks,
    color: Hsla,
    width: f32,
    fade: Option<&EdgeFade>,
) {
    for streak in spark_streaks(sparks, SPARK_TRAIL_SECONDS) {
        let from = origin + point(px(streak.from.0), px(streak.from.1));
        let to = origin + point(px(streak.to.0), px(streak.to.1));
        let faded = fade.map_or(1.0, |f| f.at((from.x.as_f32() + to.x.as_f32()) / 2.0));
        let alpha = streak.alpha * faded;
        if alpha <= 0.002 {
            continue;
        }
        if let Some(path) = stroke_path(&[from, to], width, MITER_JOIN) {
            window.paint_path(path, color.opacity(alpha));
        }
    }
}

/// Resolves the drawing colours like the CSS variables do.
pub(crate) fn resolve_colors(
    current: Hsla,
    electric: Option<Hsla>,
    core: Option<Hsla>,
    glow: Option<Hsla>,
    heat: Option<f32>,
    dark: bool,
) -> ElectricColors {
    let heat = heat.unwrap_or(if dark {
        ELECTRIC_HEAT_DARK
    } else {
        ELECTRIC_HEAT_LIGHT
    });
    electric_colors(electric.unwrap_or(current), core, glow, heat)
}

/// The glow layer's opacity (`opacity-70 dark:opacity-100`).
pub(crate) fn glow_opacity(dark: bool) -> f32 {
    if dark {
        ELECTRIC_GLOW_OPACITY_DARK
    } else {
        ELECTRIC_GLOW_OPACITY_LIGHT
    }
}

// ---------------------------------------------------------------------------
// Painting the trace.
// ---------------------------------------------------------------------------

struct PaintInput<'a> {
    trace: &'a ElectricTrace,
    geometry: ElectricTraceGeometry,
    origin: Point<Pixels>,
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
    fade: Option<EdgeFade>,
}

impl PaintInput<'_> {
    fn branch_alpha(&self, branch: &ElectricBranch, alpha: f32) -> f32 {
        let age = ((self.now_ms - branch.born_ms) / branch.life_ms) as f32;
        clamp(alpha * (1.0 - age * age), 0.0, 1.0)
    }

    fn fade_at(&self, x: f32) -> f32 {
        self.fade.as_ref().map_or(1.0, |f| f.at(x))
    }
}

/// The band between two scaled copies of the line, as blurred columns: one
/// per segment, clipped to the box like the canvas clips it.
fn glow_ribbon(
    window: &mut Window,
    input: &PaintInput,
    glow: &GlowLayer,
    inner: f32,
    outer: f32,
    alpha: f32,
) {
    let PaintInput {
        trace, geometry, ..
    } = input;
    let column_density = density(alpha);
    let (ox, oy) = (input.origin.x.as_f32(), input.origin.y.as_f32());
    let edge = |p: usize, scale: f32| trace.y_of(geometry, input.intensity, p, scale, 0.35);
    for p in 0..trace.count.saturating_sub(1) {
        let x0 = x_of(geometry, trace.count, p);
        let x1 = x_of(geometry, trace.count, p + 1);
        let a = (edge(p, outer) + edge(p + 1, outer)) / 2.0;
        let b = (edge(p, inner) + edge(p + 1, inner)) / 2.0;
        let top = a.min(b).clamp(0.0, geometry.height);
        let bottom = a.max(b).clamp(0.0, geometry.height);
        if bottom - top < 0.25 {
            continue;
        }
        let fade = input.fade_at(ox + (x0 + x1) / 2.0);
        glow_box(
            window,
            point(ox + x0, oy + top),
            point(x1 - x0, bottom - top),
            0.0,
            glow,
            column_density.min(1.0) * fade,
        );
    }
}

/// The tall, soft glow: bands that echo the wave above and below it, and a
/// wide stroke, all blurred.
fn paint_glow(window: &mut Window, input: &PaintInput, glow: &GlowLayer) {
    let trace = input.trace;
    let geometry = &input.geometry;
    let strength = (0.4 + 0.6 * trace.loudness) * trace.flicker;
    glow_ribbon(window, input, glow, 0.2, 2.6, 0.22 * strength);
    glow_ribbon(window, input, glow, 0.6, 1.6, 0.4 * strength);
    let fade = |x: f32| input.fade_at(x);
    let line = trace.line_points(geometry, input.origin, input.intensity, 1.0, 0.6);
    glow_polyline(
        window,
        &line,
        geometry.line_width * 4.0,
        0.6 * strength,
        glow,
        &fade,
    );
    for branch in trace.branches.iter().filter(|b| b.life_ms > 0.0) {
        let points = trace.branch_points(geometry, input.origin, input.intensity, branch);
        glow_polyline(
            window,
            &points,
            geometry.line_width * 1.5,
            input.branch_alpha(branch, 0.7),
            glow,
            &fade,
        );
    }
}

fn stroke_branches(window: &mut Window, input: &PaintInput, color: Hsla, width: f32, alpha: f32) {
    for branch in input.trace.branches.iter().filter(|b| b.life_ms > 0.0) {
        let points =
            input
                .trace
                .branch_points(&input.geometry, input.origin, input.intensity, branch);
        paint_stroke(
            window,
            &points,
            width,
            MITER_JOIN,
            color.opacity(input.branch_alpha(branch, alpha)),
            input.fade.as_ref(),
        );
    }
}

fn paint_main(window: &mut Window, input: &PaintInput) {
    let trace = input.trace;
    let geometry = &input.geometry;
    let fade = input.fade.as_ref();
    let body = trace.line_points(geometry, input.origin, input.intensity, 1.0, 0.6);
    paint_stroke(
        window,
        &body,
        geometry.line_width,
        ROUND_JOIN,
        input.colors.body.opacity(0.9 * trace.flicker),
        fade,
    );
    let thin = (geometry.line_width * 0.5).max(1.5);
    stroke_branches(window, input, input.colors.body, thin, 0.7);
    let core = trace.line_points(geometry, input.origin, input.intensity, 1.0, 1.0);
    paint_stroke(
        window,
        &core,
        thin,
        MITER_JOIN,
        input.colors.core.opacity(trace.flicker),
        fade,
    );
    stroke_branches(window, input, input.colors.core, 1.0, 1.0);
    paint_sparks(
        window,
        input.origin,
        &trace.sparks,
        input.colors.core,
        (geometry.line_width * 0.4).max(1.0),
        fade,
    );
}

// ---------------------------------------------------------------------------
// The component.
// ---------------------------------------------------------------------------

/// Paints frames into an [`ElectricWaveform`] directly (`actionsRef`).
///
/// ```ignore
/// let actions = ElectricWaveformActions::new();
/// ElectricWaveform::new("line").actions(actions.clone());
/// actions.paint(frame); // later
/// ```
#[derive(Clone, Default)]
pub struct ElectricWaveformActions {
    /// The last command: `Some(frame)` paints, `None` clears.
    pending: Rc<RefCell<Option<Option<VisualFrame>>>>,
}

impl ElectricWaveformActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint a frame directly.
    pub fn paint(&self, frame: VisualFrame) {
        *self.pending.borrow_mut() = Some(Some(frame));
    }

    /// Forget the last frame, so the line falls flat.
    pub fn clear(&self) {
        *self.pending.borrow_mut() = Some(None);
    }

    fn take(&self) -> Option<Option<VisualFrame>> {
        self.pending.borrow_mut().take()
    }
}

struct ElectricWaveformState {
    options: ElectricTraceOptions,
    trace: ElectricTrace,
    frame: Option<VisualFrame>,
    /// The source last read from, and its version then.
    source_seen: Option<(FrameSource<VisualFrame>, u64)>,
    last_paint_ms: f64,
    paint_ms: f64,
}

/// One electric line across the width, with a crackling white-hot core, a
/// tall glow, forks off the peaks and sparks. It follows the text colour.
///
/// ```ignore
/// ElectricWaveform::new("voice")
///     .text_color(cx.theme().primary)
///     .h(px(128.))
///     .source(signal.visual.clone())
/// ```
#[derive(IntoElement)]
pub struct ElectricWaveform {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    mode: ElectricWaveformMode,
    loading: bool,
    intensity: f32,
    arcs: bool,
    sparks: bool,
    sensitivity: f32,
    line_width: f32,
    fade_edges: bool,
    actions: Option<ElectricWaveformActions>,
    electric: Option<Hsla>,
    electric_core: Option<Hsla>,
    electric_heat: Option<f32>,
    electric_glow: Option<Hsla>,
    electric_glow_size: f32,
    aria_label: SharedString,
    style: StyleRefinement,
}

impl ElectricWaveform {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            mode: WaveLineMode::Wave,
            loading: false,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            sensitivity: 1.0,
            line_width: DEFAULT_LINE_WIDTH,
            fade_edges: true,
            actions: None,
            electric: None,
            electric_core: None,
            electric_heat: None,
            electric_glow: None,
            electric_glow_size: ELECTRIC_GLOW_SIZE,
            aria_label: "Audio waveform".into(),
            style: StyleRefinement::default(),
        }
    }

    /// A visual source; the line follows its bands (`wave`) or samples (`scope`).
    pub fn source(mut self, source: impl Into<Option<FrameSource<VisualFrame>>>) -> Self {
        self.source = source.into();
        self
    }

    /// `wave` draws a smooth wave shaped by the frequency bands. `scope` draws
    /// the signal itself, like an oscilloscope. Default `wave`.
    pub fn mode(mut self, mode: ElectricWaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Runs a pulse along the line, for connecting or thinking states. Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// How jagged and restless the line is, 0..1. Default 0.6.
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    /// Forks of lightning branch off the peaks. Default true.
    pub fn arcs(mut self, arcs: bool) -> Self {
        self.arcs = arcs;
        self
    }

    /// Sparks fly off the peaks on sudden rises. Default true.
    pub fn sparks(mut self, sparks: bool) -> Self {
        self.sparks = sparks;
        self
    }

    /// Visual gain. Default 1.
    pub fn sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Line width in pixels. Default 3.
    pub fn line_width(mut self, line_width: f32) -> Self {
        self.line_width = line_width;
        self
    }

    /// Fade the left and right ends. Default true.
    pub fn fade_edges(mut self, fade_edges: bool) -> Self {
        self.fade_edges = fade_edges;
        self
    }

    /// `actionsRef`: paint or clear frames directly.
    pub fn actions(mut self, actions: ElectricWaveformActions) -> Self {
        self.actions = Some(actions);
        self
    }

    /// `--electric`: line colour. Default the text colour.
    pub fn electric(mut self, color: impl Into<Hsla>) -> Self {
        self.electric = Some(color.into());
        self
    }

    /// `--electric-core`: core colour. Default `--electric` mixed toward white by the heat.
    pub fn electric_core(mut self, color: impl Into<Hsla>) -> Self {
        self.electric_core = Some(color.into());
        self
    }

    /// `--electric-heat`, 0..1: how white-hot the core runs. Default 0 in light mode, 0.7 in dark.
    pub fn electric_heat(mut self, heat: f32) -> Self {
        self.electric_heat = Some(heat);
        self
    }

    /// `--electric-glow`: glow colour. Default `--electric`.
    pub fn electric_glow(mut self, color: impl Into<Hsla>) -> Self {
        self.electric_glow = Some(color.into());
        self
    }

    /// `--electric-glow-size`: glow blur in pixels. Default 8 (0.5rem).
    pub fn electric_glow_size(mut self, pixels: f32) -> Self {
        self.electric_glow_size = pixels;
        self
    }

    /// `aria-label`. Default "Audio waveform".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }
}

impl Styled for ElectricWaveform {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ElectricWaveform {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let reduced_motion = prefers_reduced_motion(cx);
        let options = ElectricTraceOptions {
            mode: self.mode,
            loading: self.loading,
            intensity: clamp(self.intensity, 0.0, 1.0),
            arcs: self.arcs,
            sparks: self.sparks,
            sensitivity: self.sensitivity,
            reduced_motion,
            seed: None,
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ElectricWaveformState {
            options,
            trace: create_electric_trace(options),
            frame: None,
            source_seen: None,
            last_paint_ms: 0.0,
            paint_ms: 0.0,
        });
        state.update(cx, |state, _| {
            // A new trace when anything but the stroke changes (the effect's deps).
            if state.options != options {
                state.options = options;
                state.trace = create_electric_trace(options);
                state.last_paint_ms = 0.0;
            }
        });
        window.request_animation_frame();

        let dark = cx.theme().is_dark;
        let (source, actions) = (self.source.clone(), self.actions.clone());
        let (line_width, fade_edges) = (self.line_width, self.fade_edges);
        let (electric, core, heat, glow_color) = (
            self.electric,
            self.electric_core,
            self.electric_heat,
            self.electric_glow,
        );
        let sigma = self.electric_glow_size;
        let paint_intensity = if reduced_motion {
            0.0
        } else {
            options.intensity
        };

        let mut root = div().relative().w_full().h(px(96.0));
        root.style().refine(&self.style);
        root.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let (width, height) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    if width <= 0.0 {
                        return;
                    }
                    let now = now_ms();
                    let geometry = ElectricTraceGeometry {
                        width,
                        height,
                        line_width,
                    };
                    state.update(cx, |state, _| {
                        if let Some(command) = actions.as_ref().and_then(|a| a.take()) {
                            state.frame = command;
                        }
                        if let Some(source) = &source {
                            let version = source.version();
                            let seen = state.source_seen.as_ref();
                            if seen.is_none_or(|(seen, at)| seen != source || *at != version) {
                                state.source_seen = Some((source.clone(), version));
                                if let Some(frame) = source.latest() {
                                    state.frame = Some(frame);
                                }
                            }
                        }
                        let due = !reduced_motion
                            || now - state.last_paint_ms >= REDUCED_MOTION_INTERVAL_MS;
                        if due {
                            state.last_paint_ms = now;
                            state.paint_ms = now;
                            state.trace.step(now, state.frame.as_ref(), geometry);
                        }
                    });

                    let current = window.text_style().color;
                    let colors = resolve_colors(current, electric, core, glow_color, heat, dark);
                    let glow = GlowLayer {
                        color: colors.glow,
                        sigma,
                        opacity: glow_opacity(dark),
                    };
                    let state = state.read(cx);
                    let fade = fade_edges.then(|| EdgeFade {
                        left: bounds.origin.x.as_f32(),
                        right: bounds.origin.x.as_f32() + width,
                        edge: FADE_PX.min(width / 2.0),
                    });
                    let input = PaintInput {
                        trace: &state.trace,
                        geometry,
                        origin: bounds.origin,
                        colors,
                        intensity: paint_intensity,
                        now_ms: state.paint_ms,
                        fade,
                    };
                    paint_glow(window, &input, &glow);
                    window.with_content_mask(Some(ContentMask { bounds }), |window| {
                        paint_main(window, &input);
                    });
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

    const FRAME_MS: f64 = 16.0;
    const GEOMETRY: ElectricTraceGeometry = ElectricTraceGeometry {
        width: 256.0,
        height: 80.0,
        line_width: 3.0,
    };

    fn frame_of(bands: &[f32]) -> VisualFrame {
        VisualFrame {
            bands: bands.to_vec(),
            history: vec![0.0],
            peak_db: -12.0,
            ..Default::default()
        }
    }

    fn options() -> ElectricTraceOptions {
        ElectricTraceOptions {
            arcs: true,
            intensity: 1.0,
            loading: false,
            mode: WaveLineMode::Wave,
            reduced_motion: false,
            seed: Some(5.0),
            sensitivity: 1.0,
            sparks: true,
        }
    }

    fn run(
        trace: &mut ElectricTrace,
        frame_at: impl Fn(usize) -> Option<VisualFrame>,
        frames: usize,
    ) -> bool {
        let mut active = false;
        for frame in 1..=frames {
            active = trace.step(frame as f64 * FRAME_MS, frame_at(frame).as_ref(), GEOMETRY);
        }
        active
    }

    #[test]
    fn electric_trace_crackles_the_same_way_for_the_same_seed() {
        let mut first = create_electric_trace(options());
        let mut second = create_electric_trace(options());
        let mut other = create_electric_trace(ElectricTraceOptions {
            seed: Some(6.0),
            ..options()
        });
        for trace in [&mut first, &mut second, &mut other] {
            run(trace, |_| Some(frame_of(&[0.8, 0.6, 0.4, 0.2])), 10);
        }
        assert_eq!(first.crackle, second.crackle);
        assert_ne!(first.crackle, other.crackle);
    }

    #[test]
    fn electric_trace_keeps_the_flicker_within_12_percent() {
        let mut trace = create_electric_trace(options());
        for frame in 1..=60 {
            trace.step(
                frame as f64 * FRAME_MS,
                Some(&frame_of(&[1.0; 4])),
                GEOMETRY,
            );
            assert!(trace.flicker >= 0.88 && trace.flicker <= 1.0);
        }
    }

    #[test]
    fn electric_trace_forks_only_off_points_far_from_the_middle() {
        let mut trace = create_electric_trace(options());
        let mut forks = 0;
        for frame in 1..=600 {
            let now = frame as f64 * FRAME_MS;
            trace.step(now, Some(&frame_of(&[1.0; 4])), GEOMETRY);
            for branch in &trace.branches {
                if branch.life_ms > 0.0 && branch.born_ms == now {
                    forks += 1;
                    assert!(trace.heights()[branch.point].abs() > 0.25);
                }
            }
        }
        assert!(forks > 0);
    }

    #[test]
    fn electric_trace_throws_sparks_on_a_sudden_rise() {
        let mut trace = create_electric_trace(options());
        run(
            &mut trace,
            |frame| (frame >= 3).then(|| frame_of(&[1.0; 4])),
            3,
        );
        assert!(trace.sparks.life.iter().any(|&life| life > 0.0));
    }

    #[test]
    fn electric_trace_stays_smooth_and_still_with_reduced_motion() {
        let mut trace = create_electric_trace(ElectricTraceOptions {
            reduced_motion: true,
            ..options()
        });
        run(
            &mut trace,
            |frame| {
                Some(if frame % 2 == 0 {
                    frame_of(&[1.0; 4])
                } else {
                    frame_of(&[0.0])
                })
            },
            60,
        );
        assert!(trace.crackle.iter().all(|&offset| offset == 0.0));
        assert!(trace.branches.iter().all(|b| b.life_ms == 0.0));
        assert!(trace.sparks.life.iter().all(|&life| life == 0.0));
    }

    #[test]
    fn electric_trace_marks_an_active_signal_and_falls_flat_when_cleared() {
        let mut trace = create_electric_trace(options());
        assert!(!run(&mut trace, |_| None, 6));
        assert!(run(&mut trace, |_| Some(frame_of(&[1.0; 4])), 6));
        let mut now = 12.0 * FRAME_MS;
        let mut active = true;
        for _ in 0..60 {
            now += FRAME_MS;
            active = trace.step(now, None, GEOMETRY);
        }
        assert!(!active);
    }

    #[test]
    fn electric_trace_draws_a_smooth_line_with_no_intensity() {
        let mut trace = create_electric_trace(options());
        run(&mut trace, |_| None, 6);
        let ys: Vec<f32> = (0..trace.count)
            .map(|p| trace.y_of(&GEOMETRY, 0.0, p, 1.0, 0.6))
            .collect();
        assert!(ys.iter().all(|&y| (y - ys[0]).abs() < 1e-3));
    }

    #[test]
    fn electric_waveform_uses_points_in_whole_chunks() {
        assert_eq!(point_count_for(256.0), 65);
        assert_eq!(point_count_for(10.0), MIN_POINTS);
        assert_eq!(point_count_for(10_000.0), MAX_POINTS);
        assert_eq!((point_count_for(512.0) - 1) % CHUNK, 0);
    }

    #[test]
    fn electric_edge_fade_ramps_the_ends() {
        let fade = EdgeFade {
            left: 10.0,
            right: 110.0,
            edge: 24.0,
        };
        assert_eq!(fade.at(10.0), 0.0);
        assert_eq!(fade.at(22.0), 0.5);
        assert_eq!(fade.at(60.0), 1.0);
        assert_eq!(fade.at(110.0), 0.0);
    }

    #[test]
    fn electric_density_adds_like_stacked_alpha() {
        let stacked = 1.0 - (1.0 - 0.3f32) * (1.0 - 0.4);
        let summed = 1.0 - (-(density(0.3) + density(0.4))).exp();
        assert!((stacked - summed).abs() < 1e-5);
    }
}
