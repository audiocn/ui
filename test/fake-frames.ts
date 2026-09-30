import { act } from "@testing-library/react";
import { vi } from "vitest";

/** Fake timers that also drive requestAnimationFrame and performance.now. */
export const useFakeFrames = () => {
  vi.useFakeTimers({
    toFake: [
      "setTimeout",
      "clearTimeout",
      "setInterval",
      "clearInterval",
      "requestAnimationFrame",
      "cancelAnimationFrame",
      "performance",
      "Date",
    ],
  });
};

/** Advances fake time inside act(), running animation frames and timers. */
export const advance = (ms: number) => {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
};
