//! Port of `lib/audio/taper.ts`.
//!
//! # How to use tapers in components
//!
//! - A component prop `taper?: TaperInput` (`"linear" | "audio" | "log" | Taper`)
//!   becomes a builder method taking [`TaperInput`]:
//!   `.taper(TaperInput::Audio)`, or any concrete taper via `From`:
//!   `.taper(log_taper(20.0, 20_000.0))`. Store it as `Option<TaperInput>`
//!   (it is `Clone + Debug + PartialEq`).
//!   ```ignore
//!   pub fn taper(mut self, taper: impl Into<TaperInput>) -> Self { self.taper = Some(taper.into()); self }
//!   ```
//! - At render, resolve it for the range with [`resolve_taper`], which returns
//!   an [`AnyTaper`]: a cheap `Clone` enum (no allocation for built-ins) that
//!   implements [`Taper`], so call `to_position` / `to_value` on it directly.
//! - Concrete tapers ([`LinearTaper`], [`AudioTaper`], [`LogTaper`]) are
//!   `Copy` and implement [`Taper`] too. Custom laws: implement [`Taper`] on your
//!   own type (`TaperInput::from(my_taper)`) or use [`TaperInput::custom`] with
//!   two closures.

use std::fmt;
use std::sync::Arc;

use crate::audio::decibels::{DEFAULT_MIN_DB, clamp64};
pub use crate::audio::types::Taper;

const DEFAULT_FADER_MAX_DB: f32 = 6.0;
const DEFAULT_UNITY_POSITION: f32 = 0.75;
const AUDIO_CURVE_EXPONENT: f64 = 2.0;

/// Equal distance per unit across the range. Below `min` (and NaN) is position 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearTaper {
    pub min: f32,
    pub max: f32,
}

/// Equal distance per unit across the range.
pub fn linear_taper(min: f32, max: f32) -> LinearTaper {
    LinearTaper { min, max }
}

impl Taper for LinearTaper {
    fn to_position(&self, value: f32) -> f32 {
        if value.is_nan() || value <= self.min {
            return 0.0;
        }
        let (min, max) = (self.min as f64, self.max as f64);
        clamp64((value as f64 - min) / (max - min), 0.0, 1.0) as f32
    }

    fn to_value(&self, position: f32) -> f32 {
        let (min, max) = (self.min as f64, self.max as f64);
        (min + clamp64(position as f64, 0.0, 1.0) * (max - min)) as f32
    }
}

/// Options for [`audio_taper`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioTaperOptions {
    /// Default [`DEFAULT_MIN_DB`] (−60).
    pub min_db: f32,
    /// Default 6.
    pub max_db: f32,
    /// Where 0 dB sits on the travel, 0..1. Default 0.75.
    pub unity_position: f32,
}

impl Default for AudioTaperOptions {
    fn default() -> Self {
        Self {
            min_db: DEFAULT_MIN_DB,
            max_db: DEFAULT_FADER_MAX_DB,
            unity_position: DEFAULT_UNITY_POSITION,
        }
    }
}

/// A console-style fader law: more travel around 0 dB, a fast fall to the
/// bottom of the range, and linear headroom above 0 dB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioTaper {
    min_db: f64,
    max_db: f64,
    anchor_db: f64,
    unity: f64,
}

/// A console-style fader law (see [`AudioTaper`]).
pub fn audio_taper(options: AudioTaperOptions) -> AudioTaper {
    let AudioTaperOptions {
        min_db,
        max_db,
        unity_position,
    } = options;
    let (min_db, max_db) = (min_db as f64, max_db as f64);
    let anchor_db = 0f64.min(max_db);
    let unity = if max_db > 0.0 {
        clamp64(unity_position as f64, 0.0, 1.0)
    } else {
        1.0
    };
    AudioTaper {
        min_db,
        max_db,
        anchor_db,
        unity,
    }
}

impl Default for AudioTaper {
    fn default() -> Self {
        audio_taper(AudioTaperOptions::default())
    }
}

impl Taper for AudioTaper {
    fn to_position(&self, db: f32) -> f32 {
        let &AudioTaper {
            min_db,
            max_db,
            anchor_db,
            unity,
        } = self;
        let db = db as f64;
        if db.is_nan() || db <= min_db {
            return 0.0;
        }
        if db >= max_db {
            return 1.0;
        }
        if db >= anchor_db {
            return (unity + ((db - anchor_db) / (max_db - anchor_db)) * (1.0 - unity)) as f32;
        }
        let ratio = (db - anchor_db) / (min_db - anchor_db);
        let travel = 1.0 - ratio.powf(1.0 / AUDIO_CURVE_EXPONENT);
        (travel * unity) as f32
    }

    fn to_value(&self, position: f32) -> f32 {
        let &AudioTaper {
            min_db,
            max_db,
            anchor_db,
            unity,
        } = self;
        let clamped = clamp64(position as f64, 0.0, 1.0);
        if clamped >= unity {
            if unity >= 1.0 {
                return anchor_db as f32;
            }
            return (anchor_db + ((clamped - unity) / (1.0 - unity)) * (max_db - anchor_db)) as f32;
        }
        let travel = if unity > 0.0 { clamped / unity } else { 0.0 };
        (anchor_db + (min_db - anchor_db) * (1.0 - travel).powf(AUDIO_CURVE_EXPONENT)) as f32
    }
}

/// Logarithmic law for frequency and time parameters. `min` must be above 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogTaper {
    min: f64,
    span: f64,
}

/// Logarithmic law for frequency and time parameters. `min` must be above 0.
pub fn log_taper(min: f32, max: f32) -> LogTaper {
    LogTaper {
        min: min as f64,
        span: (max as f64 / min as f64).ln(),
    }
}

impl Taper for LogTaper {
    fn to_position(&self, value: f32) -> f32 {
        let value = value as f64;
        if value.is_nan() || value <= self.min {
            return 0.0;
        }
        clamp64((value / self.min).ln() / self.span, 0.0, 1.0) as f32
    }

    fn to_value(&self, position: f32) -> f32 {
        (self.min * (clamp64(position as f64, 0.0, 1.0) * self.span).exp()) as f32
    }
}

struct FnTaper<P, V> {
    to_position: P,
    to_value: V,
}

impl<P, V> Taper for FnTaper<P, V>
where
    P: Fn(f32) -> f32 + Send + Sync,
    V: Fn(f32) -> f32 + Send + Sync,
{
    fn to_position(&self, value: f32) -> f32 {
        (self.to_position)(value)
    }

    fn to_value(&self, position: f32) -> f32 {
        (self.to_value)(position)
    }
}

/// A taper name, or a taper (`"linear" | "audio" | "log" | Taper`).
/// Custom tapers compare by identity.
#[derive(Clone)]
pub enum TaperInput {
    Linear,
    Audio,
    Log,
    Custom(Arc<dyn Taper>),
}

impl TaperInput {
    /// A custom taper from two closures.
    pub fn custom(
        to_position: impl Fn(f32) -> f32 + Send + Sync + 'static,
        to_value: impl Fn(f32) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self::Custom(Arc::new(FnTaper {
            to_position,
            to_value,
        }))
    }
}

impl<T: Taper + 'static> From<T> for TaperInput {
    fn from(taper: T) -> Self {
        Self::Custom(Arc::new(taper))
    }
}

impl From<Arc<dyn Taper>> for TaperInput {
    fn from(taper: Arc<dyn Taper>) -> Self {
        Self::Custom(taper)
    }
}

impl fmt::Debug for TaperInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear => f.write_str("Linear"),
            Self::Audio => f.write_str("Audio"),
            Self::Log => f.write_str("Log"),
            Self::Custom(taper) => write!(f, "Custom({:p})", Arc::as_ptr(taper)),
        }
    }
}

impl PartialEq for TaperInput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear, Self::Linear) | (Self::Audio, Self::Audio) | (Self::Log, Self::Log) => {
                true
            }
            (Self::Custom(a), Self::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// A resolved taper: one of the built-ins, or a custom one. Implements
/// [`Taper`]; cheap to clone.
#[derive(Clone)]
pub enum AnyTaper {
    Linear(LinearTaper),
    Audio(AudioTaper),
    Log(LogTaper),
    Custom(Arc<dyn Taper>),
}

impl Taper for AnyTaper {
    fn to_position(&self, value: f32) -> f32 {
        match self {
            Self::Linear(t) => t.to_position(value),
            Self::Audio(t) => t.to_position(value),
            Self::Log(t) => t.to_position(value),
            Self::Custom(t) => t.to_position(value),
        }
    }

    fn to_value(&self, position: f32) -> f32 {
        match self {
            Self::Linear(t) => t.to_value(position),
            Self::Audio(t) => t.to_value(position),
            Self::Log(t) => t.to_value(position),
            Self::Custom(t) => t.to_value(position),
        }
    }
}

impl fmt::Debug for AnyTaper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear(t) => t.fmt(f),
            Self::Audio(t) => t.fmt(f),
            Self::Log(t) => t.fmt(f),
            Self::Custom(t) => write!(f, "Custom({:p})", Arc::as_ptr(t)),
        }
    }
}

impl PartialEq for AnyTaper {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear(a), Self::Linear(b)) => a == b,
            (Self::Audio(a), Self::Audio(b)) => a == b,
            (Self::Log(a), Self::Log(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Resolves a taper name or object for a range. `Audio` uses
/// `audio_taper({ min_db: min, max_db: max })` (unity at 0.75); custom tapers
/// pass through unchanged.
pub fn resolve_taper(taper: &TaperInput, min: f32, max: f32) -> AnyTaper {
    match taper {
        TaperInput::Custom(taper) => AnyTaper::Custom(taper.clone()),
        TaperInput::Audio => AnyTaper::Audio(audio_taper(AudioTaperOptions {
            min_db: min,
            max_db: max,
            ..Default::default()
        })),
        TaperInput::Log => AnyTaper::Log(log_taper(min, max)),
        TaperInput::Linear => AnyTaper::Linear(linear_taper(min, max)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIONS: [f32; 8] = [0.0, 0.1, 0.25, 0.5, 0.74, 0.75, 0.9, 1.0];

    fn close(a: f32, b: f32, digits: i32) -> bool {
        ((a - b).abs() as f64) < 10f64.powi(-digits) / 2.0
    }

    #[test]
    fn linear_maps_evenly_and_round_trips() {
        let taper = linear_taper(-60.0, 6.0);
        assert_eq!(taper.to_position(-60.0), 0.0);
        assert_eq!(taper.to_position(6.0), 1.0);
        for p in POSITIONS {
            assert!(close(taper.to_position(taper.to_value(p)), p, 5));
        }
    }

    #[test]
    fn linear_puts_silence_at_the_bottom() {
        assert_eq!(linear_taper(-60.0, 6.0).to_position(f32::NEG_INFINITY), 0.0);
        assert_eq!(linear_taper(-60.0, 6.0).to_position(f32::NAN), 0.0);
    }

    fn fader() -> AudioTaper {
        audio_taper(AudioTaperOptions {
            max_db: 6.0,
            min_db: -60.0,
            unity_position: 0.75,
        })
    }

    #[test]
    fn audio_puts_unity_at_its_position() {
        let taper = fader();
        assert!(close(taper.to_position(0.0), 0.75, 6));
        assert!(close(taper.to_value(0.75), 0.0, 6));
    }

    #[test]
    fn audio_covers_the_full_range() {
        let taper = fader();
        assert!(close(taper.to_value(0.0), -60.0, 5));
        assert!(close(taper.to_value(1.0), 6.0, 5));
    }

    #[test]
    fn audio_is_monotonic_and_invertible() {
        let taper = fader();
        let mut previous = f32::NEG_INFINITY;
        for p in POSITIONS {
            let value = taper.to_value(p);
            assert!(value > previous);
            assert!(close(taper.to_position(value), p, 5));
            previous = value;
        }
    }

    #[test]
    fn audio_gives_more_travel_near_unity() {
        let taper = fader();
        let near_unity = taper.to_position(0.0) - taper.to_position(-6.0);
        let near_bottom = taper.to_position(-54.0) - taper.to_position(-60.0);
        assert!(near_unity > near_bottom);
    }

    #[test]
    fn audio_without_headroom_uses_whole_travel() {
        let taper = audio_taper(AudioTaperOptions {
            max_db: 0.0,
            min_db: -60.0,
            ..Default::default()
        });
        assert!(close(taper.to_value(1.0), 0.0, 6));
        assert_eq!(taper.to_position(0.0), 1.0);
    }

    #[test]
    fn log_spaces_decades_evenly() {
        let taper = log_taper(20.0, 20_000.0);
        assert!(close(taper.to_position(200.0), 1.0 / 3.0, 5));
        assert!(close(taper.to_position(2000.0), 2.0 / 3.0, 5));
        assert!(close(taper.to_value(0.5), 632.46, 1));
    }

    #[test]
    fn resolves_names_and_passes_objects_through() {
        let custom: TaperInput = linear_taper(0.0, 1.0).into();
        let resolved = resolve_taper(&custom, -60.0, 6.0);
        match (&resolved, &custom) {
            (AnyTaper::Custom(a), TaperInput::Custom(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!("custom taper was not passed through"),
        }
        assert!(close(
            resolve_taper(&TaperInput::Linear, -60.0, 6.0).to_position(-27.0),
            0.5,
            6
        ));
        assert!(close(
            resolve_taper(&TaperInput::Audio, -60.0, 6.0).to_position(0.0),
            0.75,
            6
        ));
        let closure = TaperInput::custom(|v| v / 2.0, |p| p * 2.0);
        assert_eq!(resolve_taper(&closure, 0.0, 1.0).to_value(0.25), 0.5);
        assert_eq!(closure.clone(), closure);
    }
}
