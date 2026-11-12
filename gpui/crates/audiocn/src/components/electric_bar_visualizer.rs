//! Port of `components/ui/electric-bar-visualizer.tsx`.
//!
//! Frequency-band bars drawn as crackling filaments, with arcs between loud
//! neighbours and sparks on sudden rises.
//!
//! **What changed for GPUI.** As in [`ElectricWaveform`](super::ElectricWaveform),
//! the sharp canvas becomes `PathBuilder` strokes and the CSS-blurred glow
//! canvas becomes Gaussian-blurred boxes (`paint_drop_shadows`): each glow
//! filament is a blurred round-ended bar (it only follows 35% of the crackle,
//! which the blur hides), each glowing tip a blurred disc, and each arc a chain
//! of blurred pieces. The moving parts ([`ElectricScene`]) and the layout
//! ([`layout_electric_bars`]) are straight ports and are unit-tested.

use std::cell::RefCell;
use std::f32::consts::TAU;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{
    App, ContentMask, ElementId, Hsla, IntoElement, ParentElement, Pixels, Point, Refineable,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, canvas, div, point, px,
};

use super::electric_waveform::{
    EdgeFade, GlowLayer, REDUCED_MOTION_INTERVAL_MS, StrokeStyle, fill_polygon, glow_box,
    glow_opacity, glow_polyline, paint_sparks, paint_stroke, resolve_colors,
};
use crate::audio::bar_levels::{BarIdle, BarLevels, BarLevelsOptions, create_bar_levels};
use crate::audio::decibels::clamp;
use crate::audio::electric::{
    ELECTRIC_GLOW_SIZE, ElectricColors, ElectricSparks, SeededRandom, SparkLaunch,
    create_electric_sparks, create_random, displace, emit_spark, move_sparks,
};
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::types::{Orientation, VisualFrame};
use crate::config::AudioConfig;
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::ActiveTheme;

const DEFAULT_BAR_COUNT: usize = 16;
const DEFAULT_MIN_LEVEL: f32 = 0.08;
const DEFAULT_INTENSITY: f32 = 0.6;
const DEFAULT_BAR_WIDTH: f32 = 6.0;
const DEFAULT_BAR_GAP: f32 = 4.0;
const MS_PER_SECOND: f64 = 1000.0;
const FRAME_SECONDS: f64 = 1.0 / 60.0;
const MAX_STEP_SECONDS: f64 = 0.05;

/// A filament has 2^4 segments, so every split halves one cleanly.
const FILAMENT_DEPTH: u32 = 4;
/// Points along a filament.
pub const FILAMENT_POINTS: usize = (1 << FILAMENT_DEPTH) + 1;
/// Shortest segment in pixels before a filament drops to fewer points.
const SEGMENT_PX: f32 = 5.0;
/// How much each finer split bends compared with the one before.
const ROUGHNESS: f32 = 0.72;
/// How far a free tip wanders, relative to the jitter amplitude.
const FREE_TIP: f64 = 0.5;
/// Each jagged shape holds at least this long, so the crackle never strobes.
const JITTER_MS: f64 = 42.0;
/// Largest bend, relative to the filament's length.
const MAX_BEND: f32 = 0.35;
/// Brightness flicker, at most 12%.
const FLICKER: f64 = 0.12;
/// Tips start to glow above this level.
const TIP_LEVEL: f32 = 0.25;

const MAX_ARCS: usize = 3;
/// Arcs jump to the next bar or the one after.
const MAX_ARC_REACH: usize = 2;
const ARC_DEPTH: u32 = 3;
/// Points along an arc.
pub const ARC_POINTS: usize = (1 << ARC_DEPTH) + 1;
const ARC_ATTEMPTS: usize = 2;
/// Both neighbours must be this loud for an arc to jump between them.
const ARC_THRESHOLD: f32 = 0.45;
const ARC_CHANCE: f64 = 0.35;
const ARC_MIN_MS: f64 = 70.0;
const ARC_MAX_MS: f64 = 150.0;
/// How far an arc bends, relative to the distance it spans.
const ARC_BOW: f32 = 0.3;

const MAX_SPARKS: usize = 64;
/// A rise this big in one frame throws sparks.
const SPARK_RISE: f32 = 0.12;
const SPARK_MIN_LEVEL: f32 = 0.3;
const SPARK_COOLDOWN_MS: f64 = 90.0;
/// Pixels per second squared.
const SPARK_GRAVITY: f32 = 520.0;
/// Pixels per second.
const SPARK_SPEED: f64 = 180.0;
/// Pixels per second, sideways.
const SPARK_SPREAD: f64 = 70.0;
const SPARK_MIN_MS: f64 = 260.0;
const SPARK_MAX_MS: f64 = 560.0;

/// Where the bars grow from (`align`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ElectricBarAlign {
    #[default]
    Center,
    Start,
    End,
}

/// Which end of a bar: `To` is the tip, `From` the base (the far tip when centred).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ElectricTip {
    From,
    #[default]
    To,
}

/// Where the bars sit, in logical pixels. `across` runs along the row of bars
/// and `along` runs the way they grow, so both orientations share one drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricLayout {
    pub horizontal: bool,
    pub align: ElectricBarAlign,
    pub bar_width: f32,
    pub bar_gap: f32,
    /// Centre of the first bar, across.
    pub first: f32,
    /// Distance between bar centres.
    pub pitch: f32,
    /// Where bars grow from, along: the base, or the middle when centred.
    pub base: f32,
    /// Length of a bar at level 1.
    pub span: f32,
}

/// Options for [`layout_electric_bars`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricLayoutOptions {
    pub align: ElectricBarAlign,
    pub bar_count: usize,
    pub bar_gap: f32,
    pub bar_width: f32,
    pub orientation: Orientation,
}

/// Fits the bars into a box, narrowing bars and gaps when space is short.
pub fn layout_electric_bars(
    width: f32,
    height: f32,
    options: ElectricLayoutOptions,
) -> ElectricLayout {
    let ElectricLayoutOptions {
        align,
        bar_count,
        bar_gap,
        bar_width,
        orientation,
    } = options;
    let horizontal = orientation == Orientation::Horizontal;
    let count = bar_count.max(1) as f32;
    let across_length = if horizontal { width } else { height };
    let along_length = if horizontal { height } else { width };
    let pitch = (bar_width + bar_gap).min((across_length - bar_width * 2.0).max(0.0) / count);
    let unit = bar_width + bar_gap;
    let scale = pitch / if unit == 0.0 { 1.0 } else { unit };
    let resolved_width = bar_width * scale;
    let resolved_gap = bar_gap * scale;
    let inset = resolved_width.min(along_length / 4.0);
    let base = match align {
        ElectricBarAlign::Center => along_length / 2.0,
        ElectricBarAlign::End => along_length - inset,
        ElectricBarAlign::Start => inset,
    };
    ElectricLayout {
        align,
        bar_gap: resolved_gap,
        bar_width: resolved_width,
        base,
        first: (across_length - count * pitch + resolved_gap + resolved_width) / 2.0,
        horizontal,
        pitch,
        span: (along_length - inset * 2.0).max(0.0),
    }
}

fn bar_from(layout: &ElectricLayout, level: f32) -> f32 {
    if layout.align == ElectricBarAlign::Center {
        layout.base - (level * layout.span) / 2.0
    } else {
        layout.base
    }
}

fn bar_to(layout: &ElectricLayout, level: f32) -> f32 {
    if layout.align == ElectricBarAlign::Center {
        return layout.base + (level * layout.span) / 2.0;
    }
    let direction = if layout.align == ElectricBarAlign::End {
        -1.0
    } else {
        1.0
    };
    layout.base + direction * level * layout.span
}

fn bar_end(layout: &ElectricLayout, level: f32, tip: ElectricTip) -> f32 {
    match tip {
        ElectricTip::To => bar_to(layout, level),
        ElectricTip::From => bar_from(layout, level),
    }
}

/// Which way a tip points, along: sparks fly that way.
fn tip_direction(layout: &ElectricLayout, tip: ElectricTip) -> f32 {
    let growth = if layout.align == ElectricBarAlign::End {
        -1.0
    } else {
        1.0
    };
    if tip == ElectricTip::To {
        growth
    } else {
        -growth
    }
}

/// How far a filament wanders sideways, in pixels. Louder bars crackle more,
/// and short ones never bend more than a third of their length.
fn jitter_amplitude(layout: &ElectricLayout, intensity: f32, level: f32) -> f32 {
    (intensity * (layout.bar_width + layout.bar_gap) * 0.6 * (0.3 + 0.7 * level))
        .min(level * layout.span * MAX_BEND)
}

/// An arc of lightning between two bars.
#[derive(Clone, Debug, PartialEq)]
pub struct ElectricArc {
    /// The arc joins this bar and the one `reach` bars on.
    pub index: usize,
    pub reach: usize,
    /// Which end of the bars it joins: `To` is the tip, `From` the base (centred bars only).
    pub tip: ElectricTip,
    pub born_ms: f64,
    /// 0 when the arc is gone.
    pub life_ms: f64,
    /// Bend at each point, in units of the arc's bow.
    pub jitter: [f32; ARC_POINTS],
}

/// Options for [`create_electric_scene`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricSceneOptions {
    pub bar_count: usize,
    pub intensity: f32,
    pub arcs: bool,
    pub sparks: bool,
    pub loading: bool,
    pub reduced_motion: bool,
    /// Makes the crackle repeatable. Each scene gets its own by default.
    pub seed: Option<f64>,
}

fn loudest_index(levels: &[f32]) -> usize {
    let mut loudest = 0;
    for (index, &level) in levels.iter().enumerate() {
        if level > levels.get(loudest).copied().unwrap_or(0.0) {
            loudest = index;
        }
    }
    loudest
}

static SCENE_COUNT: AtomicU64 = AtomicU64::new(0);

/// The moving parts of the electric look, apart from any drawing: jagged
/// filaments, arcs between loud neighbours and sparks thrown off sudden rises.
/// With reduced motion it stays straight and still.
///
/// ```ignore
/// let mut scene = create_electric_scene(options);
/// scene.step(now_ms(), bar_levels.levels(), &layout);
/// ```
#[derive(Clone, Debug)]
pub struct ElectricScene {
    /// Sideways offset of each filament point, per bar, in units of its jitter amplitude.
    pub jitter: Vec<f32>,
    /// Brightness of each bar, 0.88..1.
    pub flicker: Vec<f32>,
    pub arcs: Vec<ElectricArc>,
    pub sparks: ElectricSparks,
    options: ElectricSceneOptions,
    random: SeededRandom,
    previous: Vec<f32>,
    last_emit_ms: Vec<f64>,
    last_ms: f64,
    last_jitter_ms: f64,
    primed: bool,
}

/// Creates an [`ElectricScene`] (`createElectricScene`).
pub fn create_electric_scene(options: ElectricSceneOptions) -> ElectricScene {
    let count = SCENE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let bars = options.bar_count;
    ElectricScene {
        jitter: vec![0.0; bars * FILAMENT_POINTS],
        flicker: vec![1.0; bars],
        arcs: (0..MAX_ARCS)
            .map(|_| ElectricArc {
                index: 0,
                reach: 1,
                tip: ElectricTip::To,
                born_ms: 0.0,
                life_ms: 0.0,
                jitter: [0.0; ARC_POINTS],
            })
            .collect(),
        sparks: create_electric_sparks(MAX_SPARKS),
        options,
        random: create_random(options.seed.unwrap_or(count as f64)),
        previous: vec![0.0; bars],
        last_emit_ms: vec![f64::NEG_INFINITY; bars],
        last_ms: 0.0,
        last_jitter_ms: f64::NEG_INFINITY,
        primed: false,
    }
}

impl ElectricScene {
    /// The options this scene was created with.
    pub fn options(&self) -> ElectricSceneOptions {
        self.options
    }

    fn signed(&mut self) -> f64 {
        self.random.next() * 2.0 - 1.0
    }

    fn reroll_filaments(&mut self, centered: bool) {
        for index in 0..self.options.bar_count {
            let from = if centered {
                (self.signed() * FREE_TIP) as f32
            } else {
                0.0
            };
            let to = (self.signed() * FREE_TIP) as f32;
            let random = &mut self.random;
            displace(
                &mut self.jitter,
                index * FILAMENT_POINTS,
                FILAMENT_POINTS,
                [from, to],
                || random.next(),
                ROUGHNESS,
            );
            self.flicker[index] = (1.0 - FLICKER * self.random.next()) as f32;
        }
    }

    fn start_arc(&mut self, arc: usize, index: usize, reach: usize, tip: ElectricTip, now_ms: f64) {
        let life = ARC_MIN_MS + self.random.next() * (ARC_MAX_MS - ARC_MIN_MS);
        let arc = &mut self.arcs[arc];
        arc.index = index;
        arc.reach = reach;
        arc.tip = tip;
        arc.born_ms = now_ms;
        arc.life_ms = life;
    }

    /// While loading, one arc rides the sweep across the bars.
    fn follow_sweep(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        let peak = loudest_index(levels);
        let before = if peak == 0 { 0.0 } else { levels[peak - 1] };
        let after = levels.get(peak + 1).copied().unwrap_or(0.0);
        let index = if before > after {
            peak as f64 - 1.0
        } else {
            peak as f64
        };
        let index = index.clamp(0.0, self.options.bar_count as f64 - 2.0) as usize;
        let tip = if centered {
            ElectricTip::From
        } else {
            ElectricTip::To
        };
        self.start_arc(0, index, 1, tip, now_ms);
        self.arcs[0].life_ms = JITTER_MS * 2.0;
    }

    fn spawn_arcs(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        let bars = self.options.bar_count;
        let intensity = self.options.intensity as f64;
        for _ in 0..ARC_ATTEMPTS {
            let reach = 1 + (self.random.next() * MAX_ARC_REACH as f64).floor() as usize;
            let index = (self.random.next() * bars.saturating_sub(reach) as f64).floor() as usize;
            let strength = levels
                .get(index)
                .copied()
                .unwrap_or(0.0)
                .min(levels.get(index + reach).copied().unwrap_or(0.0));
            let charged = strength >= ARC_THRESHOLD
                && self.random.next() < ARC_CHANCE * intensity * strength as f64;
            let free = self.arcs.iter().position(|arc| arc.life_ms == 0.0);
            let taken = self
                .arcs
                .iter()
                .any(|arc| arc.life_ms > 0.0 && arc.index == index);
            if charged
                && !taken
                && let Some(free) = free
            {
                let tip = if centered && self.random.next() < 0.5 {
                    ElectricTip::From
                } else {
                    ElectricTip::To
                };
                self.start_arc(free, index, reach, tip, now_ms);
            }
        }
    }

    fn update_arcs(&mut self, now_ms: f64, levels: &[f32], centered: bool) {
        if self.options.bar_count < 2 {
            return;
        }
        if self.options.loading {
            self.follow_sweep(now_ms, levels, centered);
        } else {
            self.spawn_arcs(now_ms, levels, centered);
        }
        for arc in 0..self.arcs.len() {
            if self.arcs[arc].life_ms > 0.0 {
                let random = &mut self.random;
                displace(
                    &mut self.arcs[arc].jitter,
                    0,
                    ARC_POINTS,
                    [0.0, 0.0],
                    || random.next(),
                    ROUGHNESS,
                );
            }
        }
    }

    fn expire_arcs(&mut self, now_ms: f64) {
        for arc in &mut self.arcs {
            if arc.life_ms > 0.0 && now_ms - arc.born_ms >= arc.life_ms {
                arc.life_ms = 0.0;
            }
        }
    }

    fn launch_spark(&mut self, index: usize, level: f32, layout: &ElectricLayout) {
        let tip = if layout.align == ElectricBarAlign::Center && self.random.next() < 0.5 {
            ElectricTip::From
        } else {
            ElectricTip::To
        };
        let along = bar_end(layout, level, tip);
        let across = layout.first + index as f32 * layout.pitch;
        let speed =
            SPARK_SPEED * (0.5 + self.random.next()) * (0.5 + self.options.intensity as f64);
        let along_velocity = tip_direction(layout, tip) * speed as f32;
        let across_velocity = (self.signed() * SPARK_SPREAD) as f32;
        let life_ms = (SPARK_MIN_MS + self.random.next() * (SPARK_MAX_MS - SPARK_MIN_MS)) as f32;
        let (x, y, vx, vy) = if layout.horizontal {
            (across, along, across_velocity, along_velocity)
        } else {
            (along, across, along_velocity, across_velocity)
        };
        emit_spark(
            &mut self.sparks,
            SparkLaunch {
                x,
                y,
                vx,
                vy,
                life_ms,
            },
        );
    }

    fn emit_sparks(&mut self, now_ms: f64, levels: &[f32], layout: &ElectricLayout) {
        for (index, &level) in levels.iter().enumerate().take(self.options.bar_count) {
            let rise = level - self.previous[index];
            let rested = now_ms - self.last_emit_ms[index] >= SPARK_COOLDOWN_MS;
            if rise >= SPARK_RISE && level >= SPARK_MIN_LEVEL && rested {
                self.last_emit_ms[index] = now_ms;
                let count = 2 + (self.random.next() * 3.0).floor() as usize;
                for _ in 0..count {
                    self.launch_spark(index, level, layout);
                }
            }
        }
    }

    /// Advances the crackle, arcs and sparks to `now_ms`.
    pub fn step(&mut self, now_ms: f64, levels: &[f32], layout: &ElectricLayout) {
        let seconds = if self.last_ms == 0.0 {
            FRAME_SECONDS
        } else {
            ((now_ms - self.last_ms) / MS_PER_SECOND).clamp(0.0, MAX_STEP_SECONDS)
        };
        self.last_ms = now_ms;
        if self.options.reduced_motion {
            return;
        }
        let centered = layout.align == ElectricBarAlign::Center;
        if now_ms - self.last_jitter_ms >= JITTER_MS {
            self.last_jitter_ms = now_ms;
            self.reroll_filaments(centered);
            if self.options.arcs {
                self.update_arcs(now_ms, levels, centered);
            }
        }
        self.expire_arcs(now_ms);
        if self.options.sparks {
            if self.primed {
                self.emit_sparks(now_ms, levels, layout);
            }
            move_sparks(&mut self.sparks, seconds as f32, SPARK_GRAVITY);
        }
        let n = self.previous.len().min(levels.len());
        self.previous[..n].copy_from_slice(&levels[..n]);
        self.primed = true;
    }
}

// ---------------------------------------------------------------------------
// Painting.
// ---------------------------------------------------------------------------

struct PaintInput<'a> {
    scene: &'a ElectricScene,
    layout: ElectricLayout,
    levels: &'a [f32],
    origin: Point<Pixels>,
    colors: ElectricColors,
    intensity: f32,
    now_ms: f64,
}

impl PaintInput<'_> {
    /// A point in canvas space, from `across` / `along`.
    fn plot(&self, across: f32, along: f32) -> Point<Pixels> {
        let (x, y) = if self.layout.horizontal {
            (across, along)
        } else {
            (along, across)
        };
        self.origin + point(px(x), px(y))
    }

    fn flicker(&self, index: usize) -> f32 {
        self.scene.flicker.get(index).copied().unwrap_or(1.0)
    }

    fn jitter(&self, index: usize) -> f32 {
        self.scene.jitter.get(index).copied().unwrap_or(0.0)
    }

    fn level(&self, index: usize) -> f32 {
        self.levels.get(index).copied().unwrap_or(0.0)
    }
}

/// Short filaments use fewer points, so they stay a line and not a scribble.
fn stride_for(length: f32) -> usize {
    let depth = ((length.max(1.0) / SEGMENT_PX).log2().ceil()).clamp(1.0, FILAMENT_DEPTH as f32);
    1 << (FILAMENT_DEPTH - depth as u32)
}

/// How one pass draws the filaments.
struct FilamentPass {
    /// Line width, relative to the bar width.
    width: f32,
    /// Least line width in pixels.
    min_width: f32,
    /// Share of the jitter the pass follows: the sheath stays close to a bar while the core crackles.
    bend: f32,
    stroke: StrokeStyle,
    alpha: fn(f32) -> f32,
}

const GLOW_PASS: FilamentPass = FilamentPass {
    alpha: |level| 0.1 + 0.35 * level,
    bend: 0.35,
    stroke: StrokeStyle::round(),
    min_width: 2.0,
    width: 1.6,
};

const SHEATH_PASS: FilamentPass = FilamentPass {
    alpha: |level| 0.35 + 0.35 * level,
    bend: 0.35,
    stroke: StrokeStyle::round(),
    min_width: 1.0,
    width: 1.0,
};

const CORE_PASS: FilamentPass = FilamentPass {
    alpha: |level| 0.8 + 0.2 * level,
    bend: 1.0,
    stroke: StrokeStyle::miter(),
    min_width: 1.25,
    width: 0.3,
};

fn trace_filament(
    input: &PaintInput,
    index: usize,
    level: f32,
    bend_share: f32,
) -> Vec<Point<Pixels>> {
    let layout = &input.layout;
    let across = layout.first + index as f32 * layout.pitch;
    let amplitude = jitter_amplitude(layout, input.intensity, level) * bend_share;
    let from = bar_from(layout, level);
    let to = bar_to(layout, level);
    let stride = stride_for((to - from).abs());
    let offset = index * FILAMENT_POINTS;
    (0..FILAMENT_POINTS)
        .step_by(stride)
        .map(|p| {
            let along = from + ((to - from) * p as f32) / (FILAMENT_POINTS - 1) as f32;
            let bend = input.jitter(offset + p) * amplitude;
            input.plot(across + bend, along)
        })
        .collect()
}

fn stroke_filaments(window: &mut Window, input: &PaintInput, pass: &FilamentPass, color: Hsla) {
    let width = (input.layout.bar_width * pass.width).max(pass.min_width);
    for (index, &level) in input.levels.iter().enumerate() {
        let alpha = clamp((pass.alpha)(level) * input.flicker(index), 0.0, 1.0);
        let points = trace_filament(input, index, level, pass.bend);
        paint_stroke(
            window,
            &points,
            width,
            pass.stroke,
            color.opacity(alpha),
            None,
        );
    }
}

/// The glow pass: each filament as a blurred, round-ended bar along its mean bend.
fn glow_filaments(window: &mut Window, input: &PaintInput, glow: &GlowLayer) {
    let layout = &input.layout;
    let pass = &GLOW_PASS;
    let width = (layout.bar_width * pass.width).max(pass.min_width);
    for (index, &level) in input.levels.iter().enumerate() {
        let alpha = clamp((pass.alpha)(level) * input.flicker(index), 0.0, 1.0);
        let points = trace_filament(input, index, level, pass.bend);
        let n = points.len().max(1) as f32;
        let mean_x = points.iter().map(|p| p.x.as_f32()).sum::<f32>() / n;
        let mean_y = points.iter().map(|p| p.y.as_f32()).sum::<f32>() / n;
        let length = (bar_to(layout, level) - bar_from(layout, level)).abs();
        let (w, h) = if layout.horizontal {
            (width, length + width)
        } else {
            (length + width, width)
        };
        glow_box(
            window,
            point(mean_x - w / 2.0, mean_y - h / 2.0),
            point(w, h),
            width / 2.0,
            glow,
            alpha,
        );
    }
}

/// Where a bar's tip is, jitter included, as `(across, along)`.
fn tip_of(input: &PaintInput, index: usize, level: f32, tip: ElectricTip) -> (f32, f32) {
    let layout = &input.layout;
    let at = if tip == ElectricTip::To {
        FILAMENT_POINTS - 1
    } else {
        0
    };
    let bend = input.jitter(index * FILAMENT_POINTS + at)
        * jitter_amplitude(layout, input.intensity, level);
    (
        layout.first + index as f32 * layout.pitch + bend,
        bar_end(layout, level, tip),
    )
}

fn tips_for(layout: &ElectricLayout) -> &'static [ElectricTip] {
    if layout.align == ElectricBarAlign::Center {
        &[ElectricTip::From, ElectricTip::To]
    } else {
        &[ElectricTip::To]
    }
}

fn tip_heat(input: &PaintInput, index: usize, level: f32) -> f32 {
    let heat = (level - TIP_LEVEL) / (1.0 - TIP_LEVEL);
    if heat > 0.0 {
        clamp(heat, 0.0, 1.0) * input.flicker(index)
    } else {
        0.0
    }
}

fn fill_tips(window: &mut Window, input: &PaintInput, radius: f32, color: Hsla) {
    const STEPS: usize = 16;
    for (index, &level) in input.levels.iter().enumerate() {
        let alpha = tip_heat(input, index, level);
        if alpha <= 0.0 {
            continue;
        }
        for &tip in tips_for(&input.layout) {
            let (across, along) = tip_of(input, index, level, tip);
            let center = input.plot(across, along);
            let disc: Vec<Point<Pixels>> = (0..STEPS)
                .map(|step| {
                    let angle = TAU * step as f32 / STEPS as f32;
                    center + point(px(radius * angle.cos()), px(radius * angle.sin()))
                })
                .collect();
            if let Some(path) = fill_polygon(&disc) {
                window.paint_path(path, color.opacity(alpha));
            }
        }
    }
}

fn glow_tips(window: &mut Window, input: &PaintInput, radius: f32, glow: &GlowLayer) {
    for (index, &level) in input.levels.iter().enumerate() {
        let alpha = tip_heat(input, index, level);
        if alpha <= 0.0 {
            continue;
        }
        for &tip in tips_for(&input.layout) {
            let (across, along) = tip_of(input, index, level, tip);
            let center = input.plot(across, along);
            glow_box(
                window,
                point(center.x.as_f32() - radius, center.y.as_f32() - radius),
                point(radius * 2.0, radius * 2.0),
                radius,
                glow,
                alpha,
            );
        }
    }
}

fn trace_arc(input: &PaintInput, arc: &ElectricArc) -> Vec<Point<Pixels>> {
    let (from_across, from_along) = tip_of(input, arc.index, input.level(arc.index), arc.tip);
    let to_index = arc.index + arc.reach;
    let (to_across, to_along) = tip_of(input, to_index, input.level(to_index), arc.tip);
    let across_span = to_across - from_across;
    let along_span = to_along - from_along;
    let length = match across_span.hypot(along_span) {
        0.0 => 1.0,
        length => length,
    };
    let bow = length * ARC_BOW;
    (0..ARC_POINTS)
        .map(|p| {
            let progress = p as f32 / (ARC_POINTS - 1) as f32;
            let bend = arc.jitter[p] * bow;
            input.plot(
                from_across + across_span * progress - (along_span / length) * bend,
                from_along + along_span * progress + (across_span / length) * bend,
            )
        })
        .collect()
}

fn arc_alpha(input: &PaintInput, arc: &ElectricArc, alpha: f32) -> f32 {
    let age = ((input.now_ms - arc.born_ms) / arc.life_ms) as f32;
    clamp(alpha * (1.0 - age * age), 0.0, 1.0)
}

fn stroke_arcs(
    window: &mut Window,
    input: &PaintInput,
    width: f32,
    alpha: f32,
    color: Hsla,
    style: StrokeStyle,
) {
    for arc in input.scene.arcs.iter().filter(|arc| arc.life_ms > 0.0) {
        let points = trace_arc(input, arc);
        let alpha = arc_alpha(input, arc, alpha);
        paint_stroke(window, &points, width, style, color.opacity(alpha), None);
    }
}

/// Wide, soft strokes, blurred.
fn paint_glow(window: &mut Window, input: &PaintInput, glow: &GlowLayer) {
    let bar_width = input.layout.bar_width;
    glow_filaments(window, input, glow);
    glow_tips(window, input, bar_width * 1.2, glow);
    for arc in input.scene.arcs.iter().filter(|arc| arc.life_ms > 0.0) {
        let points = trace_arc(input, arc);
        let alpha = arc_alpha(input, arc, 0.7);
        glow_polyline(window, &points, bar_width, alpha, glow, &|_| 1.0);
    }
}

fn paint_main(window: &mut Window, input: &PaintInput) {
    let bar_width = input.layout.bar_width;
    let ElectricColors { body, core, .. } = input.colors;
    stroke_filaments(window, input, &SHEATH_PASS, body);
    stroke_arcs(
        window,
        input,
        (bar_width * 0.45).max(1.5),
        0.6,
        body,
        StrokeStyle::miter(),
    );
    stroke_filaments(window, input, &CORE_PASS, core);
    stroke_arcs(window, input, 1.0, 1.0, core, StrokeStyle::miter());
    fill_tips(window, input, (bar_width * 0.35).max(1.0), core);
    paint_sparks(
        window,
        input.origin,
        &input.scene.sparks,
        core,
        (bar_width * 0.25).max(1.0),
        None::<&EdgeFade>,
    );
}

// ---------------------------------------------------------------------------
// The component.
// ---------------------------------------------------------------------------

/// Paints levels into an [`ElectricBarVisualizer`] directly (`actionsRef`).
///
/// ```ignore
/// let actions = ElectricBarVisualizerActions::new();
/// ElectricBarVisualizer::new("bars").actions(actions.clone());
/// actions.paint(&[0.2, 0.8, 0.5]); // resampled to the bar count
/// ```
#[derive(Clone, Default)]
pub struct ElectricBarVisualizerActions {
    pending: Rc<RefCell<Option<Vec<f32>>>>,
}

impl ElectricBarVisualizerActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paint levels (0..1) directly. They are resampled to the bar count.
    pub fn paint(&self, levels: &[f32]) {
        *self.pending.borrow_mut() = Some(levels.to_vec());
    }

    fn take(&self) -> Option<Vec<f32>> {
        self.pending.borrow_mut().take()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneKey {
    align: ElectricBarAlign,
    arcs: bool,
    bar_count: usize,
    bar_gap: f32,
    bar_width: f32,
    idle: BarIdle,
    intensity: f32,
    loading: bool,
    min_level: f32,
    mirrored: bool,
    orientation: Orientation,
    reduced_motion: bool,
    sparks: bool,
}

impl SceneKey {
    fn build(&self) -> (BarLevels, ElectricScene) {
        (
            create_bar_levels(BarLevelsOptions {
                bar_count: self.bar_count,
                idle: self.idle,
                loading: self.loading,
                min_level: self.min_level,
                mirrored: self.mirrored,
                reduced_motion: self.reduced_motion,
            }),
            create_electric_scene(ElectricSceneOptions {
                arcs: self.arcs,
                bar_count: self.bar_count,
                intensity: self.intensity,
                loading: self.loading,
                reduced_motion: self.reduced_motion,
                seed: None,
                sparks: self.sparks,
            }),
        )
    }
}

struct ElectricBarVisualizerState {
    key: SceneKey,
    bar_levels: BarLevels,
    scene: ElectricScene,
    input: Option<Vec<f32>>,
    /// The `levels` prop last applied, so it only takes over when it changes.
    last_levels: Option<Vec<f32>>,
    /// The source last read from, and its version then.
    source_seen: Option<(FrameSource<VisualFrame>, u64)>,
    last_paint_ms: f64,
    paint_ms: f64,
}

/// Frequency-band bars drawn as crackling filaments, with arcs between loud
/// neighbours and sparks on sudden rises. It follows the text colour.
///
/// ```ignore
/// ElectricBarVisualizer::new("voice")
///     .text_color(cx.theme().primary)
///     .h(px(112.))
///     .source(signal.visual.clone())
/// ```
#[derive(IntoElement)]
pub struct ElectricBarVisualizer {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    levels: Option<Vec<f32>>,
    bar_count: usize,
    align: ElectricBarAlign,
    mirrored: bool,
    min_level: f32,
    idle: BarIdle,
    loading: bool,
    orientation: Option<Orientation>,
    intensity: f32,
    arcs: bool,
    sparks: bool,
    bar_width: f32,
    bar_gap: f32,
    actions: Option<ElectricBarVisualizerActions>,
    config: AudioConfig,
    electric: Option<Hsla>,
    electric_core: Option<Hsla>,
    electric_heat: Option<f32>,
    electric_glow: Option<Hsla>,
    electric_glow_size: f32,
    aria_label: SharedString,
    style: StyleRefinement,
}

impl ElectricBarVisualizer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            levels: None,
            bar_count: DEFAULT_BAR_COUNT,
            align: ElectricBarAlign::Center,
            mirrored: false,
            min_level: DEFAULT_MIN_LEVEL,
            idle: BarIdle::Static,
            loading: false,
            orientation: None,
            intensity: DEFAULT_INTENSITY,
            arcs: true,
            sparks: true,
            bar_width: DEFAULT_BAR_WIDTH,
            bar_gap: DEFAULT_BAR_GAP,
            actions: None,
            config: AudioConfig::default(),
            electric: None,
            electric_core: None,
            electric_heat: None,
            electric_glow: None,
            electric_glow_size: ELECTRIC_GLOW_SIZE,
            aria_label: "Audio visualizer".into(),
            style: StyleRefinement::default(),
        }
    }

    /// A visual source; the bars follow its frequency bands.
    pub fn source(mut self, source: impl Into<Option<FrameSource<VisualFrame>>>) -> Self {
        self.source = source.into();
        self
    }

    /// Levels, 0..1, for declarative use.
    pub fn levels(mut self, levels: impl Into<Vec<f32>>) -> Self {
        self.levels = Some(levels.into());
        self
    }

    /// Number of bars. Bands are resampled to fit. Default 16.
    pub fn bar_count(mut self, bar_count: usize) -> Self {
        self.bar_count = bar_count;
        self
    }

    /// Where bars grow from. Default `Center`.
    pub fn align(mut self, align: ElectricBarAlign) -> Self {
        self.align = align;
        self
    }

    /// Symmetric around the middle bar. Default false.
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

    /// Runs an arc along the bars, for connecting or thinking states. Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Default from the config, else horizontal.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// How jagged and restless the filaments are, 0..1. Default 0.6.
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    /// Arcs jump between loud neighbouring bars. Default true.
    pub fn arcs(mut self, arcs: bool) -> Self {
        self.arcs = arcs;
        self
    }

    /// Sparks fly off the tips on sudden rises. Default true.
    pub fn sparks(mut self, sparks: bool) -> Self {
        self.sparks = sparks;
        self
    }

    /// Widest bar in pixels. Default 6.
    pub fn bar_width(mut self, bar_width: f32) -> Self {
        self.bar_width = bar_width;
        self
    }

    /// Gap between bars in pixels. Default 4.
    pub fn bar_gap(mut self, bar_gap: f32) -> Self {
        self.bar_gap = bar_gap;
        self
    }

    /// `actionsRef`: paint levels directly.
    pub fn actions(mut self, actions: ElectricBarVisualizerActions) -> Self {
        self.actions = Some(actions);
        self
    }

    /// The container's [`AudioConfig`] (`useAudioConfig`): its orientation.
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    /// `--electric`: bar colour. Default the text colour.
    pub fn electric(mut self, color: impl Into<Hsla>) -> Self {
        self.electric = Some(color.into());
        self
    }

    /// `--electric-core`: core colour. Default `--electric` mixed toward white by the heat.
    pub fn electric_core(mut self, color: impl Into<Hsla>) -> Self {
        self.electric_core = Some(color.into());
        self
    }

    /// `--electric-heat`, 0..1. Default 0 in light mode, 0.7 in dark.
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

    /// `aria-label`. Default "Audio visualizer".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }
}

impl Styled for ElectricBarVisualizer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ElectricBarVisualizer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let orientation = self
            .orientation
            .or(self.config.orientation)
            .unwrap_or(Orientation::Horizontal);
        let reduced_motion = prefers_reduced_motion(cx);
        let key = SceneKey {
            align: self.align,
            arcs: self.arcs,
            bar_count: self.bar_count,
            bar_gap: self.bar_gap,
            bar_width: self.bar_width,
            idle: self.idle,
            intensity: clamp(self.intensity, 0.0, 1.0),
            loading: self.loading,
            min_level: self.min_level,
            mirrored: self.mirrored,
            orientation,
            reduced_motion,
            sparks: self.sparks,
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            let (bar_levels, scene) = key.build();
            ElectricBarVisualizerState {
                key,
                bar_levels,
                scene,
                input: None,
                last_levels: None,
                source_seen: None,
                last_paint_ms: 0.0,
                paint_ms: 0.0,
            }
        });
        let levels = self.levels.clone();
        state.update(cx, |state, _| {
            if state.key != key {
                (state.bar_levels, state.scene) = key.build();
                state.key = key;
                state.last_paint_ms = 0.0;
            }
            // Clear only when levels go from set to unset, so the bars fall
            // instead of freezing, without wiping levels painted through the handle.
            match levels {
                Some(levels) if state.last_levels.as_ref() != Some(&levels) => {
                    state.input = Some(levels.clone());
                    state.last_levels = Some(levels);
                }
                Some(_) => {}
                None if state.last_levels.is_some() => {
                    state.input = None;
                    state.last_levels = None;
                }
                None => {}
            }
        });
        window.request_animation_frame();

        let dark = cx.theme().is_dark;
        let (source, actions) = (self.source.clone(), self.actions.clone());
        let (electric, core, heat, glow_color) = (
            self.electric,
            self.electric_core,
            self.electric_heat,
            self.electric_glow,
        );
        let sigma = self.electric_glow_size;
        let paint_intensity = if reduced_motion { 0.0 } else { key.intensity };
        let layout_options = ElectricLayoutOptions {
            align: key.align,
            bar_count: key.bar_count,
            bar_gap: key.bar_gap,
            bar_width: key.bar_width,
            orientation,
        };

        let mut root = div().relative();
        root = if orientation == Orientation::Horizontal {
            root.h(px(64.0)).w_full()
        } else {
            root.h_full().w(px(64.0))
        };
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
                    let layout = layout_electric_bars(width, height, layout_options);
                    state.update(cx, |state, _| {
                        if let Some(levels) = actions.as_ref().and_then(|a| a.take()) {
                            state.input = Some(levels);
                        }
                        if let Some(source) = &source {
                            let version = source.version();
                            let seen = state.source_seen.as_ref();
                            if seen.is_none_or(|(seen, at)| seen != source || *at != version) {
                                state.source_seen = Some((source.clone(), version));
                                if let Some(bands) =
                                    source.with_latest(|f| f.map(|f| f.bands.clone()))
                                {
                                    state.input = Some(bands);
                                }
                            }
                        }
                        let due = !reduced_motion
                            || now - state.last_paint_ms >= REDUCED_MOTION_INTERVAL_MS;
                        if due {
                            state.last_paint_ms = now;
                            state.paint_ms = now;
                            state.bar_levels.step(now, state.input.as_deref());
                            state.scene.step(now, state.bar_levels.levels(), &layout);
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
                    let input = PaintInput {
                        scene: &state.scene,
                        layout,
                        levels: state.bar_levels.levels(),
                        origin: bounds.origin,
                        colors,
                        intensity: paint_intensity,
                        now_ms: state.paint_ms,
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
    const CANVAS_WIDTH: f32 = 200.0;
    const CANVAS_HEIGHT: f32 = 80.0;

    fn scene_options() -> ElectricSceneOptions {
        ElectricSceneOptions {
            arcs: true,
            bar_count: 4,
            intensity: 1.0,
            loading: false,
            reduced_motion: false,
            seed: Some(7.0),
            sparks: true,
        }
    }

    fn layout() -> ElectricLayout {
        layout_electric_bars(
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
            ElectricLayoutOptions {
                align: ElectricBarAlign::End,
                bar_count: 4,
                bar_gap: 4.0,
                bar_width: 6.0,
                orientation: Orientation::Horizontal,
            },
        )
    }

    fn run(scene: &mut ElectricScene, levels_at: impl Fn(usize) -> Vec<f32>, frames: usize) {
        for frame in 1..=frames {
            scene.step(frame as f64 * FRAME_MS, &levels_at(frame), &layout());
        }
    }

    fn alive(scene: &ElectricScene) -> usize {
        scene.sparks.life.iter().filter(|&&life| life > 0.0).count()
    }

    #[test]
    fn electric_layout_centres_the_row_of_bars() {
        let bars = layout();
        assert_eq!(bars.pitch, 10.0);
        assert_eq!(bars.first, 85.0);
        assert_eq!(bars.base, 74.0);
        assert_eq!(bars.span, 68.0);
    }

    #[test]
    fn electric_layout_narrows_bars_and_gaps_together_when_space_is_short() {
        let bars = layout_electric_bars(
            52.0,
            80.0,
            ElectricLayoutOptions {
                align: ElectricBarAlign::Center,
                bar_count: 8,
                bar_gap: 4.0,
                bar_width: 6.0,
                orientation: Orientation::Horizontal,
            },
        );
        assert_eq!(bars.pitch, 5.0);
        assert_eq!(bars.bar_width, 3.0);
        assert_eq!(bars.bar_gap, 2.0);
        assert_eq!(bars.base, 40.0);
    }

    #[test]
    fn electric_layout_swaps_axes_when_vertical() {
        let bars = layout_electric_bars(
            80.0,
            200.0,
            ElectricLayoutOptions {
                align: ElectricBarAlign::Start,
                bar_count: 4,
                bar_gap: 4.0,
                bar_width: 6.0,
                orientation: Orientation::Vertical,
            },
        );
        assert!(!bars.horizontal);
        assert_eq!(bars.first, 85.0);
        assert_eq!(bars.base, 6.0);
    }

    #[test]
    fn electric_scene_crackles_the_same_way_for_the_same_seed() {
        let mut first = create_electric_scene(scene_options());
        let mut second = create_electric_scene(scene_options());
        let mut other = create_electric_scene(ElectricSceneOptions {
            seed: Some(8.0),
            ..scene_options()
        });
        for scene in [&mut first, &mut second, &mut other] {
            run(scene, |_| vec![0.5; 4], 10);
        }
        assert_eq!(first.jitter, second.jitter);
        assert_ne!(first.jitter, other.jitter);
    }

    #[test]
    fn electric_scene_pins_the_base_of_each_filament_and_frees_its_tip() {
        let mut scene = create_electric_scene(scene_options());
        run(&mut scene, |_| vec![0.5; 4], 1);
        let points = scene.jitter.len() / 4;
        assert_eq!(scene.jitter[0], 0.0);
        assert_ne!(scene.jitter[points - 1], 0.0);
    }

    #[test]
    fn electric_scene_keeps_the_flicker_within_12_percent() {
        let mut scene = create_electric_scene(scene_options());
        run(&mut scene, |_| vec![1.0; 4], 60);
        for &brightness in &scene.flicker {
            assert!((0.88..=1.0).contains(&brightness));
        }
    }

    #[test]
    fn electric_scene_jumps_arcs_only_between_loud_bars_one_or_two_apart() {
        let levels = [1.0, 0.0, 1.0, 0.9, 0.0, 0.0];
        let mut scene = create_electric_scene(ElectricSceneOptions {
            bar_count: 6,
            ..scene_options()
        });
        let mut seen = std::collections::BTreeSet::new();
        for frame in 1..=600 {
            scene.step(frame as f64 * FRAME_MS, &levels, &layout());
            for arc in &scene.arcs {
                if arc.life_ms > 0.0 {
                    seen.insert((arc.index, arc.index + arc.reach));
                }
            }
        }
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), [(0, 2), (2, 3)]);
    }

    #[test]
    fn electric_scene_runs_one_arc_along_the_loading_sweep() {
        let mut scene = create_electric_scene(ElectricSceneOptions {
            bar_count: 8,
            loading: true,
            ..scene_options()
        });
        scene.step(
            FRAME_MS,
            &[0.0, 0.0, 0.0, 0.1, 0.5, 0.3, 0.0, 0.0],
            &layout(),
        );
        let live: Vec<_> = scene.arcs.iter().filter(|arc| arc.life_ms > 0.0).collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].index, 4);
    }

    #[test]
    fn electric_scene_throws_sparks_on_a_sudden_rise_and_they_fall_and_fade() {
        let mut scene = create_electric_scene(scene_options());
        run(
            &mut scene,
            |frame| {
                if frame < 3 {
                    vec![0.0; 4]
                } else {
                    vec![1.0, 0.0, 0.0, 0.0]
                }
            },
            3,
        );
        assert!(alive(&scene) > 0);
        let first_velocity = scene.sparks.vy[0];
        run(&mut scene, |_| vec![1.0, 0.0, 0.0, 0.0], 2);
        assert!(scene.sparks.vy[0] > first_velocity);
        run(&mut scene, |_| vec![1.0, 0.0, 0.0, 0.0], 60);
        assert_eq!(alive(&scene), 0);
    }

    #[test]
    fn electric_scene_never_holds_more_than_64_sparks() {
        let mut scene = create_electric_scene(ElectricSceneOptions {
            bar_count: 32,
            ..scene_options()
        });
        run(
            &mut scene,
            |frame| vec![if frame % 2 == 0 { 1.0 } else { 0.0 }; 32],
            40,
        );
        assert_eq!(scene.sparks.life.len(), 64);
        assert!(alive(&scene) <= 64);
    }

    #[test]
    fn electric_scene_stays_straight_and_still_with_reduced_motion() {
        let mut scene = create_electric_scene(ElectricSceneOptions {
            reduced_motion: true,
            ..scene_options()
        });
        run(
            &mut scene,
            |frame| vec![if frame % 2 == 0 { 1.0 } else { 0.0 }; 4],
            60,
        );
        assert!(scene.jitter.iter().all(|&offset| offset == 0.0));
        assert!(scene.arcs.iter().all(|arc| arc.life_ms == 0.0));
        assert_eq!(alive(&scene), 0);
    }

    #[test]
    fn electric_scene_draws_no_arcs_or_sparks_when_they_are_turned_off() {
        let mut scene = create_electric_scene(ElectricSceneOptions {
            arcs: false,
            sparks: false,
            ..scene_options()
        });
        run(
            &mut scene,
            |frame| vec![if frame % 4 == 0 { 0.0 } else { 1.0 }; 4],
            120,
        );
        assert!(scene.arcs.iter().all(|arc| arc.life_ms == 0.0));
        assert_eq!(alive(&scene), 0);
    }

    #[test]
    fn electric_bars_short_filaments_use_fewer_points() {
        assert_eq!(stride_for(0.0), 8);
        assert_eq!(stride_for(20.0), 4);
        assert_eq!(stride_for(80.0), 1);
        assert_eq!(stride_for(1000.0), 1);
    }

    #[test]
    fn electric_bars_bend_more_as_intensity_rises() {
        let layout = layout();
        assert_eq!(jitter_amplitude(&layout, 0.0, 0.2), 0.0);
        assert!(jitter_amplitude(&layout, 1.0, 0.2) > 0.0);
        // Short bars never bend more than a third of their length.
        assert!(jitter_amplitude(&layout, 1.0, 0.05) <= 0.05 * layout.span * MAX_BEND + 1e-6);
    }
}
