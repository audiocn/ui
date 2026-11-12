//! Port of `hooks/use-clip-hold.ts`: a clip light that latches when a level
//! reaches the threshold and releases `hold_ms` after the last clip.
//!
//! React uses `setTimeout`; here the release is a deadline on a millisecond
//! clock. Feed readings with [`ClipHold::report`], call [`ClipHold::update`]
//! each frame (or schedule a timer for [`ClipHold::release_at_ms`]), and
//! forward the returned changes to `on_clipping_change`.

use crate::audio::zones::{CLIP_HOLD_MS, CLIP_THRESHOLD_DB};

/// Options for [`ClipHold`] (`UseClipHoldOptions` without the callback).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipHoldOptions {
    /// Levels at or above this count as a clip. Default −1 dBFS.
    pub threshold_db: f32,
    /// How long the clip state holds. `f64::INFINITY` latches until
    /// [`ClipHold::reset`]. Default 1500 ms.
    pub hold_ms: f64,
}

impl Default for ClipHoldOptions {
    fn default() -> Self {
        Self {
            threshold_db: CLIP_THRESHOLD_DB,
            hold_ms: CLIP_HOLD_MS,
        }
    }
}

/// Clip latch / hold state (`useClipHold`).
///
/// ```ignore
/// let mut hold = ClipHold::new(ClipHoldOptions::default());
/// if let Some(clipping) = hold.report(peak_db, now_ms) { on_clipping_change(clipping) }
/// if let Some(clipping) = hold.update(now_ms) { on_clipping_change(clipping) }
/// if hold.release_at_ms().is_some() { window.request_animation_frame() }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ClipHold {
    options: ClipHoldOptions,
    clipping: bool,
    count: u32,
    above: bool,
    release_at_ms: Option<f64>,
}

impl ClipHold {
    pub fn new(options: ClipHoldOptions) -> Self {
        Self {
            options,
            clipping: false,
            count: 0,
            above: false,
            release_at_ms: None,
        }
    }

    pub fn options(&self) -> &ClipHoldOptions {
        &self.options
    }

    /// Applies new options. Like the hook's effect, a new hold time re-arms
    /// a pending release from `now_ms`.
    pub fn set_options(&mut self, options: ClipHoldOptions, now_ms: f64) {
        let hold_changed = options.hold_ms != self.options.hold_ms;
        self.options = options;
        if hold_changed && self.clipping {
            self.schedule_release(now_ms);
        }
    }

    /// Whether the clip light is on.
    pub fn clipping(&self) -> bool {
        self.clipping
    }

    /// Number of separate clips since creation or the last reset.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// When the light will go out, if it is on and not latched.
    pub fn release_at_ms(&self) -> Option<f64> {
        self.release_at_ms
    }

    fn set_clipping(&mut self, next: bool) -> Option<bool> {
        if self.clipping == next {
            return None;
        }
        self.clipping = next;
        Some(next)
    }

    fn schedule_release(&mut self, now_ms: f64) {
        let hold_ms = self.options.hold_ms;
        self.release_at_ms = hold_ms.is_finite().then_some(now_ms + hold_ms);
    }

    /// Feeds one level reading (dBFS) taken at `now_ms`. Returns the new
    /// clipping state when it changed. A clip counts once per rise above
    /// the threshold; every reading above it restarts the hold.
    pub fn report(&mut self, db: f32, now_ms: f64) -> Option<bool> {
        let changed = self.update(now_ms);
        let above = db >= self.options.threshold_db;
        let was_above = self.above;
        self.above = above;
        if !above {
            return changed;
        }
        if !was_above {
            self.count += 1;
        }
        let rose = self.set_clipping(true);
        self.schedule_release(now_ms);
        // Released and clipped again in the same call: still on, no change.
        match (changed, rose) {
            (Some(false), Some(true)) => None,
            (changed, rose) => rose.or(changed),
        }
    }

    /// Releases the light once its hold has elapsed. Returns `Some(false)`
    /// when it went out.
    pub fn update(&mut self, now_ms: f64) -> Option<bool> {
        match self.release_at_ms {
            Some(release_at) if now_ms >= release_at => {
                self.release_at_ms = None;
                self.set_clipping(false)
            }
            _ => None,
        }
    }

    /// Turns the light off and clears the count.
    pub fn reset(&mut self) -> Option<bool> {
        self.release_at_ms = None;
        self.above = false;
        self.count = 0;
        self.set_clipping(false)
    }
}

impl Default for ClipHold {
    fn default() -> Self {
        Self::new(ClipHoldOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_separate_clips_and_releases_after_the_hold() {
        let mut hold = ClipHold::new(ClipHoldOptions {
            hold_ms: 200.0,
            ..Default::default()
        });
        assert_eq!(hold.report(0.0, 0.0), Some(true));
        assert_eq!(hold.report(0.0, 0.0), None);
        assert_eq!(hold.report(-20.0, 0.0), None);
        assert_eq!(hold.report(-0.5, 0.0), None);
        assert_eq!(hold.count(), 2);
        assert!(hold.clipping());
        assert_eq!(hold.update(150.0), None);
        assert_eq!(hold.update(250.0), Some(false));
        assert!(!hold.clipping());
    }

    #[test]
    fn every_clip_restarts_the_hold() {
        let mut hold = ClipHold::new(ClipHoldOptions {
            hold_ms: 200.0,
            ..Default::default()
        });
        hold.report(0.0, 0.0);
        hold.report(0.0, 150.0);
        assert_eq!(hold.update(250.0), None);
        assert_eq!(hold.update(350.0), Some(false));
    }

    #[test]
    fn infinite_hold_latches_until_reset() {
        let mut hold = ClipHold::new(ClipHoldOptions {
            hold_ms: f64::INFINITY,
            ..Default::default()
        });
        hold.report(-1.0, 0.0);
        assert_eq!(hold.release_at_ms(), None);
        assert_eq!(hold.update(1e9), None);
        assert!(hold.clipping());
        assert_eq!(hold.reset(), Some(false));
        assert_eq!(hold.count(), 0);
        assert_eq!(hold.report(-1.1, 0.0), None);
    }

    #[test]
    fn clipping_again_after_release_is_reported_once() {
        let mut hold = ClipHold::new(ClipHoldOptions {
            hold_ms: 100.0,
            ..Default::default()
        });
        hold.report(0.0, 0.0);
        hold.report(-30.0, 10.0);
        // Released at 100 and clipped again in the same reading.
        assert_eq!(hold.report(0.0, 120.0), None);
        assert!(hold.clipping());
        assert_eq!(hold.count(), 2);
    }
}
