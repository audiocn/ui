//! Port of `hooks/use-tap-tempo.ts`: tap tempo with a four-interval average
//! and a two-second idle reset, controlled or uncontrolled.
//!
//! React calls `onValueChange` / `onValueCommitted`; here [`TapTempo::tap`]
//! and [`TapTempo::set_value`] return what should be reported, and the
//! component forwards it to its callbacks.

const MAX_TAPS: usize = 5;
const MIN_TAP_INTERVAL: f64 = 80.0;
const MINUTE_MS: f64 = 60_000.0;

/// Why the tempo changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TempoChangeReason {
    #[default]
    Input,
    Keyboard,
    Tap,
}

/// Details passed with a tempo change (`TempoChangeDetails`, minus the DOM event).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TempoChangeDetails {
    pub reason: TempoChangeReason,
}

impl TempoChangeDetails {
    pub fn new(reason: TempoChangeReason) -> Self {
        Self { reason }
    }
}

/// Options for [`TapTempo`] (`UseTapTempoOptions` without the callbacks).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapTempoOptions {
    /// Controlled BPM. `None` makes the tempo uncontrolled.
    pub value: Option<f32>,
    /// Initial uncontrolled BPM. Default 120.
    pub default_value: f32,
    /// Lowest BPM (rounded up). Default 30.
    pub min: f32,
    /// Highest BPM (rounded down). Default 600.
    pub max: f32,
    /// Idle time after which taps start a new measurement. Default 2000 ms.
    pub tap_timeout: f64,
    pub disabled: bool,
    pub read_only: bool,
}

impl Default for TapTempoOptions {
    fn default() -> Self {
        Self {
            value: None,
            default_value: 120.0,
            min: 30.0,
            max: 600.0,
            tap_timeout: 2000.0,
            disabled: false,
            read_only: false,
        }
    }
}

/// What a [`TapTempo::tap`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TapOutcome {
    /// The tap was counted (not disabled, not a bounce under 80 ms).
    pub accepted: bool,
    /// Report this to `on_value_change` (with reason `Tap`).
    pub changed: Option<f32>,
    /// Report this to `on_value_committed`: every measured tap, changed or not.
    pub committed: Option<f32>,
}

/// Tap tempo state (`useTapTempo`).
///
/// ```ignore
/// let mut tempo = TapTempo::new(TapTempoOptions::default());
/// let outcome = tempo.tap(now_ms);
/// if let Some(bpm) = outcome.changed { on_value_change(bpm, TempoChangeDetails::new(TempoChangeReason::Tap)) }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct TapTempo {
    options: TapTempoOptions,
    uncontrolled: f32,
    latest_value: f32,
    taps: Vec<f64>,
    tap_count: u32,
}

fn round(value: f32) -> f32 {
    // `Math.round`: halves go up, also for negatives.
    (value + 0.5).floor()
}

impl TapTempo {
    pub fn new(options: TapTempoOptions) -> Self {
        let mut tempo = Self {
            options,
            uncontrolled: options.default_value,
            latest_value: 0.0,
            taps: Vec::with_capacity(MAX_TAPS + 1),
            tap_count: 0,
        };
        tempo.latest_value = tempo.value();
        tempo
    }

    /// Applies new options (call on every render). Like the hook's memoised
    /// measurement, the tap history restarts when `disabled`, `read_only`,
    /// the bounds or `tap_timeout` change.
    pub fn set_options(&mut self, options: TapTempoOptions) {
        let previous = self.options;
        self.options = options;
        if previous.disabled != options.disabled
            || previous.read_only != options.read_only
            || self.min_for(&previous) != self.min()
            || self.max_for(&previous) != self.max()
            || previous.tap_timeout != options.tap_timeout
        {
            self.taps.clear();
        }
        self.latest_value = self.value();
    }

    pub fn options(&self) -> &TapTempoOptions {
        &self.options
    }

    fn min_for(&self, options: &TapTempoOptions) -> f32 {
        options.min.ceil()
    }

    fn max_for(&self, options: &TapTempoOptions) -> f32 {
        options.max.floor()
    }

    /// Lowest whole BPM.
    pub fn min(&self) -> f32 {
        self.min_for(&self.options)
    }

    /// Highest whole BPM.
    pub fn max(&self) -> f32 {
        self.max_for(&self.options)
    }

    fn bound(&self, value: f32) -> f32 {
        round(value).max(self.min()).min(self.max())
    }

    /// The current tempo, a whole BPM inside the bounds.
    pub fn value(&self) -> f32 {
        self.bound(self.options.value.unwrap_or(self.uncontrolled))
    }

    /// Accepted taps since creation (for feedback such as a pulse).
    pub fn tap_count(&self) -> u32 {
        self.tap_count
    }

    /// Forgets the tap history; the next tap starts a new measurement.
    pub fn reset_taps(&mut self) {
        self.taps.clear();
    }

    fn measure(&mut self, now: f64) -> Option<Option<f32>> {
        if self.options.disabled || self.options.read_only {
            return None;
        }
        if let Some(&previous) = self.taps.last() {
            let interval = now - previous;
            if (0.0..MIN_TAP_INTERVAL).contains(&interval) {
                return None;
            }
            if interval > self.options.tap_timeout || interval < 0.0 {
                self.taps.clear();
            }
        }
        self.taps.push(now);
        if self.taps.len() > MAX_TAPS {
            self.taps.remove(0);
        }
        let first = self.taps[0];
        if self.taps.len() < 2 {
            return Some(None);
        }
        let bpm = (MINUTE_MS * (self.taps.len() - 1) as f64) / (now - first);
        Some(Some(self.bound(bpm as f32)))
    }

    /// Registers a tap at `now_ms` (a monotonic clock, like `performance.now()`).
    pub fn tap(&mut self, now_ms: f64) -> TapOutcome {
        let Some(measured) = self.measure(now_ms) else {
            return TapOutcome::default();
        };
        self.tap_count += 1;
        let Some(next) = measured else {
            return TapOutcome {
                accepted: true,
                ..Default::default()
            };
        };
        TapOutcome {
            accepted: true,
            changed: self.change(next),
            committed: Some(next),
        }
    }

    /// Sets the tempo from an edit (typing, keyboard). Clears the tap
    /// history. Returns the whole, bounded BPM to report to
    /// `on_value_change`, or `None` when nothing changes (or disabled).
    /// Controlled edits are proposals: the value only moves when the
    /// parent passes it back.
    pub fn set_value(&mut self, next: f32) -> Option<f32> {
        self.taps.clear();
        self.change(next)
    }

    fn change(&mut self, next: f32) -> Option<f32> {
        if self.options.disabled || self.options.read_only || !next.is_finite() {
            return None;
        }
        let bounded = self.bound(next);
        let current = if self.options.value.is_none() {
            self.latest_value
        } else {
            self.value()
        };
        if bounded == current {
            return None;
        }
        if self.options.value.is_none() {
            self.latest_value = bounded;
            self.uncontrolled = bounded;
        }
        Some(bounded)
    }
}

impl Default for TapTempo {
    fn default() -> Self {
        Self::new(TapTempoOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_default_controlled_edited_and_tapped_values_to_whole_bpm() {
        let mut tempo = TapTempo::new(TapTempoOptions {
            default_value: 90.6,
            ..Default::default()
        });
        assert_eq!(tempo.value(), 91.0);
        assert_eq!(tempo.set_value(123.4), Some(123.0));
        assert_eq!(tempo.value(), 123.0);
        tempo.tap(0.0);
        tempo.tap(487.0);
        assert_eq!(tempo.value(), 123.0);
        tempo.set_options(TapTempoOptions {
            default_value: 90.6,
            value: Some(96.7),
            ..Default::default()
        });
        assert_eq!(tempo.value(), 97.0);
    }

    #[test]
    fn keeps_normalized_bpm_inside_fractional_bounds() {
        let mut tempo = TapTempo::new(TapTempoOptions {
            max: 180.5,
            min: 60.5,
            ..Default::default()
        });
        tempo.set_value(30.0);
        assert_eq!(tempo.value(), 61.0);
        tempo.set_value(200.0);
        assert_eq!(tempo.value(), 180.0);
    }

    #[test]
    fn reports_every_distinct_edit_including_returning_to_the_original_value() {
        let mut tempo = TapTempo::default();
        let calls: Vec<_> = [100.0, 100.0, 120.0]
            .into_iter()
            .filter_map(|bpm| tempo.set_value(bpm))
            .collect();
        assert_eq!(tempo.value(), 120.0);
        assert_eq!(calls, vec![100.0, 120.0]);
    }

    #[test]
    fn reports_repeated_controlled_edits_without_requiring_the_parent_to_rerender() {
        let mut tempo = TapTempo::new(TapTempoOptions {
            value: Some(90.0),
            ..Default::default()
        });
        assert_eq!(tempo.set_value(100.0), Some(100.0));
        assert_eq!(tempo.value(), 90.0);
        assert_eq!(tempo.set_value(100.0), Some(100.0));
        assert_eq!(tempo.value(), 90.0);
    }

    #[test]
    fn compares_controlled_edits_against_the_authoritative_value_after_a_rejected_edit() {
        let mut tempo = TapTempo::new(TapTempoOptions {
            value: Some(90.0),
            ..Default::default()
        });
        assert_eq!(tempo.set_value(100.0), Some(100.0));
        assert_eq!(tempo.set_value(90.0), None);
        assert_eq!(tempo.value(), 90.0);
    }

    #[test]
    fn measures_high_tempos() {
        let mut tempo = TapTempo::default();
        tempo.tap(0.0);
        assert_eq!(tempo.value(), 120.0);
        let outcome = tempo.tap(150.0);
        assert_eq!(outcome.changed, Some(400.0));
        assert_eq!(outcome.committed, Some(400.0));
        assert_eq!(tempo.value(), 400.0);
        assert_eq!(tempo.tap_count(), 2);
    }

    #[test]
    fn counts_accepted_taps_for_feedback_even_when_bpm_does_not_change() {
        let mut tempo = TapTempo::default();
        tempo.tap(0.0);
        assert!(!tempo.tap(40.0).accepted);
        assert_eq!(tempo.tap_count(), 1);
        let outcome = tempo.tap(500.0);
        assert_eq!(outcome.changed, None);
        assert_eq!(outcome.committed, Some(120.0));
        assert_eq!(tempo.value(), 120.0);
        assert_eq!(tempo.tap_count(), 2);
        tempo.tap(1000.0);
        assert_eq!(tempo.value(), 120.0);
        assert_eq!(tempo.tap_count(), 3);
    }

    #[test]
    fn resets_history_on_edits_and_clamps_values_to_the_default_maximum() {
        let mut tempo = TapTempo::default();
        tempo.tap(0.0);
        tempo.set_value(750.0);
        assert_eq!(tempo.value(), 600.0);
        tempo.tap(600.0);
        assert_eq!(tempo.value(), 600.0);
        tempo.tap(1200.0);
        assert_eq!(tempo.value(), 100.0);
        tempo.reset_taps();
        tempo.tap(1700.0);
        assert_eq!(tempo.value(), 100.0);
    }

    #[test]
    fn idle_timeout_and_disabled() {
        let mut tempo = TapTempo::default();
        tempo.tap(0.0);
        tempo.tap(500.0);
        // More than two seconds later: a new measurement.
        assert_eq!(tempo.tap(3000.0).committed, None);
        assert_eq!(tempo.tap(3250.0).changed, Some(240.0));
        tempo.set_options(TapTempoOptions {
            disabled: true,
            ..Default::default()
        });
        assert_eq!(tempo.tap(4000.0), TapOutcome::default());
        assert_eq!(tempo.set_value(90.0), None);
    }
}
