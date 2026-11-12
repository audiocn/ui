//! Port of `lib/audio/history.ts`.

use crate::audio::types::VisualFrame;

/// Appends `level` to the frame's history ring if at least `interval_ms` has
/// passed since the last entry (a non-finite interval counts as 0). Updates
/// `history_updated_at`, `history_interval_ms` and, once the ring is full,
/// `history_previous_level`. Does nothing for an empty history.
pub fn append_history(frame: &mut VisualFrame, level: f32, now_ms: f64, interval_ms: f64) {
    let interval = if interval_ms.is_finite() {
        interval_ms.max(0.0)
    } else {
        0.0
    };
    let elapsed = now_ms - frame.history_updated_at.unwrap_or(0.0);
    if elapsed < interval || frame.history.is_empty() {
        return;
    }

    frame.history_interval_ms = Some(interval);
    frame.history_updated_at = Some(now_ms);
    let size = frame.history.len();
    if frame.history_length < size {
        frame.history[(frame.history_start + frame.history_length) % size] = level;
        frame.history_length += 1;
    } else {
        frame.history_previous_level = Some(frame.history[frame.history_start]);
        frame.history[frame.history_start] = level;
        frame.history_start = (frame.history_start + 1) % size;
    }
}

#[cfg(test)]
pub(crate) fn test_frame(size: usize) -> VisualFrame {
    VisualFrame {
        bands: vec![0.0; 1],
        history: vec![0.0; size],
        history_length: 0,
        history_start: 0,
        peak_db: -12.0,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels(frame: &VisualFrame) -> Vec<f32> {
        (0..frame.history_length)
            .map(|i| frame.history[(frame.history_start + i) % frame.history.len()])
            .collect()
    }

    #[test]
    fn matches_the_original_sample_schedule() {
        for frame_ms in [16.0, 1000.0 / 60.0, 1000.0 / 120.0, 1000.0 / 144.0] {
            let mut frame = test_frame(32);
            let mut last_sample_ms = 0.0;
            let mut expected_length = 0;
            let mut tick = 1.0;
            while tick * frame_ms <= 1024.0 {
                let now_ms = tick * frame_ms;
                if now_ms - last_sample_ms >= 50.0 {
                    last_sample_ms = now_ms;
                    expected_length += 1;
                }
                append_history(&mut frame, 0.5, now_ms, 50.0);
                assert_eq!(frame.history_length, expected_length);
                if expected_length > 0 {
                    assert_eq!(frame.history_updated_at, Some(last_sample_ms));
                }
                tick += 1.0;
            }
        }
    }

    #[test]
    fn does_not_carry_late_frame_time() {
        let mut frame = test_frame(32);
        append_history(&mut frame, 0.25, 64.0, 50.0);
        append_history(&mut frame, 0.5, 112.0, 50.0);
        assert_eq!(frame.history_updated_at, Some(64.0));
        assert_eq!(levels(&frame), [0.25]);
        append_history(&mut frame, 0.5, 128.0, 50.0);
        assert_eq!(frame.history_updated_at, Some(128.0));
        assert_eq!(frame.history_interval_ms, Some(50.0));
        append_history(&mut frame, 0.75, 192.0, 50.0);
        assert_eq!(levels(&frame), [0.25, 0.5, 0.75]);
    }

    #[test]
    fn appends_only_the_new_sample_after_a_pause() {
        let mut frame = test_frame(3);
        append_history(&mut frame, 0.25, 50.0, 50.0);
        append_history(&mut frame, 0.5, 100.0, 50.0);
        append_history(&mut frame, 0.75, 100_010.0, 50.0);
        assert_eq!(frame.history_updated_at, Some(100_010.0));
        assert_eq!(frame.history_interval_ms, Some(50.0));
        assert_eq!(levels(&frame), [0.25, 0.5, 0.75]);
        assert_eq!(frame.history_previous_level, None);
        append_history(&mut frame, 1.0, 100_074.0, 50.0);
        assert_eq!(frame.history_updated_at, Some(100_074.0));
        assert_eq!(levels(&frame), [0.5, 0.75, 1.0]);
        assert_eq!(frame.history_previous_level, Some(0.25));
    }

    #[test]
    fn does_not_fill_with_held_values() {
        let mut frame = test_frame(32);
        append_history(&mut frame, 0.25, 50.0, 50.0);
        append_history(&mut frame, 0.5, 1000.0, 50.0);
        append_history(&mut frame, 0.75, 2000.0, 50.0);
        assert_eq!(levels(&frame), [0.25, 0.5, 0.75]);
        assert_eq!(frame.history_updated_at, Some(2000.0));
    }

    #[test]
    fn keeps_the_outgoing_level_when_wrapping() {
        let mut frame = test_frame(2);
        append_history(&mut frame, 0.25, 50.0, 50.0);
        append_history(&mut frame, 0.5, 100.0, 50.0);
        append_history(&mut frame, 0.75, 150.0, 50.0);
        assert_eq!(levels(&frame), [0.5, 0.75]);
        assert_eq!(frame.history_previous_level, Some(0.25));
    }

    #[test]
    fn uses_the_new_interval() {
        let mut frame = test_frame(32);
        append_history(&mut frame, 0.25, 50.0, 50.0);
        append_history(&mut frame, 0.5, 125.0, 100.0);
        assert_eq!(frame.history_updated_at, Some(50.0));
        assert_eq!(levels(&frame), [0.25]);
        append_history(&mut frame, 0.5, 150.0, 100.0);
        assert_eq!(frame.history_interval_ms, Some(100.0));
        append_history(&mut frame, 0.75, 174.0, 25.0);
        assert_eq!(levels(&frame), [0.25, 0.5]);
        append_history(&mut frame, 0.75, 175.0, 25.0);
        assert_eq!(levels(&frame), [0.25, 0.5, 0.75]);
        assert_eq!(frame.history_interval_ms, Some(25.0));
    }

    #[test]
    fn supports_zero_interval_and_empty_history() {
        let mut frame = test_frame(32);
        append_history(&mut frame, 0.25, 0.0, 0.0);
        append_history(&mut frame, 0.5, 16.0, 0.0);
        assert_eq!(levels(&frame), [0.25, 0.5]);
        assert_eq!(frame.history_interval_ms, Some(0.0));
        let mut empty = test_frame(0);
        append_history(&mut empty, 1.0, 50.0, 50.0);
        assert_eq!(empty.history_length, 0);
        assert_eq!(empty.history_start, 0);
    }
}
