import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Mock } from "vitest";

import { createDemoSignal, useDemoSignal } from "@/hooks/use-demo-signal";
import type { MeterFrame, VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const latest = <T,>(listener: Mock<(frame: T) => void>): T => {
  const call = listener.mock.lastCall;
  if (!call) {
    throw new Error("Expected a signal frame");
  }
  return call[0];
};

beforeEach(useFakeFrames);
afterEach(() => vi.useRealTimers());

it("applies gain to stereo peak, RMS, bands, history and waveform without clipping boosts", () => {
  const dry = createDemoSignal({ channels: 2, kind: "tone" });
  const wet = createDemoSignal({ channels: 2, gainDb: -6, kind: "tone" });
  const dryMeter = vi.fn<(frame: MeterFrame) => void>();
  const wetMeter = vi.fn<(frame: MeterFrame) => void>();
  const dryVisual = vi.fn<(frame: VisualFrame) => void>();
  const wetVisual = vi.fn<(frame: VisualFrame) => void>();
  const stops = [
    dry.meter.subscribe(dryMeter),
    wet.meter.subscribe(wetMeter),
    dry.visual.subscribe(dryVisual),
    wet.visual.subscribe(wetVisual),
  ];
  try {
    advance(100);
    for (const side of [0, 1]) {
      const input = latest(dryMeter).channels[side];
      const output = latest(wetMeter).channels[side];
      expect(output.peakDb).toBeCloseTo(input.peakDb - 6);
      expect(output.rmsDb).toBeCloseTo((input.rmsDb ?? 0) - 6);
    }
    const input = latest(dryVisual);
    const output = latest(wetVisual);
    expect(output.peakDb).toBeCloseTo(input.peakDb - 6);
    expect(output.bands[0]).toBeLessThan(input.bands[0]);
    expect(output.history[0]).toBeLessThan(input.history[0]);
    expect(output.timeDomain?.[1]).toBeCloseTo(
      (input.timeDomain?.[1] ?? 0) * 10 ** (-6 / 20)
    );
    wet.configure({ gainDb: 18 });
    advance(16);
    expect(latest(wetMeter).channels[0].peakDb).toBeCloseTo(6);
    expect(latest(wetVisual).peakDb).toBeCloseTo(latest(dryVisual).peakDb + 18);
    wet.configure({ gainDb: Number.NEGATIVE_INFINITY });
    advance(64);
    expect(latest(wetMeter).channels).toEqual([
      { peakDb: Number.NEGATIVE_INFINITY, rmsDb: Number.NEGATIVE_INFINITY },
      { peakDb: Number.NEGATIVE_INFINITY, rmsDb: Number.NEGATIVE_INFINITY },
    ]);
    expect(latest(wetVisual).bands.every((value) => value === 0)).toBe(true);
    expect(latest(wetVisual).timeDomain?.every((value) => value === 0)).toBe(
      true
    );
  } finally {
    for (const stop of stops) {
      stop();
    }
  }
});

it("updates hook gain without replacing sources or resetting the signal pattern", () => {
  const { result, rerender, unmount } = renderHook(
    ({ gainDb }) => useDemoSignal({ gainDb, kind: "music", seed: 4 }),
    { initialProps: { gainDb: 0 } }
  );
  const signal = result.current;
  const reference = createDemoSignal({ kind: "music", seed: 4 });
  const input = vi.fn<(frame: MeterFrame) => void>();
  const output = vi.fn<(frame: MeterFrame) => void>();
  const stopInput = reference.meter.subscribe(input);
  const stopOutput = signal.meter.subscribe(output);
  try {
    advance(320);
    rerender({ gainDb: -12 });
    advance(16);
    expect(result.current).toBe(signal);
    expect(latest(output).channels[0].peakDb).toBeCloseTo(
      (latest(input).channels[0].peakDb ?? 0) - 12
    );
  } finally {
    stopInput();
    stopOutput();
    unmount();
  }
});
