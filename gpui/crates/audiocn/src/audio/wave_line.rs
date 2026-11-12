//! Port of `lib/audio/wave-line.ts`.
//!
//! The shape of a single line across the width, shared by the line
//! visualizers. Keep a [`WaveLine`] in element state, call
//! [`WaveLine::step`] each animation frame, and draw
//! [`WaveLine::heights`] (`count` points, left to right).

use std::f64::consts::PI;

use crate::audio::bar_levels::{BarIdle, BarLevels, BarLevelsOptions, create_bar_levels};
use crate::audio::decibels::clamp64;
use crate::audio::types::VisualFrame;

const MS_PER_SECOND: f64 = 1000.0;
const FRAME_MS: f64 = 16.67;
const MAX_STEP_SECONDS: f64 = 0.05;
const TWO_PI: f64 = PI * 2.0;
/// Below this the line counts as silent.
const SIGNAL_THRESHOLD: f32 = 0.02;
/// Share of the distance to the new shape still left after one frame.
const EASE_PER_FRAME: f64 = 0.45;
/// Loudness left after one frame of silence.
const LOUDNESS_RELEASE: f64 = 0.92;

struct WavePartial {
    cycles: f64,
    drift: f64,
    weight: f64,
}

/// Cycles across the width, drift in radians per second and weight of each band group, lows first.
const WAVE_PARTIALS: [WavePartial; 4] = [
    WavePartial {
        cycles: 1.5,
        drift: 0.8,
        weight: 0.6,
    },
    WavePartial {
        cycles: 2.5,
        drift: -1.3,
        weight: 0.4,
    },
    WavePartial {
        cycles: 4.0,
        drift: 2.1,
        weight: 0.25,
    },
    WavePartial {
        cycles: 6.5,
        drift: -3.2,
        weight: 0.16,
    },
];

/// Scope mode lifts quiet signals by at most this much.
const SCOPE_MAX_GAIN: f64 = 4.0;
/// Scope mode scales its running peak to this height.
const SCOPE_TARGET: f64 = 0.9;
/// Share of the scope's running peak left after one frame.
const SCOPE_PEAK_RELEASE: f64 = 0.985;

/// Widths per second the loading pulse travels.
const LOADING_SPEED: f64 = 0.55;
/// Where the loading pulse rests with reduced motion: the middle.
const LOADING_REST_SECONDS: f64 = 0.7 / LOADING_SPEED;

/// Most points a line can have.
pub const WAVE_LINE_MAX_POINTS: usize = 513;

/// `Wave` is a smooth wave shaped by the frequency bands. `Scope` is the
/// signal itself, like an oscilloscope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WaveLineMode {
    #[default]
    Wave,
    Scope,
}

/// Options for [`create_wave_line`]. All fields are required in the TS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveLineOptions {
    pub mode: WaveLineMode,
    /// Runs a pulse along the line instead of following the signal.
    pub loading: bool,
    /// Visual gain.
    pub sensitivity: f32,
    /// No drift or easing: the line jumps to each new shape.
    pub reduced_motion: bool,
}

/// Where the signal first rises through zero in its first quarter, between
/// samples, so a steady tone stands still. 0 when it never does.
pub fn trigger_index(samples: &[f32]) -> f32 {
    let limit = samples.len() / 4;
    for index in 1..=limit {
        let before = samples[index - 1] as f64;
        let after = samples.get(index).copied().unwrap_or(0.0) as f64;
        if before < 0.0 && after >= 0.0 {
            return (index as f64 - 1.0 + -before / (after - before)) as f32;
        }
    }
    0.0
}

fn peak_of(values: &[f32], count: usize) -> f64 {
    values
        .iter()
        .take(count)
        .fold(0.0f64, |peak, &v| peak.max((v as f64).abs()))
}

/// Fits three quarters of the samples, from the trigger, into `count` heights.
fn fill_scope(samples: &[f32], out: &mut [f32], count: usize, gain: f64) {
    let start = trigger_index(samples) as f64;
    let span = (samples.len() as f64 * 0.75).floor() - 1.0;
    let last = samples.len() - 1;
    for (point, slot) in out.iter_mut().enumerate().take(count) {
        let position = start + (span * point as f64) / (count as f64 - 1.0);
        let index = position.floor() as usize;
        let before = samples.get(index).copied().unwrap_or(0.0) as f64;
        let after = samples
            .get(last.min(index + 1))
            .map_or(before, |&v| v as f64);
        let value = before + (after - before) * (position - index as f64);
        *slot = clamp64(value * gain, -1.0, 1.0) as f32;
    }
}

/// A short wave packet that travels left to right.
fn loading_pulse(position: f64, seconds: f64) -> f64 {
    let center = ((seconds * LOADING_SPEED) % 1.4) - 0.2;
    let envelope = (-((position - center).powi(2)) / 0.012).exp();
    0.6 * envelope * (TWO_PI * (position * 7.0 - seconds * 2.5)).sin()
}

/// A drifting wave shaped by the frequency bands, a triggered oscilloscope
/// trace, or a loading pulse, eased from frame to frame.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveLine {
    /// Points in use, left to right (2..=[`WAVE_LINE_MAX_POINTS`]).
    pub count: usize,
    /// Height at each point, −1..1, up positive, eased toward the latest shape.
    /// Always [`WAVE_LINE_MAX_POINTS`] long; only the first `count` are in use.
    pub heights: Vec<f32>,
    /// Highest point of the latest shape before easing, 0..1. It jumps on sudden rises.
    pub peak: f32,
    /// How loud the line is, 0..1: its highest point, released slowly.
    pub loudness: f32,
    options: WaveLineOptions,
    targets: Vec<f32>,
    bands: BarLevels,
    phases: [f32; 4],
    last_ms: f64,
    scope_peak: f64,
}

/// Creates a flat [`WaveLine`].
pub fn create_wave_line(options: WaveLineOptions) -> WaveLine {
    WaveLine::new(options)
}

impl WaveLine {
    /// Same as [`create_wave_line`].
    pub fn new(options: WaveLineOptions) -> Self {
        Self {
            count: 2,
            heights: vec![0.0; WAVE_LINE_MAX_POINTS],
            peak: 0.0,
            loudness: 0.0,
            options,
            targets: vec![0.0; WAVE_LINE_MAX_POINTS],
            bands: create_bar_levels(BarLevelsOptions {
                bar_count: WAVE_PARTIALS.len(),
                idle: BarIdle::Static,
                loading: false,
                min_level: 0.0,
                mirrored: false,
                reduced_motion: options.reduced_motion,
            }),
            phases: [0.0; 4],
            last_ms: 0.0,
            scope_peak: 0.0,
        }
    }

    /// The options this was created with.
    pub fn options(&self) -> WaveLineOptions {
        self.options
    }

    /// The heights in use: `&heights[..count]`.
    pub fn points(&self) -> &[f32] {
        &self.heights[..self.count]
    }

    fn fill_wave(&mut self, frame: Option<&VisualFrame>, now_ms: f64) {
        self.bands.step(now_ms, frame.map(|f| f.bands.as_slice()));
        let sensitivity = self.options.sensitivity as f64;
        let levels = self.bands.levels();
        for point in 0..self.count {
            let position = point as f64 / (self.count as f64 - 1.0);
            let mut height = 0.0;
            for (index, partial) in WAVE_PARTIALS.iter().enumerate() {
                height += partial.weight
                    * levels[index] as f64
                    * (TWO_PI * partial.cycles * position + self.phases[index] as f64).sin();
            }
            self.targets[point] = clamp64(height * sensitivity, -1.0, 1.0) as f32;
        }
    }

    fn fill_targets(&mut self, now_ms: f64, frame: Option<&VisualFrame>) {
        let count = self.count;
        let o = self.options;
        if o.loading {
            let seconds = if o.reduced_motion {
                LOADING_REST_SECONDS
            } else {
                now_ms / MS_PER_SECOND
            };
            for point in 0..count {
                self.targets[point] =
                    loading_pulse(point as f64 / (count as f64 - 1.0), seconds) as f32;
            }
            return;
        }
        let samples = frame.and_then(|f| f.time_domain.as_deref());
        if o.mode == WaveLineMode::Scope
            && let Some(samples) = samples
            && samples.len() > 1
        {
            self.scope_peak =
                peak_of(samples, samples.len()).max(self.scope_peak * SCOPE_PEAK_RELEASE);
            let peak = if self.scope_peak == 0.0 || self.scope_peak.is_nan() {
                1.0
            } else {
                self.scope_peak
            };
            let gain = o.sensitivity as f64 * SCOPE_MAX_GAIN.min(SCOPE_TARGET / peak);
            fill_scope(samples, &mut self.targets, count, gain);
            return;
        }
        self.fill_wave(frame, now_ms);
    }

    fn ease(&mut self, elapsed_ms: f64) {
        let share = if self.options.reduced_motion {
            1.0
        } else {
            1.0 - EASE_PER_FRAME.powf(elapsed_ms / FRAME_MS)
        };
        for point in 0..self.count {
            let height = self.heights[point] as f64;
            self.heights[point] = (height + (self.targets[point] as f64 - height) * share) as f32;
        }
        let release = LOUDNESS_RELEASE.powf(elapsed_ms / FRAME_MS);
        self.loudness =
            peak_of(&self.heights, self.count).max(self.loudness as f64 * release) as f32;
    }

    /// Advances to `now_ms` with the latest frame, over `count` points
    /// (rounded, clamped to 2..=[`WAVE_LINE_MAX_POINTS`]). Returns true when
    /// there is a signal (never while loading).
    pub fn step(&mut self, now_ms: f64, frame: Option<&VisualFrame>, count: f32) -> bool {
        let elapsed_ms = if self.last_ms == 0.0 {
            FRAME_MS
        } else {
            now_ms - self.last_ms
        };
        self.last_ms = now_ms;
        // Math.round: halves round up.
        let rounded = (count as f64 + 0.5).floor();
        self.count = clamp64(rounded, 2.0, WAVE_LINE_MAX_POINTS as f64) as usize;
        if !self.options.reduced_motion {
            let seconds = clamp64(elapsed_ms / MS_PER_SECOND, 0.0, MAX_STEP_SECONDS);
            for (phase, partial) in self.phases.iter_mut().zip(WAVE_PARTIALS.iter()) {
                *phase = ((*phase as f64 + partial.drift * seconds) % TWO_PI) as f32;
            }
        }
        self.fill_targets(now_ms, frame);
        self.peak = peak_of(&self.targets, self.count) as f32;
        self.ease(elapsed_ms);
        !self.options.loading && self.peak >= SIGNAL_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f64 = 16.0;
    const POINTS: f32 = 65.0;

    fn frame_of(bands: &[f32], time_domain: Option<Vec<f32>>) -> VisualFrame {
        VisualFrame {
            bands: bands.to_vec(),
            history: vec![0.0; 1],
            history_length: 0,
            history_start: 0,
            peak_db: -12.0,
            time_domain,
            ..Default::default()
        }
    }

    fn sine(length: usize, cycles: f64, phase: f64, gain: f64) -> Vec<f32> {
        (0..length)
            .map(|i| (gain * ((2.0 * PI * cycles * i as f64) / length as f64 + phase).sin()) as f32)
            .collect()
    }

    fn steady_tone(_: usize) -> Option<VisualFrame> {
        Some(frame_of(&[0.0], Some(sine(256, 4.0, 0.0, 0.9))))
    }

    fn options() -> WaveLineOptions {
        WaveLineOptions {
            loading: false,
            mode: WaveLineMode::Wave,
            reduced_motion: false,
            sensitivity: 1.0,
        }
    }

    fn run(
        line: &mut WaveLine,
        frame_at: impl Fn(usize) -> Option<VisualFrame>,
        frames: usize,
    ) -> bool {
        let mut active = false;
        for frame in 1..=frames {
            let f = frame_at(frame);
            active = line.step(frame as f64 * FRAME, f.as_ref(), POINTS);
        }
        active
    }

    fn max(v: &[f32]) -> f32 {
        v.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    fn min(v: &[f32]) -> f32 {
        v.iter().fold(f32::INFINITY, |a, &b| a.min(b))
    }

    fn scope_shape(phase: f64) -> Vec<f32> {
        let mut line = create_wave_line(WaveLineOptions {
            mode: WaveLineMode::Scope,
            reduced_motion: true,
            ..options()
        });
        run(
            &mut line,
            |_| Some(frame_of(&[0.0], Some(sine(256, 8.0, phase, 0.9)))),
            1,
        );
        line.points().to_vec()
    }

    fn pulse_at(frames: usize) -> usize {
        let mut line = create_wave_line(WaveLineOptions {
            loading: true,
            ..options()
        });
        run(&mut line, |_| None, frames);
        let heights: Vec<f32> = line.points().iter().map(|h| h.abs()).collect();
        let top = max(&heights);
        heights.iter().position(|&h| h == top).unwrap()
    }

    #[test]
    fn trigger_finds_first_rising_crossing() {
        let samples = [0.5, 0.2, -0.3, 0.1, 0.4, 0.6, 0.2, -0.2];
        let doubled: Vec<f32> = samples.iter().chain(samples.iter()).copied().collect();
        assert!((trigger_index(&doubled) - 2.75).abs() < 1e-5);
    }

    #[test]
    fn trigger_starts_at_zero_without_early_crossing() {
        assert_eq!(
            trigger_index(&[0.5, 0.4, 0.3, 0.2, 0.1, 0.0, -0.1, -0.2]),
            0.0
        );
    }

    #[test]
    fn uses_requested_points() {
        let mut line = create_wave_line(options());
        line.step(FRAME, None, 101.0);
        assert_eq!(line.count, 101);
    }

    #[test]
    fn rests_flat_without_signal() {
        let mut line = create_wave_line(options());
        assert!(!run(&mut line, |_| None, 30));
        assert_eq!(
            max(&line.points().iter().map(|h| h.abs()).collect::<Vec<_>>()),
            0.0
        );
        assert_eq!(line.loudness, 0.0);
    }

    #[test]
    fn shapes_a_wave_from_bands() {
        let mut line = create_wave_line(options());
        assert!(run(&mut line, |_| Some(frame_of(&[1.0; 8], None)), 30));
        let heights = line.points();
        assert!(max(heights) > 0.2);
        assert!(min(heights) < -0.2);
        assert!(heights.iter().all(|h| h.abs() <= 1.0));
        assert!(line.loudness > 0.2);
    }

    #[test]
    fn eases_and_jumps_with_reduced_motion() {
        let mut eased = create_wave_line(WaveLineOptions {
            mode: WaveLineMode::Scope,
            ..options()
        });
        let mut still = create_wave_line(WaveLineOptions {
            mode: WaveLineMode::Scope,
            reduced_motion: true,
            ..options()
        });
        run(&mut eased, steady_tone, 1);
        run(&mut still, steady_tone, 1);
        assert!((eased.peak - still.peak).abs() < 1e-5);
        assert!(max(eased.points()) < max(still.points()));
    }

    #[test]
    fn holds_a_steady_tone_still_in_scope_mode() {
        let first = scope_shape(0.0);
        let shifted = scope_shape(1.3);
        for (a, b) in first.iter().zip(shifted.iter()) {
            assert!((a - b).abs() < 0.05);
        }
    }

    #[test]
    fn lifts_a_quiet_scope_signal() {
        let mut line = create_wave_line(WaveLineOptions {
            mode: WaveLineMode::Scope,
            reduced_motion: true,
            ..options()
        });
        run(
            &mut line,
            |_| Some(frame_of(&[0.0], Some(sine(256, 4.0, 0.0, 0.1)))),
            1,
        );
        assert!(max(line.points()) > 0.3);
    }

    #[test]
    fn runs_a_pulse_while_loading() {
        assert!(pulse_at(40) > pulse_at(20));
        let mut line = create_wave_line(WaveLineOptions {
            loading: true,
            ..options()
        });
        assert!(!run(&mut line, |_| Some(frame_of(&[1.0; 4], None)), 30));
    }
}
