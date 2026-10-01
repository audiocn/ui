import { describe, expect, it } from "vitest";

import { appendHistory } from "@/lib/audio/history";
import type { VisualFrame } from "@/lib/audio/types";

const frameOf = (size = 32): VisualFrame => ({
  bands: new Float32Array(1),
  history: new Float32Array(size),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
});

const levels = (frame: VisualFrame) =>
  Array.from(
    { length: frame.historyLength },
    (_, index) =>
      frame.history[(frame.historyStart + index) % frame.history.length]
  );

describe("appendHistory", () => {
  it.each([60, 120, 144])("keeps the 50 ms cadence at %i Hz", (hz) => {
    const frame = frameOf();
    appendHistory(frame, 0.5, 0, 50);
    for (let tick = 1; tick <= hz; tick += 1) {
      appendHistory(frame, 0.5, (tick * 1000) / hz, 50);
    }
    expect(frame.historyLength).toBe(21);
    expect(frame.historyUpdatedAt).toBe(1000);
    expect(frame.historyIntervalMs).toBe(50);
  });

  it("keeps the remainder when a frame arrives after the sample boundary", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 0, 50);
    appendHistory(frame, 0.5, 64, 50);
    expect(frame.historyUpdatedAt).toBe(50);
    appendHistory(frame, 0.75, 112, 50);
    expect(frame.historyUpdatedAt).toBe(100);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
  });

  it("appends only the new sample after a pause and restarts the clock", () => {
    const frame = frameOf(3);
    appendHistory(frame, 0.25, 0, 50);
    appendHistory(frame, 0.5, 50, 50);
    appendHistory(frame, 0.75, 100_010, 50);
    expect(frame.historyUpdatedAt).toBe(100_010);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
    expect(frame.historyPreviousLevel).toBeUndefined();
    appendHistory(frame, 1, 100_074, 50);
    expect(frame.historyUpdatedAt).toBe(100_060);
    expect(levels(frame)).toEqual([0.5, 0.75, 1]);
    expect(frame.historyPreviousLevel).toBe(0.25);
  });

  it("does not fill the history with held values during repeated throttled frames", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 0, 50);
    appendHistory(frame, 0.5, 1000, 50);
    appendHistory(frame, 0.75, 2000, 50);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
    expect(frame.historyUpdatedAt).toBe(2000);
  });

  it("keeps the outgoing level when the ring wraps", () => {
    const frame = frameOf(2);
    appendHistory(frame, 0.25, 0, 50);
    appendHistory(frame, 0.5, 50, 50);
    appendHistory(frame, 0.75, 100, 50);
    expect(levels(frame)).toEqual([0.5, 0.75]);
    expect(frame.historyPreviousLevel).toBe(0.25);
  });

  it("resets timing when the interval changes", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 0, 50);
    appendHistory(frame, 0.5, 75, 100);
    expect(frame.historyUpdatedAt).toBe(75);
    expect(frame.historyIntervalMs).toBe(100);
    appendHistory(frame, 0.75, 100, 100);
    expect(levels(frame)).toEqual([0.25, 0.5]);
  });

  it("supports a zero interval and an empty history", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 0, 0);
    appendHistory(frame, 0.5, 16, 0);
    expect(levels(frame)).toEqual([0.25, 0.5]);
    const empty = frameOf(0);
    appendHistory(empty, 1, 50, 50);
    expect(empty.historyLength).toBe(0);
    expect(empty.historyStart).toBe(0);
  });
});
