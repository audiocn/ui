//! Port of `hooks/use-waveform-data.ts`: the maths that reduces decoded
//! audio to waveform peaks for the static `Waveform` component.
//!
//! Decoding files is not in scope here (that is `io`, behind the `device`
//! feature); pass decoded channel samples to [`compute_peaks`] or
//! [`waveform_data_from_samples`].

use gpui::SharedString;

/// Default number of peaks (`UseWaveformDataOptions.samples`).
pub const DEFAULT_PEAK_COUNT: usize = 512;

/// Where waveform data is in its life (`WaveformDataStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WaveformDataStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

/// Peaks plus load status (`WaveformData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveformData {
    /// Loudest absolute sample per bucket, normalised so the loudest is 1.
    pub peaks: Option<Vec<f32>>,
    /// Duration in seconds.
    pub duration: f64,
    pub status: WaveformDataStatus,
    pub error: Option<SharedString>,
}

impl WaveformData {
    /// No source (`IDLE`).
    pub fn idle() -> Self {
        Self::default()
    }

    /// Decoding (`LOADING`).
    pub fn loading() -> Self {
        Self {
            status: WaveformDataStatus::Loading,
            ..Default::default()
        }
    }

    /// Decoding failed.
    pub fn error(message: impl Into<SharedString>) -> Self {
        Self {
            status: WaveformDataStatus::Error,
            error: Some(message.into()),
            ..Default::default()
        }
    }
}

fn loudest_in_range(data: &[f32], start: usize, end: usize) -> f32 {
    let mut peak = 0.0f32;
    for sample in data.iter().take(end).skip(start) {
        let magnitude = sample.abs();
        if magnitude > peak {
            peak = magnitude;
        }
    }
    peak
}

/// Reduces decoded audio (one slice per channel, `length` frames each) to
/// `samples` peaks, 0..1 (`computePeaks`). Each peak is the loudest absolute
/// sample of its bucket across all channels; the result is normalised so the
/// loudest peak is 1 (silence stays 0). Buckets are
/// `max(1, floor(length / samples))` frames from the start, so a short
/// buffer leaves trailing peaks at 0, exactly like the TS.
pub fn compute_peaks<C: AsRef<[f32]>>(channels: &[C], samples: usize) -> Vec<f32> {
    let mut peaks = vec![0.0f32; samples];
    let length = channels
        .iter()
        .map(|channel| channel.as_ref().len())
        .max()
        .unwrap_or(0);
    let bucket = (length / samples.max(1)).max(1);
    let mut loudest = 0.0f32;
    for channel in channels {
        let data = channel.as_ref();
        for (index, peak) in peaks.iter_mut().enumerate() {
            let start = index * bucket;
            let value = peak.max(loudest_in_range(
                data,
                start,
                data.len().min(start + bucket),
            ));
            *peak = value;
            loudest = loudest.max(value);
        }
    }
    if loudest > 0.0 {
        for peak in &mut peaks {
            *peak /= loudest;
        }
    }
    peaks
}

/// Ready waveform data for decoded audio: peaks from [`compute_peaks`] and
/// the duration from the frame count and sample rate.
pub fn waveform_data_from_samples<C: AsRef<[f32]>>(
    channels: &[C],
    sample_rate: f32,
    samples: usize,
) -> WaveformData {
    let length = channels
        .iter()
        .map(|channel| channel.as_ref().len())
        .max()
        .unwrap_or(0);
    WaveformData {
        peaks: Some(compute_peaks(channels, samples)),
        duration: if sample_rate > 0.0 {
            length as f64 / sample_rate as f64
        } else {
            0.0
        },
        status: WaveformDataStatus::Ready,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_loudest_sample_per_bucket_across_channels_and_normalises() {
        let left = [0.1, -0.2, 0.0, 0.05, -0.4, 0.1, 0.0, 0.0];
        let right = [0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.2, -0.1];
        let peaks = compute_peaks(&[&left[..], &right[..]], 4);
        let expected = [0.5, 0.75, 1.0, 0.5];
        for (peak, expected) in peaks.iter().zip(expected) {
            assert!((peak - expected).abs() < 1e-6, "{peaks:?}");
        }
    }

    #[test]
    fn short_buffers_and_silence() {
        let peaks = compute_peaks(&[vec![0.5f32, -1.0]], 4);
        assert_eq!(peaks, vec![0.5, 1.0, 0.0, 0.0]);
        assert_eq!(compute_peaks(&[vec![0.0f32; 16]], 4), vec![0.0; 4]);
        assert_eq!(compute_peaks::<Vec<f32>>(&[], 3), vec![0.0; 3]);
    }

    #[test]
    fn ready_data_has_a_duration() {
        let data =
            waveform_data_from_samples(&[vec![0.25f32; 48_000]], 48_000.0, DEFAULT_PEAK_COUNT);
        assert_eq!(data.status, WaveformDataStatus::Ready);
        assert_eq!(data.duration, 1.0);
        assert_eq!(data.peaks.unwrap().len(), DEFAULT_PEAK_COUNT);
    }
}
