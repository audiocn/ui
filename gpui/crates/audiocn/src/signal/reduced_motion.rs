//! Port of `hooks/use-reduced-motion.ts`.
//!
//! The browser reads `(prefers-reduced-motion: reduce)`. GPUI has no such
//! query, so the app sets it: a global flag, false by default. Components
//! check [`prefers_reduced_motion`] and skip ballistics / animation when true.

use gpui::{App, Global};

/// Whether the user asked to reduce motion. Set with [`set_reduced_motion`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ReducedMotion(pub bool);

impl Global for ReducedMotion {}

/// True when the app said the user prefers reduced motion (`useReducedMotion`).
///
/// ```ignore
/// if audiocn::signal::reduced_motion::prefers_reduced_motion(cx) { /* jump, don't animate */ }
/// ```
pub fn prefers_reduced_motion(cx: &App) -> bool {
    cx.try_global::<ReducedMotion>()
        .is_some_and(|reduced| reduced.0)
}

/// Sets the reduced-motion preference for every component (e.g. from the
/// OS accessibility setting or an app preference) and refreshes windows.
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    cx.set_global(ReducedMotion(reduced));
    cx.refresh_windows();
}
