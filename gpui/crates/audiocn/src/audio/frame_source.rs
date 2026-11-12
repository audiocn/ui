//! Port of `lib/audio/frame-source.ts` and `hooks/use-frame-source.ts`.
//!
//! In React, components subscribe to a `FrameSource` and paint on the animation
//! frame without re-rendering. In GPUI the same idea is a shared slot: producers
//! (a demo signal, an audio thread, a socket) [`emit`](FrameEmitter::emit) frames
//! from any thread, and components read [`latest`](FrameSource::latest) inside
//! their paint callback and call `window.request_animation_frame()` while live.
//!
//! [`FrameSource`] is cheap to clone (an `Arc`), `Send + Sync`, and compares by
//! identity, so it can be stored in element builders.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

struct Slot<T> {
    latest: Mutex<Option<T>>,
    version: AtomicU64,
}

/// A readable stream of frames. Clone it freely; every clone sees the same frames.
pub struct FrameSource<T> {
    slot: Arc<Slot<T>>,
}

/// The write end of a [`FrameSource`]. Created by [`create_frame_emitter`].
pub struct FrameEmitter<T> {
    slot: Arc<Slot<T>>,
}

impl<T> Clone for FrameSource<T> {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
        }
    }
}

impl<T> Clone for FrameEmitter<T> {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
        }
    }
}

impl<T> PartialEq for FrameSource<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.slot, &other.slot)
    }
}

impl<T> std::fmt::Debug for FrameSource<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameSource")
            .field("version", &self.version())
            .finish()
    }
}

/// Creates a frame source you push to yourself: from an audio thread, a socket,
/// a worker or a test. Returns the emitter; call [`FrameEmitter::source`] for
/// the read side.
pub fn create_frame_emitter<T>() -> FrameEmitter<T> {
    FrameEmitter {
        slot: Arc::new(Slot {
            latest: Mutex::new(None),
            version: AtomicU64::new(0),
        }),
    }
}

impl<T> FrameEmitter<T> {
    pub fn new() -> Self {
        create_frame_emitter()
    }

    /// Sends a frame to every reader.
    pub fn emit(&self, frame: T) {
        *self.slot.latest.lock() = Some(frame);
        self.slot.version.fetch_add(1, Ordering::Release);
    }

    /// Clears the latest frame (readers see `None`, i.e. "no signal").
    pub fn clear(&self) {
        *self.slot.latest.lock() = None;
        self.slot.version.fetch_add(1, Ordering::Release);
    }

    /// The read side of this emitter.
    pub fn source(&self) -> FrameSource<T> {
        FrameSource {
            slot: self.slot.clone(),
        }
    }
}

impl<T> Default for FrameEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FrameSource<T> {
    /// Increments on every emit. Compare against a stored value to detect new frames.
    pub fn version(&self) -> u64 {
        self.slot.version.load(Ordering::Acquire)
    }

    /// Runs `f` with the latest frame, without cloning it.
    pub fn with_latest<R>(&self, f: impl FnOnce(Option<&T>) -> R) -> R {
        f(self.slot.latest.lock().as_ref())
    }
}

impl<T: Clone> FrameSource<T> {
    /// The last frame emitted, if any.
    pub fn latest(&self) -> Option<T> {
        self.slot.latest.lock().clone()
    }
}
