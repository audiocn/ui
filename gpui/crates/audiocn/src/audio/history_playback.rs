//! Port of `lib/audio/history-playback.ts`.
//!
//! Plays a frame's history back two sample intervals late, so a scrolling
//! waveform moves through known samples instead of predicting the next one.
//! Keep a [`HistoryPlayback`] in element state and call
//! [`read`](HistoryPlayback::read) once per paint with the latest frame.

use std::collections::VecDeque;

use crate::audio::decibels::clamp64;
use crate::audio::types::VisualFrame;

#[derive(Clone, Debug)]
struct HistorySample {
    frame: VisualFrame,
    from_ms: f64,
    to_ms: f64,
}

/// The frame to draw and how far (0..1) the newest entry has scrolled in.
#[derive(Clone, Copy, Debug)]
pub struct HistoryPlaybackRead<'a> {
    /// Either the frame passed in (untimed frames) or a snapshot of an earlier one.
    pub frame: &'a VisualFrame,
    pub progress: f32,
}

/// Delayed playback of a frame's history ring.
#[derive(Clone, Debug, Default)]
pub struct HistoryPlayback {
    samples: VecDeque<HistorySample>,
    previous_ms: Option<f64>,
    previous_length: usize,
    interval_ms: Option<f64>,
}

/// Creates an empty [`HistoryPlayback`].
pub fn create_history_playback() -> HistoryPlayback {
    HistoryPlayback::default()
}

impl HistoryPlayback {
    /// Same as [`create_history_playback`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets all samples.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.previous_ms = None;
        self.previous_length = 0;
        self.interval_ms = None;
    }

    /// The frame to draw at `now_ms` and its scroll progress. Frames without
    /// timing (`history_updated_at` / a positive `history_interval_ms`) or with
    /// an empty history are returned as they are, with progress 1.
    pub fn read<'a>(&'a mut self, frame: &'a VisualFrame, now_ms: f64) -> HistoryPlaybackRead<'a> {
        let (Some(time), Some(interval)) = (frame.history_updated_at, frame.history_interval_ms)
        else {
            self.clear();
            return HistoryPlaybackRead {
                frame,
                progress: 1.0,
            };
        };
        if !time.is_finite()
            || !interval.is_finite()
            || interval <= 0.0
            || frame.history_length == 0
        {
            self.clear();
            return HistoryPlaybackRead {
                frame,
                progress: 1.0,
            };
        }
        if self.interval_ms != Some(interval)
            || frame.history_length < self.previous_length
            || self.previous_ms.is_some_and(|previous| time < previous)
        {
            self.clear();
        }
        self.interval_ms = Some(interval);
        self.previous_length = frame.history_length;
        if Some(time) != self.previous_ms {
            self.samples.push_back(HistorySample {
                frame: frame.clone(),
                from_ms: self.previous_ms.unwrap_or(time - interval),
                to_ms: time,
            });
            self.previous_ms = Some(time);
        }

        let display_ms = now_ms - interval * 2.0;
        while self.samples.len() > 1 && self.samples.front().is_some_and(|s| s.to_ms <= display_ms)
        {
            self.samples.pop_front();
        }
        let Some(sample) = self.samples.front() else {
            return HistoryPlaybackRead {
                frame,
                progress: 1.0,
            };
        };
        HistoryPlaybackRead {
            frame: &sample.frame,
            progress: clamp64(
                (display_ms - sample.from_ms) / (sample.to_ms - sample.from_ms),
                0.0,
                1.0,
            ) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::history::{append_history, test_frame};

    fn close(a: f64, b: f64, digits: i32) -> bool {
        (a - b).abs() < 10f64.powi(-digits) / 2.0
    }

    #[test]
    fn moves_through_known_sample_intervals() {
        let mut frame = test_frame(32);
        let mut playback = create_history_playback();
        append_history(&mut frame, 0.25, 50.0, 50.0);
        playback.read(&frame, 50.0);
        append_history(&mut frame, 0.5, 116.6, 50.0);
        playback.read(&frame, 116.6);
        append_history(&mut frame, 0.75, 166.6, 50.0);
        playback.read(&frame, 166.6);

        for now_ms in [180.0, 200.0, 216.5] {
            let drawn = playback.read(&frame, now_ms);
            assert_eq!(drawn.frame.history_length, 2);
            assert!(close(
                drawn.progress as f64,
                (now_ms - 100.0 - 50.0) / 66.6,
                2
            ));
        }
        let boundary = playback.read(&frame, 216.6);
        assert_eq!(boundary.frame.history_length, 3);
        assert!(close(boundary.progress as f64, 0.0, 2));
        let next = playback.read(&frame, 233.3);
        assert!(close(next.progress as f64, 16.7 / 50.0, 2));
    }

    #[test]
    fn keeps_the_same_travel_with_steady_samples() {
        let mut frame = test_frame(32);
        let mut playback = create_history_playback();
        let mut previous: Option<f64> = None;
        let mut now_ms = 16.0;
        while now_ms <= 1024.0 {
            append_history(&mut frame, 0.5, now_ms, 50.0);
            let drawn = playback.read(&frame, now_ms);
            let position = drawn.frame.history_length as f64 + drawn.progress as f64;
            if now_ms >= 192.0
                && let Some(previous) = previous
            {
                assert!(close(position - previous, 16.0 / 64.0, 2));
            }
            previous = Some(position);
            now_ms += 16.0;
        }
        assert_eq!(frame.history_length, 16);
    }

    #[test]
    fn same_position_with_extra_paints() {
        let mut frame = test_frame(32);
        let mut regular = create_history_playback();
        let mut frequent = create_history_playback();
        let mut now_ms = 16.0;
        while now_ms <= 2048.0 {
            append_history(&mut frame, 0.5, now_ms, 50.0);
            let expected = regular.read(&frame, now_ms);
            let (expected_at, expected_progress) =
                (expected.frame.history_updated_at, expected.progress);
            let actual = frequent.read(&frame, now_ms);
            assert_eq!(actual.frame.history_updated_at, expected_at);
            assert!(close(actual.progress as f64, expected_progress as f64, 6));
            frequent.read(&frame, now_ms + 4.0);
            frequent.read(&frame, now_ms + 8.0);
            frequent.read(&frame, now_ms + 12.0);
            now_ms += 16.0;
        }
    }

    #[test]
    fn bounded_travel_and_delay_after_wrap() {
        for size in [1usize, 4, 120] {
            let mut frame = test_frame(size);
            let mut playback = create_history_playback();
            let mut times: Vec<f64> = Vec::new();
            let mut previous_position = 0.0;
            let mut now_ms = 16.0;
            while now_ms <= 16_384.0 {
                append_history(&mut frame, (now_ms / 32_768.0) as f32, now_ms, 50.0);
                if frame.history_updated_at == Some(now_ms) {
                    times.push(now_ms);
                }
                let read = playback.read(&frame, now_ms);
                let drawn = read.frame;
                let updated = drawn.history_updated_at.unwrap_or(-1.0);
                let index = times
                    .iter()
                    .position(|&t| t == updated)
                    .map_or(-1.0, |i| i as f64);
                let position = index + read.progress as f64;
                if now_ms >= 192.0 {
                    assert!(
                        close(position - previous_position, 0.25, 3),
                        "size {size} at {now_ms}"
                    );
                    let timestamp = drawn.history_updated_at.unwrap_or(0.0);
                    assert!(now_ms - timestamp >= 36.0);
                    assert!(now_ms - timestamp < 100.0);
                    let newest = (drawn.history_start + drawn.history_length - 1) % size;
                    assert_eq!(drawn.history[newest], (timestamp / 32_768.0) as f32);
                }
                previous_position = position;
                now_ms += 16.0;
            }
            assert_eq!(times.len(), 256);
        }
    }

    #[test]
    fn copies_history_only_when_a_new_sample_arrives() {
        let mut frame = test_frame(32);
        let mut playback = create_history_playback();
        append_history(&mut frame, 0.25, 50.0, 50.0);
        let first = playback.read(&frame, 50.0).frame as *const VisualFrame;
        frame.history[0] = 0.75;
        let same = playback.read(&frame, 80.0);
        assert!(std::ptr::eq(same.frame, first));
        assert_eq!(same.frame.history[0], 0.25);
        append_history(&mut frame, 0.5, 100.0, 50.0);
        playback.read(&frame, 100.0);
        let next = playback.read(&frame, 180.0);
        assert!(!std::ptr::eq(next.frame, first));
        assert_eq!(next.frame.history[0], 0.75);
    }

    #[test]
    fn stops_at_the_newest_sample_during_a_pause() {
        let mut frame = test_frame(32);
        let mut playback = create_history_playback();
        append_history(&mut frame, 0.25, 64.0, 50.0);
        playback.read(&frame, 64.0);
        assert_eq!(playback.read(&frame, 1000.0).progress, 1.0);
        append_history(&mut frame, 0.75, 10_000.0, 50.0);
        let resumed = playback.read(&frame, 10_000.0);
        assert_eq!(resumed.frame.history_length, 2);
        assert!(resumed.progress > 0.98);
    }

    #[test]
    fn clears_when_timing_or_history_resets() {
        let mut frame = test_frame(32);
        let mut playback = create_history_playback();
        append_history(&mut frame, 0.25, 50.0, 50.0);
        playback.read(&frame, 50.0);
        append_history(&mut frame, 0.5, 100.0, 50.0);
        playback.read(&frame, 100.0);
        let mut replacement = test_frame(32);
        append_history(&mut replacement, 1.0, 200.0, 100.0);
        let drawn = playback.read(&replacement, 200.0);
        assert_eq!(drawn.frame.history_length, 1);
        assert_eq!(drawn.frame.history[0], 1.0);
        playback.clear();
        assert_eq!(playback.read(&frame, 300.0).frame.history[0], 0.25);
    }

    #[test]
    fn leaves_untimed_frames_unchanged() {
        let frame = test_frame(32);
        let mut playback = create_history_playback();
        let read = playback.read(&frame, 100.0);
        assert!(std::ptr::eq(read.frame, &frame));
        assert_eq!(read.progress, 1.0);
    }
}
