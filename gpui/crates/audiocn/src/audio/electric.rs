//! Port of `lib/electric.ts`.
//!
//! Shared drawing for the electric visualizers: repeatable crackle, a spark
//! pool, canvas sizing and theme colours.
//!
//! **What changed for GPUI.** The TS draws on two `<canvas>` elements (a
//! blurred glow canvas under a sharp one) and reads their colours from CSS.
//! Here:
//! - [`create_random`] / [`displace`] / the spark pool are unchanged, and the
//!   random sequence is bit-for-bit the TS one (computed in `f64`).
//! - The CSS classes become constants: [`ELECTRIC_GLOW_OPACITY_LIGHT`] /
//!   [`ELECTRIC_GLOW_OPACITY_DARK`], [`ELECTRIC_GLOW_SIZE`] (the blur),
//!   [`ELECTRIC_HEAT_LIGHT`] / [`ELECTRIC_HEAT_DARK`]; `readElectricColors`
//!   becomes [`electric_colors`] (resolve from the theme / props).
//! - `fitElectricCanvases` becomes [`fit_electric_canvases`] (pure sizing
//!   maths from bounds + `window.scale_factor()`); `clearElectricCanvas` has
//!   no equivalent (GPUI repaints from scratch each frame).
//! - `strokeSparks` becomes [`spark_streaks`] (geometry + alpha per spark) and
//!   [`stroke_sparks`] (paints them with `PathBuilder`).

use gpui::{Hsla, PathBuilder, Pixels, Point, Window, point, px};

use crate::audio::decibels::clamp64;
use crate::theme::mix;

const RANDOM_PERIOD: u32 = 1_000_003;

/// Glow layer opacity in light mode (`opacity-70`).
pub const ELECTRIC_GLOW_OPACITY_LIGHT: f32 = 0.7;
/// Glow layer opacity in dark mode (`dark:opacity-100`).
pub const ELECTRIC_GLOW_OPACITY_DARK: f32 = 1.0;
/// Default glow blur, `--electric-glow-size` (0.5rem), in pixels.
pub const ELECTRIC_GLOW_SIZE: f32 = 8.0;
/// Default `--electric-heat` in light mode: how far the core mixes toward white.
pub const ELECTRIC_HEAT_LIGHT: f32 = 0.0;
/// Default `--electric-heat` in dark mode.
pub const ELECTRIC_HEAT_DARK: f32 = 0.7;

fn fract(value: f64) -> f64 {
    value - value.floor()
}

/// A repeatable pseudo-random sequence, 0..1 (`createRandom`). Call
/// [`next`](Self::next), or pass `|| random.next()` where a `() => number` goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeededRandom {
    seed: f64,
    counter: u32,
}

/// A repeatable pseudo-random sequence for `seed`, 0..1.
pub fn create_random(seed: f64) -> SeededRandom {
    SeededRandom { seed, counter: 0 }
}

impl SeededRandom {
    /// The next value, 0..1 (identical to the TS sequence).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        self.counter = (self.counter + 1) % RANDOM_PERIOD;
        fract((self.seed * 12.9898 + self.counter as f64 * 78.233).sin() * 43_758.545_3)
    }

    /// [`next`](Self::next) as `f32`.
    pub fn next_f32(&mut self) -> f32 {
        self.next() as f32
    }
}

/// Midpoint displacement: fills `count` offsets from `start` (`count - 1` a
/// power of two) between the two end values, big bends first and finer ones
/// on top. Each finer split bends `roughness` times the one before.
/// `random` returns 0..1, e.g. `|| rng.next()`.
pub fn displace(
    out: &mut [f32],
    start: usize,
    count: usize,
    ends: [f32; 2],
    mut random: impl FnMut() -> f64,
    roughness: f32,
) {
    if count == 0 {
        return;
    }
    let last = count - 1;
    out[start] = ends[0];
    out[start + last] = ends[1];
    let mut amplitude = 1.0f64;
    let mut step = last;
    while step > 1 {
        let half = step / 2;
        let mut index = half;
        while index < last {
            let before = out[start + index - half] as f64;
            let after = out[start + index + half] as f64;
            out[start + index] =
                ((before + after) / 2.0 + (random() * 2.0 - 1.0) * amplitude) as f32;
            index += step;
        }
        amplitude *= roughness as f64;
        step /= 2;
    }
}

/// The drawing colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricColors {
    /// The arc body (`--electric`, else the text colour).
    pub body: Hsla,
    /// The hot core (`--electric-core`, else the body mixed toward white by `heat`).
    pub core: Hsla,
    /// The blurred glow (`--electric-glow`, else `--electric`, else the text colour).
    pub glow: Hsla,
}

/// Resolves the colours like the CSS does. `color` is `--electric` or the
/// current text colour; `heat` is 0..1 ([`ELECTRIC_HEAT_LIGHT`] /
/// [`ELECTRIC_HEAT_DARK`] by default). The mix runs in OKLab (CSS: OKLCH).
pub fn electric_colors(
    color: Hsla,
    core: Option<Hsla>,
    glow: Option<Hsla>,
    heat: f32,
) -> ElectricColors {
    ElectricColors {
        body: color,
        core: core.unwrap_or_else(|| mix(color, gpui::white(), heat)),
        glow: glow.unwrap_or(color),
    }
}

/// Sizes of the two layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElectricCanvasSize {
    /// Logical pixels.
    pub width: f32,
    pub height: f32,
    /// Device pixels per logical pixel on the sharp layer.
    pub ratio: f32,
    /// Device pixels per logical pixel on the glow layer. It is blurred, so it
    /// never needs more than one.
    pub glow_ratio: f32,
}

/// Sizing for both layers from the sharp layer's box and the scale factor
/// (`window.scale_factor()`; 0 counts as 1).
pub fn fit_electric_canvases(width: f32, height: f32, scale_factor: f32) -> ElectricCanvasSize {
    let ratio = if scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    ElectricCanvasSize {
        width,
        height,
        ratio,
        glow_ratio: ratio.min(1.0),
    }
}

/// A fixed pool of sparks, in canvas pixels. A spark with `life` 0 is gone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ElectricSparks {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    /// Milliseconds.
    pub age: Vec<f32>,
    /// Milliseconds; 0 when the spark is gone.
    pub life: Vec<f32>,
    /// The slot the next spark takes, replacing the oldest when full.
    pub next: usize,
}

/// A pool of `size` sparks, all gone.
pub fn create_electric_sparks(size: usize) -> ElectricSparks {
    ElectricSparks {
        x: vec![0.0; size],
        y: vec![0.0; size],
        vx: vec![0.0; size],
        vy: vec![0.0; size],
        age: vec![0.0; size],
        life: vec![0.0; size],
        next: 0,
    }
}

/// A new spark.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SparkLaunch {
    pub x: f32,
    pub y: f32,
    /// Pixels per second.
    pub vx: f32,
    pub vy: f32,
    pub life_ms: f32,
}

/// Launches a spark into the next slot (replacing the oldest when full).
pub fn emit_spark(sparks: &mut ElectricSparks, launch: SparkLaunch) {
    let size = sparks.life.len();
    if size == 0 {
        return;
    }
    let slot = sparks.next;
    sparks.next = (slot + 1) % size;
    sparks.x[slot] = launch.x;
    sparks.y[slot] = launch.y;
    sparks.vx[slot] = launch.vx;
    sparks.vy[slot] = launch.vy;
    sparks.age[slot] = 0.0;
    sparks.life[slot] = launch.life_ms;
}

/// Moves every live spark, pulled down by `gravity` in pixels per second squared.
pub fn move_sparks(sparks: &mut ElectricSparks, seconds: f32, gravity: f32) {
    let (seconds, gravity) = (seconds as f64, gravity as f64);
    for slot in 0..sparks.life.len() {
        let life = sparks.life[slot];
        if life > 0.0 {
            let age = sparks.age[slot] as f64 + seconds * 1000.0;
            sparks.age[slot] = age as f32;
            if age >= life as f64 {
                sparks.life[slot] = 0.0;
            } else {
                let vy = sparks.vy[slot] as f64 + gravity * seconds;
                sparks.vy[slot] = vy as f32;
                sparks.x[slot] = (sparks.x[slot] as f64 + sparks.vx[slot] as f64 * seconds) as f32;
                sparks.y[slot] = (sparks.y[slot] as f64 + vy * seconds) as f32;
            }
        }
    }
}

/// One live spark as a streak: from its trail end to its head, fading with age.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SparkStreak {
    pub from: (f32, f32),
    pub to: (f32, f32),
    /// 0..1.
    pub alpha: f32,
}

/// Each live spark as a short streak `trail_seconds` long.
pub fn spark_streaks(
    sparks: &ElectricSparks,
    trail_seconds: f32,
) -> impl Iterator<Item = SparkStreak> + '_ {
    sparks
        .life
        .iter()
        .enumerate()
        .filter(|(_, life)| **life > 0.0)
        .map(move |(slot, &life)| {
            let (x, y) = (sparks.x[slot], sparks.y[slot]);
            SparkStreak {
                from: (
                    x - sparks.vx[slot] * trail_seconds,
                    y - sparks.vy[slot] * trail_seconds,
                ),
                to: (x, y),
                alpha: clamp64(1.0 - sparks.age[slot] as f64 / life as f64, 0.0, 1.0) as f32,
            }
        })
}

/// Draws each live spark as a short streak that fades with age. Spark
/// coordinates are relative to `origin` (the canvas's top-left).
pub fn stroke_sparks(
    window: &mut Window,
    origin: Point<Pixels>,
    sparks: &ElectricSparks,
    color: Hsla,
    line_width: f32,
    trail_seconds: f32,
) {
    for streak in spark_streaks(sparks, trail_seconds) {
        let mut builder = PathBuilder::stroke(px(line_width));
        builder.move_to(origin + point(px(streak.from.0), px(streak.from.1)));
        builder.line_to(origin + point(px(streak.to.0), px(streak.to.1)));
        if let Ok(path) = builder.build() {
            window.paint_path(path, color.opacity(color.a * streak.alpha));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spread(roughness: f32) -> f32 {
        let mut out = vec![0.0f32; 257];
        let mut random = create_random(9.0);
        displace(&mut out, 0, 257, [0.0, 0.0], || random.next(), roughness);
        out.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }

    fn sequence(seed: f64) -> Vec<f64> {
        let mut random = create_random(seed);
        (0..50).map(|_| random.next()).collect()
    }

    #[test]
    fn random_repeats_for_a_seed_and_stays_in_range() {
        let a = sequence(3.0);
        assert_eq!(sequence(3.0), a);
        assert_ne!(sequence(4.0), a);
        assert!(a.iter().all(|v| (0.0..1.0).contains(v)));
    }

    #[test]
    fn random_matches_javascript() {
        // Values printed by node for createRandom(1) and createRandom(7.5).
        let mut one = create_random(1.0);
        for expected in [0.740084824198675, 0.07390410361767863, 0.4199000090593472] {
            assert!((one.next() - expected).abs() < 1e-9);
        }
        let mut other = create_random(7.5);
        for expected in [0.8757241622370202, 0.6069754138334247] {
            assert!((other.next() - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn displace_keeps_the_ends_and_bends_between() {
        let mut out = vec![0.0f32; 20];
        let mut random = create_random(1.0);
        displace(&mut out, 2, 17, [0.25, -0.5], || random.next(), 0.6);
        assert_eq!(out[2], 0.25);
        assert_eq!(out[18], -0.5);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[19], 0.0);
        assert!(out[3..18].iter().any(|&v| v != 0.0));
    }

    #[test]
    fn displace_bends_less_with_lower_roughness() {
        assert!(spread(0.3) < spread(0.8));
    }

    #[test]
    fn sparks_fly_fall_and_fade() {
        let mut sparks = create_electric_sparks(4);
        emit_spark(
            &mut sparks,
            SparkLaunch {
                life_ms: 100.0,
                vx: 10.0,
                vy: -50.0,
                x: 5.0,
                y: 5.0,
            },
        );
        move_sparks(&mut sparks, 0.05, 100.0);
        assert!((sparks.x[0] - 5.5).abs() < 1e-5);
        assert!((sparks.vy[0] - -45.0).abs() < 1e-5);
        let streak = spark_streaks(&sparks, 0.1).next().unwrap();
        assert!((streak.alpha - 0.5).abs() < 1e-5);
        move_sparks(&mut sparks, 0.06, 100.0);
        assert_eq!(sparks.life[0], 0.0);
        assert_eq!(spark_streaks(&sparks, 0.1).count(), 0);
    }

    #[test]
    fn sparks_replace_the_oldest_when_full() {
        let mut sparks = create_electric_sparks(2);
        for x in [1.0, 2.0, 3.0] {
            emit_spark(
                &mut sparks,
                SparkLaunch {
                    life_ms: 100.0,
                    x,
                    ..Default::default()
                },
            );
        }
        assert_eq!(sparks.x, [3.0, 2.0]);
    }
}
