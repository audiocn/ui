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
  it.each([16, 1000 / 60, 1000 / 120, 1000 / 144])(
    "matches the original sample schedule with %f ms frames",
    (frameMs) => {
      const frame = frameOf();
      let lastSampleMs = 0;
      let expectedLength = 0;
      for (let tick = 1; tick * frameMs <= 1024; tick += 1) {
        const nowMs = tick * frameMs;
        if (nowMs - lastSampleMs >= 50) {
          lastSampleMs = nowMs;
          expectedLength += 1;
        }
        appendHistory(frame, 0.5, nowMs, 50);
        expect(frame.historyLength).toBe(expectedLength);
        if (expectedLength > 0) {
          expect(frame.historyUpdatedAt).toBe(lastSampleMs);
        }
      }
    }
  );

  it("does not carry late-frame time into the next sample interval", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 64, 50);
    appendHistory(frame, 0.5, 112, 50);
    expect(frame.historyUpdatedAt).toBe(64);
    expect(levels(frame)).toEqual([0.25]);
    appendHistory(frame, 0.5, 128, 50);
    expect(frame.historyUpdatedAt).toBe(128);
    expect(frame.historyIntervalMs).toBe(50);
    appendHistory(frame, 0.75, 192, 50);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
  });

  it("appends only the new sample after a pause and restarts the clock", () => {
    const frame = frameOf(3);
    appendHistory(frame, 0.25, 50, 50);
    appendHistory(frame, 0.5, 100, 50);
    appendHistory(frame, 0.75, 100_010, 50);
    expect(frame.historyUpdatedAt).toBe(100_010);
    expect(frame.historyIntervalMs).toBe(50);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
    expect(frame.historyPreviousLevel).toBeUndefined();
    appendHistory(frame, 1, 100_074, 50);
    expect(frame.historyUpdatedAt).toBe(100_074);
    expect(levels(frame)).toEqual([0.5, 0.75, 1]);
    expect(frame.historyPreviousLevel).toBe(0.25);
  });

  it("does not fill the history with held values during repeated throttled frames", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 50, 50);
    appendHistory(frame, 0.5, 1000, 50);
    appendHistory(frame, 0.75, 2000, 50);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
    expect(frame.historyUpdatedAt).toBe(2000);
  });

  it("keeps the outgoing level when the ring wraps", () => {
    const frame = frameOf(2);
    appendHistory(frame, 0.25, 50, 50);
    appendHistory(frame, 0.5, 100, 50);
    appendHistory(frame, 0.75, 150, 50);
    expect(levels(frame)).toEqual([0.5, 0.75]);
    expect(frame.historyPreviousLevel).toBe(0.25);
  });

  it("uses the new configured interval to decide when to sample", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 50, 50);
    appendHistory(frame, 0.5, 125, 100);
    expect(frame.historyUpdatedAt).toBe(50);
    expect(levels(frame)).toEqual([0.25]);
    appendHistory(frame, 0.5, 150, 100);
    expect(frame.historyIntervalMs).toBe(100);
    appendHistory(frame, 0.75, 174, 25);
    expect(levels(frame)).toEqual([0.25, 0.5]);
    appendHistory(frame, 0.75, 175, 25);
    expect(levels(frame)).toEqual([0.25, 0.5, 0.75]);
    expect(frame.historyIntervalMs).toBe(25);
  });

  it("supports a zero interval and an empty history", () => {
    const frame = frameOf();
    appendHistory(frame, 0.25, 0, 0);
    appendHistory(frame, 0.5, 16, 0);
    expect(levels(frame)).toEqual([0.25, 0.5]);
    expect(frame.historyIntervalMs).toBe(0);
    const empty = frameOf(0);
    appendHistory(empty, 1, 50, 50);
    expect(empty.historyLength).toBe(0);
    expect(empty.historyStart).toBe(0);
  });
});
