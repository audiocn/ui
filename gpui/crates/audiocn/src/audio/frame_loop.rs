//! Port of `lib/audio/frame-loop.ts`.
//!
//! **What changed for GPUI.** The TS file runs one page-wide
//! `requestAnimationFrame` loop (`subscribeFrame`, with an "update" phase for
//! sources before a "paint" phase for renderers). GPUI already has that loop:
//! an element calls `window.request_animation_frame()` during render/paint and
//! is painted again next frame. So `subscribeFrame` has no port; instead:
//!
//! - [`now_ms`] is the frame clock (TS: the rAF timestamp), milliseconds on a
//!   monotonic `web_time::Instant` clock since the first call in the process.
//!   Use it for every `now_ms` argument and for `VisualFrame` timestamps.
//! - [`FrameTask`] is `createFrameTask` as a state machine: call
//!   [`FrameTask::run`] from paint with your step; it returns whether to call
//!   `window.request_animation_frame()` again. A sleeping task costs nothing
//!   until [`FrameTask::wake`] (e.g. a new source frame arrived).
//! - [`PainterClock`] / [`create_painter_clock`] port unchanged.
//! - The update-before-paint ordering is the producer's job: emit into a
//!   `FrameEmitter` before (or independently of) painting; painters read
//!   `FrameSource::latest()`.

use std::sync::OnceLock;

use web_time::Instant;

use crate::audio::decibels::clamp64;

static START: OnceLock<Instant> = OnceLock::new();

/// Milliseconds on the frame clock: a monotonic clock starting at the first
/// call in this process. Replaces the `requestAnimationFrame` timestamp.
pub fn now_ms() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// One frame of a task. Return true while the task needs another frame.
pub trait FrameTaskStep: FnMut(f64) -> bool {}
impl<F: FnMut(f64) -> bool> FrameTaskStep for F {}

/// `createFrameTask`: runs a step every frame for as long as it returns true,
/// then sleeps (requests no frames) until [`wake`](Self::wake). Starts awake.
///
/// ```ignore
/// // in paint, with `task: FrameTask` in keyed state:
/// if task.run(now_ms(), |now| painter.paint(now)) {
///     window.request_animation_frame();
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTask {
    awake: bool,
    stopped: bool,
}

impl Default for FrameTask {
    fn default() -> Self {
        create_frame_task()
    }
}

/// Creates an awake [`FrameTask`].
pub fn create_frame_task() -> FrameTask {
    FrameTask {
        awake: true,
        stopped: false,
    }
}

impl FrameTask {
    /// Runs the step again from the next frame, if the task is asleep.
    /// A stopped task ignores `wake`.
    pub fn wake(&mut self) {
        if !self.stopped {
            self.awake = true;
        }
    }

    /// Stops the task for good.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.awake = false;
    }

    /// Whether the task wants frames.
    pub fn is_awake(&self) -> bool {
        self.awake
    }

    /// Whether [`stop`](Self::stop) was called.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Runs one frame if awake; the task sleeps when `step` returns false.
    /// Returns whether another animation frame is needed.
    pub fn run(&mut self, now_ms: f64, step: impl FnOnce(f64) -> bool) -> bool {
        if self.awake && !step(now_ms) {
            self.awake = false;
        }
        self.awake
    }
}

/// The longest step a painter clock takes, so waking never jumps.
pub const MAX_FRAME_GAP_MS: f64 = 100.0;

/// A clock that follows the frames but never advances more than
/// [`MAX_FRAME_GAP_MS`] at once, and never runs backwards. A painter that
/// slept resumes its ballistics where they were instead of jumping to the end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PainterClock {
    last_ms: Option<f64>,
    clock_ms: f64,
}

/// Creates a [`PainterClock`] starting at 0.
pub fn create_painter_clock() -> PainterClock {
    PainterClock::default()
}

impl PainterClock {
    /// Advances to the frame time `now_ms` and returns the painter time.
    pub fn tick(&mut self, now_ms: f64) -> f64 {
        if let Some(last_ms) = self.last_ms {
            self.clock_ms += clamp64(now_ms - last_ms, 0.0, MAX_FRAME_GAP_MS);
        }
        self.last_ms = Some(now_ms);
        self.clock_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: f64 = 16.0;

    /// Drives a task like the host would: one run per frame while it asks for frames.
    fn drive(task: &mut FrameTask, frames: usize, mut step: impl FnMut(f64) -> bool) -> bool {
        let mut requested = task.is_awake();
        for frame in 1..=frames {
            if !requested {
                break;
            }
            requested = task.run(frame as f64 * FRAME_MS, &mut step);
        }
        requested
    }

    #[test]
    fn now_ms_is_monotonic() {
        let a = now_ms();
        let b = now_ms();
        assert!(b >= a && a >= 0.0);
    }

    #[test]
    fn runs_while_the_step_needs_frames() {
        let mut task = create_frame_task();
        let mut remaining = 3;
        let mut calls = 0;
        let requested = drive(&mut task, 10, |_| {
            calls += 1;
            remaining -= 1;
            remaining > 0
        });
        assert_eq!(calls, 3);
        assert!(!requested);
    }

    #[test]
    fn wakes_a_sleeping_task() {
        let mut task = create_frame_task();
        let mut calls = 0;
        drive(&mut task, 4, |_| {
            calls += 1;
            false
        });
        assert_eq!(calls, 1);
        task.wake();
        task.wake();
        let requested = drive(&mut task, 4, |_| {
            calls += 1;
            false
        });
        assert_eq!(calls, 2);
        assert!(!requested);
    }

    #[test]
    fn never_runs_once_stopped() {
        let mut task = create_frame_task();
        let mut calls = 0;
        drive(&mut task, 2, |_| {
            calls += 1;
            true
        });
        assert_eq!(calls, 2);
        task.stop();
        task.wake();
        let requested = drive(&mut task, 4, |_| {
            calls += 1;
            true
        });
        assert_eq!(calls, 2);
        assert!(!requested);
    }

    #[test]
    fn painter_clock_follows_frames() {
        let mut clock = create_painter_clock();
        assert_eq!(clock.tick(1000.0), 0.0);
        assert_eq!(clock.tick(1016.0), 16.0);
        assert_eq!(clock.tick(1032.0), 32.0);
    }

    #[test]
    fn painter_clock_never_jumps() {
        let mut clock = create_painter_clock();
        clock.tick(0.0);
        clock.tick(16.0);
        assert_eq!(clock.tick(60_000.0), 16.0 + MAX_FRAME_GAP_MS);
    }

    #[test]
    fn painter_clock_never_runs_backwards() {
        let mut clock = create_painter_clock();
        clock.tick(500.0);
        assert_eq!(clock.tick(400.0), 0.0);
        assert_eq!(clock.tick(416.0), 16.0);
    }
}
