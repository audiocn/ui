type FrameCallback = (nowMs: number) => void;

const callbacks = new Set<FrameCallback>();
let handle: number | null = null;

const reportError = (error: unknown) => {
  queueMicrotask(() => {
    throw error;
  });
};

const tick = (nowMs: number) => {
  handle = null;
  for (const callback of [...callbacks]) {
    try {
      callback(nowMs);
    } catch (error) {
      reportError(error);
    }
  }
  if (callbacks.size > 0) {
    handle = requestAnimationFrame(tick);
  }
};

/**
 * Runs `callback` on every animation frame, on one loop shared by the whole
 * page. The loop stops when nothing is subscribed, and the browser pauses it
 * while the tab is hidden. Returns an unsubscribe function.
 */
export const subscribeFrame = (callback: FrameCallback): (() => void) => {
  callbacks.add(callback);
  if (handle === null && typeof requestAnimationFrame === "function") {
    handle = requestAnimationFrame(tick);
  }
  return () => {
    callbacks.delete(callback);
    if (callbacks.size === 0 && handle !== null) {
      cancelAnimationFrame(handle);
      handle = null;
    }
  };
};
