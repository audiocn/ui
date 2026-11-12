//! Port of `lib/audio/decibels.ts`.
//!
//! Decibel maths and formatting. Values are `f32`; maths runs in `f64` like
//! JavaScript, so results match the React version.
//! [`clamp`] has JavaScript `Math.min(max, Math.max(min, v))` semantics: it
//! never panics (unlike `f32::clamp` when `min > max`) and passes NaN through.

/// Default bottom of a meter's range, in dBFS.
pub const DEFAULT_MIN_DB: f32 = -60.0;
/// Default top of a meter's range, in dBFS.
pub const DEFAULT_MAX_DB: f32 = 0.0;
/// Digital silence.
pub const SILENCE_DB: f32 = f32::NEG_INFINITY;

const AMPLITUDE_DB_FACTOR: f64 = 20.0;
const MINUS_SIGN: &str = "\u{2212}";
const INFINITY_SIGN: &str = "\u{221e}";

/// `Math.max` semantics: NaN if either side is NaN.
pub(crate) fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// `Math.min` semantics: NaN if either side is NaN.
pub(crate) fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// [`clamp`] on `f64`, for internal maths.
pub(crate) fn clamp64(value: f64, min: f64, max: f64) -> f64 {
    js_min(max, js_max(min, value))
}

/// `Math.min(max, Math.max(min, value))`. Never panics; NaN stays NaN, and
/// `max` wins when `min > max`.
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    clamp64(value as f64, min as f64, max as f64) as f32
}

/// Decibels to linear gain. `-Infinity` gives 0.
pub fn db_to_gain(db: f32) -> f32 {
    db_to_gain64(db as f64) as f32
}

pub(crate) fn db_to_gain64(db: f64) -> f64 {
    if db == f64::NEG_INFINITY {
        return 0.0;
    }
    10f64.powf(db / AMPLITUDE_DB_FACTOR)
}

/// Linear gain to decibels. 0 (or less) gives `-Infinity`.
pub fn gain_to_db(gain: f32) -> f32 {
    gain_to_db64(gain as f64) as f32
}

pub(crate) fn gain_to_db64(gain: f64) -> f64 {
    if gain <= 0.0 {
        return f64::NEG_INFINITY;
    }
    AMPLITUDE_DB_FACTOR * gain.log10()
}

/// dBFS to a 0..1 level, linear in dB over `min_db..max_db`, clamped.
/// Silence and NaN give 0. (TS defaults: [`DEFAULT_MIN_DB`], [`DEFAULT_MAX_DB`].)
pub fn db_to_level(db: f32, min_db: f32, max_db: f32) -> f32 {
    if db.is_nan() || db <= min_db {
        return 0.0;
    }
    let (db, min, max) = (db as f64, min_db as f64, max_db as f64);
    clamp64((db - min) / (max - min), 0.0, 1.0) as f32
}

/// A 0..1 level back to dBFS over `min_db..max_db`. The inverse of [`db_to_level`].
pub fn level_to_db(level: f32, min_db: f32, max_db: f32) -> f32 {
    let (min, max) = (min_db as f64, max_db as f64);
    (min + clamp64(level as f64, 0.0, 1.0) * (max - min)) as f32
}

/// Clamps a dB value to a range. With `allow_silence`, `-Infinity` passes
/// through instead of becoming `min_db`. NaN becomes `min_db`.
pub fn clamp_db(db: f32, min_db: f32, max_db: f32, allow_silence: bool) -> f32 {
    if db == SILENCE_DB && allow_silence {
        return SILENCE_DB;
    }
    if db.is_nan() {
        return min_db;
    }
    clamp(db, min_db, max_db)
}

/// Sign style for [`format_db`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FormatDbSign {
    /// "+" for positive values and "−" for negative ones.
    #[default]
    Auto,
    /// Only "−".
    Negative,
    /// No sign.
    Never,
}

/// Options for [`format_db`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormatDbOptions {
    /// Digits after the decimal point. Default 1.
    pub decimals: usize,
    /// Append " dB". Default true.
    pub unit: bool,
    /// Default [`FormatDbSign::Auto`].
    pub sign: FormatDbSign,
    /// At or below this value, show "−∞". Default `-Infinity`.
    pub floor_db: f32,
}

impl Default for FormatDbOptions {
    fn default() -> Self {
        Self {
            decimals: 1,
            unit: true,
            sign: FormatDbSign::Auto,
            floor_db: SILENCE_DB,
        }
    }
}

/// JavaScript `Math.abs(x).toFixed(decimals)` as (digits, rounded-to-zero).
/// Ties round away from zero like `toFixed`. Exact for f32 input up to 6 decimals.
fn to_fixed_abs(value: f32, decimals: usize) -> (String, bool) {
    let magnitude = (value as f64).abs();
    if decimals <= 6 {
        let scale = 10f64.powi(decimals as i32);
        // Exact: a 24-bit mantissa times 10^d (< 2^20) fits in 53 bits.
        let scaled = magnitude * scale;
        if scaled < 9.0e15 {
            let n = (scaled + 0.5).floor() as u64;
            let p = 10u64.pow(decimals as u32);
            let text = if decimals == 0 {
                format!("{n}")
            } else {
                format!("{}.{:0width$}", n / p, n % p, width = decimals)
            };
            return (text, n == 0);
        }
    }
    let text = format!("{magnitude:.decimals$}");
    let zero = text.chars().all(|c| c == '0' || c == '.');
    (text, zero)
}

/// Formats decibels for display: "−12.3 dB", "+3.0 dB", "−∞ dB".
/// NaN (no reading yet) gives "-- dB" (two hyphens, never a dash).
pub fn format_db(db: f32, options: FormatDbOptions) -> String {
    let FormatDbOptions {
        decimals,
        unit,
        sign,
        floor_db,
    } = options;
    let suffix = if unit { " dB" } else { "" };

    if db.is_nan() {
        return format!("--{suffix}");
    }
    if db == SILENCE_DB || db <= floor_db {
        let prefix = if sign == FormatDbSign::Never {
            ""
        } else {
            MINUS_SIGN
        };
        return format!("{prefix}{INFINITY_SIGN}{suffix}");
    }

    let (magnitude, is_zero) = to_fixed_abs(db, decimals);
    if is_zero || sign == FormatDbSign::Never {
        return format!("{magnitude}{suffix}");
    }
    if db < 0.0 {
        return format!("{MINUS_SIGN}{magnitude}{suffix}");
    }
    let plus = if sign == FormatDbSign::Auto { "+" } else { "" };
    format!("{plus}{magnitude}{suffix}")
}

/// Sample peak of a block of samples, in dBFS.
pub fn peak_db(samples: &[f32]) -> f32 {
    let mut peak = 0.0f32;
    for sample in samples {
        let magnitude = sample.abs();
        if magnitude > peak {
            peak = magnitude;
        }
    }
    gain_to_db(peak)
}

/// RMS level of a block of samples, in dBFS. Empty input is silence.
pub fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return SILENCE_DB;
    }
    let sum_of_squares: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    gain_to_db64((sum_of_squares / samples.len() as f64).sqrt()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, digits: i32) -> bool {
        (a - b).abs() < 10f32.powi(-digits) / 2.0
    }

    fn opts() -> FormatDbOptions {
        FormatDbOptions::default()
    }

    #[test]
    fn round_trips_common_values() {
        for db in [-60.0, -20.0, -6.0, 0.0, 6.0] {
            assert!(close(gain_to_db(db_to_gain(db)), db, 4));
        }
    }

    #[test]
    fn maps_silence_both_ways() {
        assert_eq!(db_to_gain(f32::NEG_INFINITY), 0.0);
        assert_eq!(gain_to_db(0.0), f32::NEG_INFINITY);
        assert_eq!(gain_to_db(-1.0), f32::NEG_INFINITY);
    }

    #[test]
    fn minus_six_is_about_half_gain() {
        assert!(close(db_to_gain(-6.0), 0.501, 3));
    }

    #[test]
    fn level_is_linear_in_db() {
        assert_eq!(db_to_level(-60.0, DEFAULT_MIN_DB, DEFAULT_MAX_DB), 0.0);
        assert_eq!(db_to_level(-30.0, DEFAULT_MIN_DB, DEFAULT_MAX_DB), 0.5);
        assert_eq!(db_to_level(0.0, DEFAULT_MIN_DB, DEFAULT_MAX_DB), 1.0);
    }

    #[test]
    fn level_clamps_and_handles_silence() {
        let l = |db| db_to_level(db, DEFAULT_MIN_DB, DEFAULT_MAX_DB);
        assert_eq!(l(-90.0), 0.0);
        assert_eq!(l(6.0), 1.0);
        assert_eq!(l(f32::NEG_INFINITY), 0.0);
        assert_eq!(l(f32::NAN), 0.0);
    }

    #[test]
    fn level_to_db_inverts() {
        for db in [-48.0, -24.0, -12.0, -3.0] {
            let level = db_to_level(db, DEFAULT_MIN_DB, DEFAULT_MAX_DB);
            assert!(close(
                level_to_db(level, DEFAULT_MIN_DB, DEFAULT_MAX_DB),
                db,
                4
            ));
        }
        assert_eq!(level_to_db(0.5, -40.0, 0.0), -20.0);
    }

    #[test]
    fn clamp_db_clamps_and_keeps_silence_when_allowed() {
        assert_eq!(clamp_db(-80.0, -60.0, 6.0, false), -60.0);
        assert_eq!(clamp_db(12.0, -60.0, 6.0, false), 6.0);
        assert_eq!(clamp_db(f32::NEG_INFINITY, -60.0, 6.0, false), -60.0);
        assert_eq!(
            clamp_db(f32::NEG_INFINITY, -60.0, 6.0, true),
            f32::NEG_INFINITY
        );
        assert_eq!(clamp_db(f32::NAN, -60.0, 6.0, false), -60.0);
    }

    #[test]
    fn clamp_matches_javascript() {
        assert!(clamp(f32::NAN, 0.0, 1.0).is_nan());
        assert_eq!(clamp(0.5, 1.0, 0.0), 0.0);
    }

    #[test]
    fn formats_values() {
        assert_eq!(format_db(-12.34, opts()), "−12.3 dB");
        assert_eq!(format_db(3.0, opts()), "+3.0 dB");
        assert_eq!(format_db(0.0, opts()), "0.0 dB");
        assert_eq!(format_db(-0.04, opts()), "0.0 dB");
        assert_eq!(format_db(0.25, opts()), "+0.3 dB");
        assert_eq!(format_db(-0.25, opts()), "−0.3 dB");
    }

    #[test]
    fn formats_no_reading_with_hyphens() {
        assert_eq!(format_db(f32::NAN, opts()), "-- dB");
        let no_unit = FormatDbOptions {
            unit: false,
            ..opts()
        };
        assert_eq!(format_db(f32::NAN, no_unit), "--");
        assert!(!format_db(f32::NAN, opts()).contains('\u{2014}'));
    }

    #[test]
    fn formats_silence_and_floor() {
        assert_eq!(format_db(f32::NEG_INFINITY, opts()), "−∞ dB");
        let floor = FormatDbOptions {
            floor_db: -60.0,
            ..opts()
        };
        assert_eq!(format_db(-70.0, floor), "−∞ dB");
    }

    #[test]
    fn respects_options() {
        let o = FormatDbOptions {
            decimals: 0,
            unit: false,
            ..opts()
        };
        assert_eq!(format_db(-6.0, o), "−6");
        let neg = FormatDbOptions {
            sign: FormatDbSign::Negative,
            ..opts()
        };
        assert_eq!(format_db(6.0, neg), "6.0 dB");
        let never = FormatDbOptions {
            sign: FormatDbSign::Never,
            ..opts()
        };
        assert_eq!(format_db(-6.0, never), "6.0 dB");
    }

    #[test]
    fn square_wave_is_zero_dbfs() {
        let square: Vec<f32> = (0..64)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        assert!(close(peak_db(&square), 0.0, 6));
        assert!(close(rms_db(&square), 0.0, 6));
    }

    #[test]
    fn sine_rms_is_three_db_below_peak() {
        let sine: Vec<f32> = (0..4800)
            .map(|i| ((i as f64 / 48.0) * std::f64::consts::PI * 2.0).sin() as f32)
            .collect();
        assert!(close(peak_db(&sine), 0.0, 2));
        assert!(close(rms_db(&sine), -3.01, 1));
    }

    #[test]
    fn empty_or_silent_is_silence() {
        assert_eq!(peak_db(&[0.0; 8]), f32::NEG_INFINITY);
        assert_eq!(rms_db(&[]), f32::NEG_INFINITY);
    }
}
