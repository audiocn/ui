import { describe, expect, it } from "vitest";

import { appendHistory } from "@/lib/audio/history";
import { createHistoryPlayback } from "@/lib/audio/history-playback";
import type { VisualFrame } from "@/lib/audio/types";

const frameOf = (): VisualFrame => ({
  bands: new Float32Array(1),
  history: new Float32Array(32),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
});

describe("history playback", () => {
  it("moves through known sample intervals instead of predicting the next one", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    appendHistory(frame, 0.25, 50, 50);
    playback.read(frame, 50);
    appendHistory(frame, 0.5, 116.6, 50);
    playback.read(frame, 116.6);
    appendHistory(frame, 0.75, 166.6, 50);
    playback.read(frame, 166.6);

    for (const nowMs of [180, 200, 216.5]) {
      const drawn = playback.read(frame, nowMs);
      expect(drawn.frame.historyLength).toBe(2);
      expect(drawn.progress).toBeCloseTo((nowMs - 100 - 50) / 66.6);
    }
    const boundary = playback.read(frame, 216.6);
    expect(boundary.frame.historyLength).toBe(3);
    expect(boundary.progress).toBeCloseTo(0);
    const next = playback.read(frame, 233.3);
    expect(next.progress).toBeCloseTo(16.7 / 50);
  });

  it("keeps the same travel on each frame with steady 64 ms samples", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    let previousPosition: number | undefined;
    for (let nowMs = 16; nowMs <= 1024; nowMs += 16) {
      appendHistory(frame, 0.5, nowMs, 50);
      const drawn = playback.read(frame, nowMs);
      const position = drawn.frame.historyLength + drawn.progress;
      if (nowMs >= 192 && previousPosition !== undefined) {
        expect(position - previousPosition).toBeCloseTo(16 / 64);
      }
      previousPosition = position;
    }
    expect(frame.historyLength).toBe(16);
  });

  it("copies mutable history only when a new sample arrives", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    appendHistory(frame, 0.25, 50, 50);
    const first = playback.read(frame, 50);
    frame.history[0] = 0.75;
    const same = playback.read(frame, 80);
    expect(same.frame).toBe(first.frame);
    expect(same.frame.history[0]).toBe(0.25);
    appendHistory(frame, 0.5, 100, 50);
    playback.read(frame, 100);
    const next = playback.read(frame, 180);
    expect(next.frame).not.toBe(first.frame);
    expect(next.frame.history[0]).toBe(0.75);
  });

  it("stops at the newest sample during a pause without adding held samples", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    appendHistory(frame, 0.25, 64, 50);
    playback.read(frame, 64);
    expect(playback.read(frame, 1000).progress).toBe(1);
    appendHistory(frame, 0.75, 10_000, 50);
    const resumed = playback.read(frame, 10_000);
    expect(resumed.frame.historyLength).toBe(2);
    expect(resumed.progress).toBeGreaterThan(0.98);
  });

  it("clears old samples when the timing or history resets", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    appendHistory(frame, 0.25, 50, 50);
    playback.read(frame, 50);
    appendHistory(frame, 0.5, 100, 50);
    playback.read(frame, 100);
    const replacement = frameOf();
    appendHistory(replacement, 1, 200, 100);
    const drawn = playback.read(replacement, 200);
    expect(drawn.frame.historyLength).toBe(1);
    expect(drawn.frame.history[0]).toBe(1);
    playback.clear();
    expect(playback.read(frame, 300).frame.history[0]).toBe(0.25);
  });

  it("leaves untimed frames unchanged", () => {
    const frame = frameOf();
    const playback = createHistoryPlayback();
    expect(playback.read(frame, 100)).toEqual({ frame, progress: 1 });
  });
});
