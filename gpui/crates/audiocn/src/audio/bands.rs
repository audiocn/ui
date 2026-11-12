//! Port of `lib/audio/bands.ts`.
//!
//! Spectrum → frequency bands, and ring-buffer resampling. Buffers are `f32`
//! slices (TS `Float32Array`); functions write into `out` like the TS.

use crate::audio::decibels::clamp64;

/// `AnalyserNode.minDecibels` default.
pub const SPECTRUM_MIN_DB: f32 = -100.0;
/// `AnalyserNode.maxDecibels` default.
pub const SPECTRUM_MAX_DB: f32 = -30.0;

const POWER_DB_FACTOR: f64 = 10.0;

/// `count + 1` band edges in Hz, spaced evenly on a log scale.
pub fn log_band_edges(count: usize, min_hz: f32, max_hz: f32) -> Vec<f32> {
    let ratio = max_hz as f64 / min_hz as f64;
    (0..=count)
        .map(|index| (min_hz as f64 * ratio.powf(index as f64 / count as f64)) as f32)
        .collect()
}

/// Options for [`bands_from_spectrum`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandsFromSpectrumOptions {
    /// Default [`SPECTRUM_MIN_DB`].
    pub min_db: f32,
    /// Default [`SPECTRUM_MAX_DB`].
    pub max_db: f32,
}

impl Default for BandsFromSpectrumOptions {
    fn default() -> Self {
        Self {
            min_db: SPECTRUM_MIN_DB,
            max_db: SPECTRUM_MAX_DB,
        }
    }
}

/// Reduces analyser frequency data (dB per bin) to 0..1 bands between the
/// given edges, writing `min(out.len(), edges.len() - 1)` bands into `out`.
pub fn bands_from_spectrum(
    spectrum_db: &[f32],
    sample_rate: f32,
    edges: &[f32],
    out: &mut [f32],
    options: BandsFromSpectrumOptions,
) {
    let min_db = options.min_db as f64;
    let max_db = options.max_db as f64;
    let bin_count = spectrum_db.len() as i64;
    let hz_per_bin = sample_rate as f64 / 2.0 / bin_count as f64;
    let band_count = out.len().min(edges.len().saturating_sub(1));
    let edge = |index: usize| edges.get(index).copied().unwrap_or(0.0) as f64;

    for (band, slot) in out.iter_mut().enumerate().take(band_count) {
        let low = (edge(band) / hz_per_bin).floor() as i64;
        let high = (low + 1).max((edge(band + 1) / hz_per_bin).ceil() as i64);
        // clamp(low, 0, binCount - 1), clamp(high, first + 1, binCount), JS-style.
        let first = (bin_count - 1).min(low.max(0));
        let last = bin_count.min(high.max(first + 1));
        let mut power = 0.0f64;
        for bin in first..last {
            let db = if bin >= 0 {
                spectrum_db.get(bin as usize).map_or(min_db, |&v| v as f64)
            } else {
                min_db
            };
            power += 10f64.powf(db / POWER_DB_FACTOR);
        }
        let mean_db = POWER_DB_FACTOR * (power / (last - first) as f64).log10();
        *slot = clamp64((mean_db - min_db) / (max_db - min_db), 0.0, 1.0) as f32;
    }
}

/// Fits `length` values from a ring buffer, starting at `start`, into
/// `out.len()` values. Each output takes the loudest value in its span.
/// Empty input clears `out` to 0.
pub fn resample_levels(source: &[f32], start: usize, length: usize, out: &mut [f32]) {
    let size = source.len();
    let count = out.len();

    if size == 0 || length == 0 || count == 0 {
        out.fill(0.0);
        return;
    }

    for (index, slot) in out.iter_mut().enumerate() {
        let from = (index * length) / count;
        let to = (from + 1).max(((index + 1) * length) / count);
        let mut loudest = 0.0f32;
        for offset in from..to {
            let value = source[(start + offset) % size];
            if value > loudest {
                loudest = value;
            }
        }
        *slot = loudest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, digits: i32) -> bool {
        ((a - b).abs() as f64) < 10f64.powi(-digits) / 2.0
    }

    #[test]
    fn edges_cover_the_range() {
        let edges = log_band_edges(10, 20.0, 20_000.0);
        assert_eq!(edges.len(), 11);
        assert!(close(edges[0], 20.0, 3));
        assert!(close(edges[10], 20_000.0, 0));
        for i in 1..edges.len() {
            assert!(edges[i] > edges[i - 1]);
        }
    }

    #[test]
    fn edges_have_constant_ratio() {
        let edges = log_band_edges(3, 100.0, 100_000.0);
        assert!(close(edges[1], 1000.0, 0));
        assert!(close(edges[2], 10_000.0, 0));
    }

    #[test]
    fn maps_loud_bins_to_one_and_quiet_to_zero() {
        let mut spectrum = vec![-120.0f32; 1024];
        for bin in spectrum.iter_mut().take(20) {
            *bin = -20.0;
        }
        let edges = log_band_edges(4, 40.0, 16_000.0);
        let mut bands = [0.0f32; 4];
        bands_from_spectrum(&spectrum, 48_000.0, &edges, &mut bands, Default::default());
        assert_eq!(bands[0], 1.0);
        assert_eq!(bands[3], 0.0);
    }

    #[test]
    fn handles_silent_and_empty_spectra() {
        let edges = log_band_edges(2, 40.0, 16_000.0);
        let mut bands = [1.0f32; 2];
        bands_from_spectrum(
            &[f32::NEG_INFINITY; 64],
            48_000.0,
            &edges,
            &mut bands,
            Default::default(),
        );
        assert_eq!(bands, [0.0, 0.0]);
        bands_from_spectrum(&[], 48_000.0, &edges, &mut bands, Default::default());
        assert_eq!(bands, [0.0, 0.0]);
    }

    #[test]
    fn resample_takes_loudest() {
        let mut out = [0.0f32; 2];
        resample_levels(&[0.1, 0.9, 0.2, 0.4], 0, 4, &mut out);
        assert!(close(out[0], 0.9, 5) && close(out[1], 0.4, 5));
    }

    #[test]
    fn resample_reads_ring_from_start() {
        let mut out = [0.0f32; 3];
        resample_levels(&[0.5, 0.1, 0.3], 1, 3, &mut out);
        assert!(close(out[0], 0.1, 5) && close(out[1], 0.3, 5) && close(out[2], 0.5, 5));
    }

    #[test]
    fn resample_repeats_values() {
        let mut out = [0.0f32; 4];
        resample_levels(&[0.2, 0.8], 0, 2, &mut out);
        assert_eq!(out, [0.2, 0.2, 0.8, 0.8]);
    }

    #[test]
    fn resample_clears_for_empty_input() {
        let mut out = [1.0f32, 1.0];
        resample_levels(&[], 0, 0, &mut out);
        assert_eq!(out, [0.0, 0.0]);
    }
}
