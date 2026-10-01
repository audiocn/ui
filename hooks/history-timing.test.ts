import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAnalyserTap } from "@/hooks/use-audio-analyser";
import { createDemoSignal } from "@/hooks/use-demo-signal";
import type { VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const createTestTap = () => {
  const analyser = {
    frequencyBinCount: 16,
    getFloatFrequencyData: (data: Float32Array) => data.fill(-24),
    getFloatTimeDomainData: (data: Float32Array) => data.fill(0.25),
  };
  const context = {
    createAnalyser: () => analyser,
    sampleRate: 48_000,
  } as unknown as BaseAudioContext;
  const node = {
    connect: vi.fn(),
    disconnect: vi.fn(),
  } as unknown as AudioNode;
  return createAnalyserTap(context, node, { historyIntervalMs: 50 });
};

describe("built-in history timing", () => {
  it.each(["demo", "analyser"] as const)(
    "%s sends history timing and keeps the sample schedule",
    (kind) => {
      const source = kind === "demo" ? createDemoSignal() : createTestTap();
      const frames: { count: number; interval?: number; time?: number }[] = [];
      const unsubscribe = source.visual.subscribe((frame: VisualFrame) => {
        frames.push({
          count: frame.historyLength,
          interval: frame.historyIntervalMs,
          time: frame.historyUpdatedAt,
        });
      });
      try {
        advance(1024);
        expect(frames[0]).toEqual({ count: 1, interval: 50, time: 16 });
        expect(frames.at(-1)).toEqual({ count: 21, interval: 50, time: 1016 });
        expect(new Set(frames.map((frame) => frame.time)).size).toBe(21);
        expect(frames.length).toBeGreaterThan(21);
      } finally {
        unsubscribe();
        if ("dispose" in source) {
          source.dispose();
        }
      }
    }
  );

  it.each(["demo", "analyser"] as const)(
    "%s skips missed samples when frames are throttled and resumes its cadence",
    (kind) => {
      let tick: FrameRequestCallback | undefined;
      vi.stubGlobal("requestAnimationFrame", (next: FrameRequestCallback) => {
        tick = next;
        return 1;
      });
      vi.stubGlobal("cancelAnimationFrame", () => {
        tick = undefined;
      });
      const source = kind === "demo" ? createDemoSignal() : createTestTap();
      const listener = vi.fn<(frame: VisualFrame) => void>();
      const unsubscribe = source.visual.subscribe(listener);
      try {
        for (const nowMs of [16, 80, 1017, 2018, 100_019]) {
          tick?.(nowMs);
        }
        expect(listener.mock.lastCall?.[0]).toMatchObject({
          historyLength: 5,
          historyUpdatedAt: 100_019,
        });
        tick?.(100_035);
        expect(listener.mock.lastCall?.[0].historyLength).toBe(5);
        tick?.(100_083);
        expect(listener.mock.lastCall?.[0]).toMatchObject({
          historyLength: 6,
          historyUpdatedAt: 100_069,
        });
      } finally {
        unsubscribe();
        if ("dispose" in source) {
          source.dispose();
        }
      }
    }
  );

  it("resets demo history timing when the history size changes", () => {
    const signal = createDemoSignal({ historySize: 2 });
    const listener = vi.fn<(frame: VisualFrame) => void>();
    const unsubscribe = signal.visual.subscribe(listener);
    try {
      advance(160);
      expect(listener.mock.lastCall?.[0].historyPreviousLevel).toBeDefined();
      signal.configure({ historyIntervalMs: 100, historySize: 4 });
      advance(16);
      expect(listener.mock.lastCall?.[0]).toMatchObject({
        historyIntervalMs: 100,
        historyLength: 1,
        historyPreviousLevel: undefined,
        historyStart: 0,
        historyUpdatedAt: 176,
      });
    } finally {
      unsubscribe();
    }
  });
});
