import type { VisualFrame } from "@/lib/audio/types";

export const appendHistory = (
  frame: VisualFrame,
  level: number,
  nowMs: number,
  intervalMs: number
): void => {
  const interval = Number.isFinite(intervalMs) ? Math.max(0, intervalMs) : 0;
  const reset =
    frame.historyUpdatedAt === undefined ||
    frame.historyLength === 0 ||
    frame.historyIntervalMs !== interval;
  const elapsed = nowMs - (frame.historyUpdatedAt ?? nowMs);
  const steps = reset || interval === 0 ? 1 : Math.floor(elapsed / interval);
  frame.historyIntervalMs = interval;
  if (steps < 1 || frame.history.length === 0) {
    return;
  }

  frame.historyUpdatedAt =
    reset || interval === 0 || steps > 1 ? nowMs : nowMs - (elapsed % interval);
  const size = frame.history.length;
  if (frame.historyLength < size) {
    frame.history[(frame.historyStart + frame.historyLength) % size] = level;
    frame.historyLength += 1;
  } else {
    frame.historyPreviousLevel = frame.history[frame.historyStart];
    frame.history[frame.historyStart] = level;
    frame.historyStart = (frame.historyStart + 1) % size;
  }
};
