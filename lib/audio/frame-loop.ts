type FrameListener = (nowMs: number) => void;

const listeners = new Set<FrameListener>();
let handle: number | null = null;

const reportError = (error: unknown) => {
  queueMicrotask(() => {
    throw error;
  });
};

const tick = (nowMs: number) => {
  handle = null;
  for (const listener of Array.from(listeners)) {
    try {
      listener(nowMs);
    } catch (error) {
      reportError(error);
    }
  }
  if (listeners.size > 0) {
    handle = requestAnimationFrame(tick);
  }
};

/**
 * Runs `listener` on every animation frame, on one loop shared by the whole
 * page. The loop stops when nothing is subscribed, and the browser pauses it
 * while the tab is hidden. Returns an unsubscribe function.
 */
export const subscribeFrame = (listener: FrameListener): (() => void) => {
  listeners.add(listener);
  if (handle === null && typeof requestAnimationFrame === "function") {
    handle = requestAnimationFrame(tick);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && handle !== null) {
      cancelAnimationFrame(handle);
      handle = null;
    }
  };
};
