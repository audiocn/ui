//! Port of `lib/audio/ballistics.ts`.
//!
//! How a meter moves between readings. `BallisticsInput` is either a preset
//! (`"peak"`, `"vu"`, `"instant"` → [`BallisticsPreset`]) or a partial set of
//! options merged over the peak preset (→ [`PartialBallisticsOptions`]):
//!
//! ```ignore
//! let input: BallisticsInput = BallisticsPreset::Vu.into();
//! let input: BallisticsInput = PartialBallisticsOptions { release_ms: Some(100.0), ..Default::default() }.into();
//! let mut meter = create_ballistics(Some(&input));
//! let state = meter.step(peak_db, now_ms());
//! ```

use crate::audio::decibels::{db_to_gain64, gain_to_db64};

/// How a meter moves between readings. All times are in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallisticsOptions {
    /// Time constant for rising levels. 0 jumps instantly.
    pub attack_ms: f32,
    /// Time constant for falling levels. 0 drops instantly.
    pub release_ms: f32,
    /// How long the peak-hold marker stays before it starts to fall. 0 disables it.
    pub peak_hold_ms: f32,
    /// Time constant for the peak-hold marker's fall.
    pub peak_release_ms: f32,
}

impl Default for BallisticsOptions {
    /// The peak preset.
    fn default() -> Self {
        BALLISTICS.peak
    }
}

/// The built-in presets (`BALLISTICS.instant`, `.peak`, `.vu`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallisticsPresets {
    pub instant: BallisticsOptions,
    pub peak: BallisticsOptions,
    pub vu: BallisticsOptions,
}

/// The built-in presets.
pub const BALLISTICS: BallisticsPresets = BallisticsPresets {
    instant: BallisticsOptions {
        attack_ms: 0.0,
        peak_hold_ms: 0.0,
        peak_release_ms: 0.0,
        release_ms: 0.0,
    },
    peak: BallisticsOptions {
        attack_ms: 15.0,
        peak_hold_ms: 1200.0,
        peak_release_ms: 600.0,
        release_ms: 350.0,
    },
    vu: BallisticsOptions {
        attack_ms: 300.0,
        peak_hold_ms: 0.0,
        peak_release_ms: 300.0,
        release_ms: 300.0,
    },
};

/// A preset name (`keyof typeof BALLISTICS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BallisticsPreset {
    Instant,
    #[default]
    Peak,
    Vu,
}

impl BallisticsPreset {
    /// The preset's options.
    pub const fn options(self) -> BallisticsOptions {
        match self {
            Self::Instant => BALLISTICS.instant,
            Self::Peak => BALLISTICS.peak,
            Self::Vu => BALLISTICS.vu,
        }
    }
}

/// `Partial<BallisticsOptions>`: unset fields come from the peak preset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PartialBallisticsOptions {
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub peak_hold_ms: Option<f32>,
    pub peak_release_ms: Option<f32>,
}

/// A preset or partial options. Default: the peak preset.
#[derive(Clone, Debug, PartialEq)]
pub enum BallisticsInput {
    Preset(BallisticsPreset),
    Options(PartialBallisticsOptions),
}

impl Default for BallisticsInput {
    fn default() -> Self {
        Self::Preset(BallisticsPreset::Peak)
    }
}

impl From<BallisticsPreset> for BallisticsInput {
    fn from(preset: BallisticsPreset) -> Self {
        Self::Preset(preset)
    }
}

impl From<PartialBallisticsOptions> for BallisticsInput {
    fn from(options: PartialBallisticsOptions) -> Self {
        Self::Options(options)
    }
}

impl From<BallisticsOptions> for BallisticsInput {
    fn from(o: BallisticsOptions) -> Self {
        Self::Options(PartialBallisticsOptions {
            attack_ms: Some(o.attack_ms),
            release_ms: Some(o.release_ms),
            peak_hold_ms: Some(o.peak_hold_ms),
            peak_release_ms: Some(o.peak_release_ms),
        })
    }
}

/// Resolves a preset or partial options (merged over the peak preset).
/// `None` is the peak preset.
pub fn resolve_ballistics(input: Option<&BallisticsInput>) -> BallisticsOptions {
    match input {
        None => BALLISTICS.peak,
        Some(BallisticsInput::Preset(preset)) => preset.options(),
        Some(BallisticsInput::Options(partial)) => {
            let peak = BALLISTICS.peak;
            BallisticsOptions {
                attack_ms: partial.attack_ms.unwrap_or(peak.attack_ms),
                release_ms: partial.release_ms.unwrap_or(peak.release_ms),
                peak_hold_ms: partial.peak_hold_ms.unwrap_or(peak.peak_hold_ms),
                peak_release_ms: partial.peak_release_ms.unwrap_or(peak.peak_release_ms),
            }
        }
    }
}

/// One step's output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallisticsState {
    /// The smoothed level in dBFS.
    pub db: f32,
    /// The peak-hold level in dBFS.
    pub hold_db: f32,
}

fn smoothing_factor(elapsed_ms: f64, time_constant_ms: f64) -> f64 {
    if time_constant_ms <= 0.0 {
        return 1.0;
    }
    1.0 - (-elapsed_ms / time_constant_ms).exp()
}

/// A meter ballistics integrator. Smoothing runs on linear amplitude, so a
/// falling level drops at a steady rate in dB, like a hardware meter.
#[derive(Clone, Debug, PartialEq)]
pub struct Ballistics {
    options: BallisticsOptions,
    amplitude: f64,
    hold_amplitude: f64,
    hold_until_ms: f64,
    last_ms: Option<f64>,
}

/// Creates a meter ballistics integrator (`None` = peak preset).
pub fn create_ballistics(input: Option<&BallisticsInput>) -> Ballistics {
    Ballistics::new(resolve_ballistics(input))
}

impl Ballistics {
    /// An integrator with resolved options.
    pub fn new(options: BallisticsOptions) -> Self {
        Self {
            options,
            amplitude: 0.0,
            hold_amplitude: 0.0,
            hold_until_ms: 0.0,
            last_ms: None,
        }
    }

    /// The resolved options.
    pub fn options(&self) -> BallisticsOptions {
        self.options
    }

    /// Forgets all state; the next step starts at its reading.
    pub fn reset(&mut self) {
        self.amplitude = 0.0;
        self.hold_amplitude = 0.0;
        self.hold_until_ms = 0.0;
        self.last_ms = None;
    }

    /// Advances to `now_ms` with a new reading in dBFS.
    pub fn step(&mut self, input_db: f32, now_ms: f64) -> BallisticsState {
        let options = &self.options;
        let target = db_to_gain64(input_db as f64);

        match self.last_ms {
            None => self.amplitude = target,
            Some(last_ms) => {
                let elapsed_ms = (now_ms - last_ms).max(0.0);
                let time_constant = if target > self.amplitude {
                    options.attack_ms
                } else {
                    options.release_ms
                };
                self.amplitude +=
                    (target - self.amplitude) * smoothing_factor(elapsed_ms, time_constant as f64);
            }
        }

        let peak_hold_ms = options.peak_hold_ms as f64;
        if peak_hold_ms <= 0.0 {
            self.hold_amplitude = self.amplitude;
        } else if self.amplitude >= self.hold_amplitude {
            self.hold_amplitude = self.amplitude;
            self.hold_until_ms = now_ms + peak_hold_ms;
        } else if let Some(last_ms) = self.last_ms
            && now_ms > self.hold_until_ms
        {
            let elapsed_ms = (now_ms - last_ms.max(self.hold_until_ms)).max(0.0);
            self.hold_amplitude -= (self.hold_amplitude - self.amplitude)
                * smoothing_factor(elapsed_ms, options.peak_release_ms as f64);
        }

        self.last_ms = Some(now_ms);
        BallisticsState {
            db: gain_to_db64(self.amplitude) as f32,
            hold_db: gain_to_db64(self.hold_amplitude) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: f64 = 16.0;

    fn run(b: &mut Ballistics, input_db: f32, from_ms: f64, to_ms: f64) -> BallisticsState {
        let mut state = b.step(input_db, from_ms);
        let mut now = from_ms + FRAME_MS;
        while now <= to_ms {
            state = b.step(input_db, now);
            now += FRAME_MS;
        }
        state
    }

    fn preset(p: BallisticsPreset) -> Ballistics {
        create_ballistics(Some(&p.into()))
    }

    #[test]
    fn starts_at_the_first_reading() {
        let mut b = create_ballistics(None);
        assert!((b.step(-12.0, 0.0).db - -12.0).abs() < 1e-4);
    }

    #[test]
    fn rises_quickly_with_peak() {
        let mut b = preset(BallisticsPreset::Peak);
        b.step(f32::NEG_INFINITY, 0.0);
        let state = run(&mut b, -6.0, FRAME_MS, 100.0);
        assert!(state.db > -6.5);
    }

    #[test]
    fn falls_at_a_steady_rate_in_db() {
        let mut b = preset(BallisticsPreset::Peak);
        run(&mut b, 0.0, 0.0, 100.0);
        let half = run(&mut b, f32::NEG_INFINITY, 116.0, 600.0);
        let one = run(&mut b, f32::NEG_INFINITY, 616.0, 1100.0);
        let first_drop = 0.0 - half.db;
        let second_drop = half.db - one.db;
        assert!(first_drop > 5.0);
        // toBeCloseTo(x, -1): |a - b| < 5
        assert!((second_drop - first_drop).abs() < 5.0);
    }

    #[test]
    fn holds_the_peak_before_falling() {
        let mut b = preset(BallisticsPreset::Peak);
        run(&mut b, -3.0, 0.0, 50.0);
        let held = run(&mut b, -40.0, 66.0, 1000.0);
        assert!((held.hold_db - -3.0).abs() < 0.05);
        let released = run(&mut b, -40.0, 1016.0, 4000.0);
        assert!(released.hold_db < -30.0);
    }

    #[test]
    fn instant_follows_exactly() {
        let mut b = preset(BallisticsPreset::Instant);
        b.step(-10.0, 0.0);
        assert!((b.step(-40.0, 16.0).db - -40.0).abs() < 1e-4);
        assert!((b.step(-40.0, 32.0).hold_db - -40.0).abs() < 1e-4);
    }

    #[test]
    fn forgets_on_reset() {
        let mut b = preset(BallisticsPreset::Peak);
        run(&mut b, 0.0, 0.0, 100.0);
        b.reset();
        assert!((b.step(-30.0, 200.0).db - -30.0).abs() < 1e-4);
    }

    #[test]
    fn merges_partial_over_peak() {
        let input = PartialBallisticsOptions {
            release_ms: Some(100.0),
            ..Default::default()
        }
        .into();
        assert_eq!(
            resolve_ballistics(Some(&input)),
            BallisticsOptions {
                attack_ms: 15.0,
                peak_hold_ms: 1200.0,
                peak_release_ms: 600.0,
                release_ms: 100.0,
            }
        );
    }
}
