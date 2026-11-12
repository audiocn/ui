//! Port of `lib/audio/bar-levels.ts`.
//!
//! Bar levels shared by the bar visualizers. Keep a [`BarLevels`] in element
//! state, call [`BarLevels::step`] each animation frame with the latest bands,
//! paint [`BarLevels::levels`], and stop requesting frames once
//! [`BarLevels::settled`] (and there is no live source).

use crate::audio::bands::resample_levels;
use crate::audio::decibels::clamp64;

const RELEASE_PER_FRAME: f64 = 0.86;
const FRAME_MS: f64 = 16.67;
const SIGNAL_THRESHOLD: f32 = 0.02;
const MS_PER_SECOND: f64 = 1000.0;
/// Bars this close to their target have nothing left to animate.
const SETTLE_EPSILON: f64 = 0.002;

/// What bars do with no signal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BarIdle {
    #[default]
    Static,
    Pulse,
    Wave,
}

/// Options for [`create_bar_levels`]. All fields are required in the TS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarLevelsOptions {
    pub bar_count: usize,
    pub idle: BarIdle,
    /// Runs a sweep, for connecting or thinking states.
    pub loading: bool,
    /// Resting bar size, 0..1.
    pub min_level: f32,
    /// Symmetric around the middle bar, lows in the centre.
    pub mirrored: bool,
    /// No release smoothing, idle animation or sweep.
    pub reduced_motion: bool,
}

fn idle_level(idle: BarIdle, index: usize, seconds: f64) -> f64 {
    match idle {
        BarIdle::Pulse => 0.1 * (0.5 + 0.5 * (seconds * std::f64::consts::PI * 1.6).sin()),
        BarIdle::Wave => 0.16 * (0.5 + 0.5 * (seconds * 4.0 - index as f64 * 0.55).sin()),
        BarIdle::Static => 0.0,
    }
}

fn sweep_level(index: usize, count: usize, seconds: f64) -> f64 {
    let count = count as f64;
    let span = count + 6.0;
    // JS `%` keeps the dividend's sign, like Rust's.
    let position = ((seconds * count * 0.9) % span) - 3.0;
    0.55 * (-((index as f64 - position).powi(2)) / 3.0).exp()
}

fn target_index_for(index: usize, count: usize, half: usize, mirrored: bool) -> usize {
    if !mirrored {
        return index;
    }
    let distance = (index as f64 - (count as f64 - 1.0) / 2.0).abs().floor() as usize;
    distance.min(half.saturating_sub(1))
}

/// Bar levels: input resampled to the bar count, instant attack with a
/// smooth release, mirroring, idle animations and the loading sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLevels {
    options: BarLevelsOptions,
    half: usize,
    targets: Vec<f32>,
    current: Vec<f32>,
    levels: Vec<f32>,
    last_ms: f64,
    settled: bool,
}

/// Creates bar levels, all at `min_level`.
pub fn create_bar_levels(options: BarLevelsOptions) -> BarLevels {
    BarLevels::new(options)
}

impl BarLevels {
    /// Same as [`create_bar_levels`].
    pub fn new(options: BarLevelsOptions) -> Self {
        let bar_count = options.bar_count;
        let half = if options.mirrored {
            bar_count.div_ceil(2)
        } else {
            bar_count
        };
        Self {
            options,
            half,
            targets: vec![0.0; half],
            current: vec![0.0; bar_count],
            levels: vec![options.min_level; bar_count],
            last_ms: 0.0,
            settled: false,
        }
    }

    /// The options this was created with.
    pub fn options(&self) -> BarLevelsOptions {
        self.options
    }

    /// The level each bar shows, 0..1, never below `min_level`.
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// True after a step that left nothing to animate: every bar is at its
    /// target, and no idle animation or loading sweep is running.
    pub fn settled(&self) -> bool {
        self.settled
    }

    fn extra_level(&self, index: usize, seconds: f64, quiet: bool) -> f64 {
        let o = &self.options;
        if o.loading && !o.reduced_motion {
            return sweep_level(index, o.bar_count, seconds);
        }
        if quiet {
            idle_level(o.idle, index, seconds)
        } else {
            0.0
        }
    }

    /// Advances to `now_ms` with the latest input levels (resampled to the bar
    /// count). Returns true when the input is above the signal floor.
    pub fn step(&mut self, now_ms: f64, input: Option<&[f32]>) -> bool {
        let o = self.options;
        let elapsed = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            now_ms - self.last_ms
        };
        self.last_ms = now_ms;
        let release = if o.reduced_motion {
            0.0
        } else {
            RELEASE_PER_FRAME.powf(elapsed / FRAME_MS)
        };
        let seconds = now_ms / MS_PER_SECOND;

        match input {
            Some(input) if !input.is_empty() => {
                resample_levels(input, 0, input.len(), &mut self.targets);
            }
            _ => self.targets.fill(0.0),
        }
        let loudest = self.targets.iter().fold(0.0f32, |a, &b| a.max(b));
        let active = loudest >= SIGNAL_THRESHOLD;
        let quiet = !(o.reduced_motion || active);
        let animating = (o.loading && !o.reduced_motion) || (quiet && o.idle != BarIdle::Static);
        let mut converged = true;

        for index in 0..o.bar_count {
            let input_target = self
                .targets
                .get(target_index_for(index, o.bar_count, self.half, o.mirrored))
                .copied()
                .unwrap_or(0.0) as f64;
            let target = input_target.max(self.extra_level(index, seconds, quiet));
            let previous = self.current[index] as f64;
            let next = if target >= previous {
                target
            } else {
                previous * release + target * (1.0 - release)
            };
            // Stored as f32, like the TS Float32Array.
            self.current[index] = next as f32;
            self.levels[index] = clamp64((o.min_level as f64).max(next), 0.0, 1.0) as f32;
            if (next - target).abs() > SETTLE_EPSILON {
                converged = false;
            }
        }
        self.settled = converged && !animating;
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: f64 = 16.67;

    fn options() -> BarLevelsOptions {
        BarLevelsOptions {
            bar_count: 4,
            idle: BarIdle::Static,
            loading: false,
            min_level: 0.0,
            mirrored: false,
            reduced_motion: false,
        }
    }

    fn close(a: f32, b: f32, digits: i32) -> bool {
        ((a - b).abs() as f64) < 10f64.powi(-digits) / 2.0
    }

    fn max(levels: &[f32]) -> f32 {
        levels.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    #[test]
    fn resamples_input_to_bar_count() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 2,
            ..options()
        });
        bars.step(FRAME_MS, Some(&[0.2, 0.6, 1.0, 0.4]));
        assert!(close(bars.levels()[0], 0.6, 5) && close(bars.levels()[1], 1.0, 5));
    }

    #[test]
    fn rises_instantly_and_releases_smoothly() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 1,
            ..options()
        });
        bars.step(FRAME_MS, Some(&[1.0]));
        assert_eq!(bars.levels()[0], 1.0);
        bars.step(FRAME_MS * 2.0, Some(&[0.0]));
        assert!(close(bars.levels()[0], 0.86, 2));
    }

    #[test]
    fn settles_once_every_bar_reaches_target() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 1,
            ..options()
        });
        bars.step(FRAME_MS, Some(&[1.0]));
        assert!(bars.settled());
        let mut now = FRAME_MS * 2.0;
        bars.step(now, Some(&[0.0]));
        assert!(!bars.settled());
        for _ in 0..120 {
            now += FRAME_MS;
            bars.step(now, Some(&[0.0]));
        }
        assert!(bars.settled());
    }

    #[test]
    fn never_settles_while_idle_or_loading() {
        let mut pulsing = create_bar_levels(BarLevelsOptions {
            idle: BarIdle::Pulse,
            ..options()
        });
        let mut loading = create_bar_levels(BarLevelsOptions {
            loading: true,
            ..options()
        });
        for frame in 1..=120 {
            pulsing.step(frame as f64 * FRAME_MS, None);
            loading.step(frame as f64 * FRAME_MS, Some(&[0.5]));
        }
        assert!(!pulsing.settled());
        assert!(!loading.settled());
        pulsing.step(121.0 * FRAME_MS, Some(&[0.5, 0.5, 0.5, 0.5]));
        assert!(pulsing.settled());
    }

    #[test]
    fn drops_at_once_with_reduced_motion() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 1,
            reduced_motion: true,
            ..options()
        });
        bars.step(FRAME_MS, Some(&[1.0]));
        bars.step(FRAME_MS * 2.0, Some(&[0.0]));
        assert_eq!(bars.levels()[0], 0.0);
    }

    #[test]
    fn never_below_resting_level() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            min_level: 0.1,
            ..options()
        });
        bars.step(FRAME_MS, None);
        assert_eq!(bars.levels(), &[0.1f32; 4]);
    }

    #[test]
    fn reports_signal_floor() {
        let mut bars = create_bar_levels(options());
        assert!(!bars.step(FRAME_MS, Some(&[0.0, 0.01, 0.0, 0.0])));
        assert!(bars.step(FRAME_MS * 2.0, Some(&[0.0, 0.5, 0.0, 0.0])));
    }

    #[test]
    fn mirrors_lows_into_centre() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 5,
            mirrored: true,
            ..options()
        });
        bars.step(FRAME_MS, Some(&[1.0, 0.5, 0.0]));
        let l = bars.levels();
        assert_eq!(l[2], 1.0);
        assert_eq!(l[1], l[3]);
        assert_eq!(l[0], l[4]);
    }

    #[test]
    fn idle_wave_only_without_signal() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            idle: BarIdle::Wave,
            ..options()
        });
        bars.step(1000.0, None);
        assert!(max(bars.levels()) > 0.0);
        let mut loud = create_bar_levels(BarLevelsOptions {
            idle: BarIdle::Wave,
            ..options()
        });
        loud.step(1000.0, Some(&[0.5, 0.5, 0.5, 0.5]));
        assert_eq!(loud.levels(), &[0.5f32; 4]);
    }

    #[test]
    fn sweeps_while_loading() {
        let mut bars = create_bar_levels(BarLevelsOptions {
            bar_count: 8,
            loading: true,
            ..options()
        });
        bars.step(1000.0, None);
        assert!(max(bars.levels()) > 0.1);
    }
}
