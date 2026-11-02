import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Mock } from "vitest";

import { useMixer } from "@/hooks/use-mixer";
import { dbToGain, gainToDb } from "@/lib/audio/decibels";
import type { MeterFrame } from "@/lib/audio/types";
import { createDemoMixer, useDemoMixer } from "@/lib/docs/use-demo-mixer";
import { advance, useFakeFrames } from "@/test/fake-frames";

const CHANNELS = [
  { id: "mono", kind: "tone" },
  { channels: 2, id: "stereo", kind: "tone", seed: 3 },
] as const;

const latest = <T,>(listener: Mock<(frame: T) => void>): T => {
  const call = listener.mock.lastCall;
  if (!call) {
    throw new Error("Expected a signal frame");
  }
  return call[0];
};

beforeEach(useFakeFrames);
afterEach(() => vi.useRealTimers());

it("mixes current post-fader channels in linear units and applies master gain once", () => {
  const { result, unmount } = renderHook(() => {
    const mixer = useMixer({
      channels: [{ id: "mono" }, { gainDb: -6, id: "stereo" }],
    });
    return { demo: useDemoMixer(CHANNELS, mixer.state), mixer };
  });
  const original = result.current.demo;
  const master = vi.fn<(frame: MeterFrame) => void>();
  const mono = vi.fn<(frame: MeterFrame) => void>();
  const stereo = vi.fn<(frame: MeterFrame) => void>();
  // The master alone must start all upstream sources, before channel meters mount.
  const stops = [
    original.master.subscribe(master),
    original.sources.mono.subscribe(mono),
    original.sources.stereo.subscribe(stereo),
  ];
  try {
    advance(16);
    expect(latest(mono).channels[0].peakDb).toBeCloseTo(-12);
    expect(latest(stereo).channels[0].peakDb).toBeCloseTo(-18);
    for (const side of [0, 1]) {
      const input = latest(stereo).channels[side];
      const output = latest(master).channels[side];
      expect(output.peakDb).toBeCloseTo(
        gainToDb(dbToGain(-12) + dbToGain(input.peakDb))
      );
      expect(output.rmsDb).toBeCloseTo(
        gainToDb(Math.hypot(dbToGain(-15.01), dbToGain(input.rmsDb ?? 0)))
      );
    }
    act(() => {
      result.current.mixer.setGain("mono", -8);
      result.current.mixer.setMuted("stereo", true);
      result.current.mixer.setMasterGain(-4);
    });
    advance(16);
    expect(result.current.demo).toBe(original);
    expect(latest(mono).channels[0].peakDb).toBeCloseTo(-20);
    for (const level of latest(master).channels) {
      expect(level.peakDb).toBeCloseTo(-24);
      expect(level.rmsDb).toBeCloseTo(-27.01);
    }
    act(() => {
      result.current.mixer.setMuted("stereo", false);
      result.current.mixer.setSolo("mono", true);
    });
    advance(16);
    expect(latest(stereo).channels[0].peakDb).toBe(Number.NEGATIVE_INFINITY);
    expect(latest(master).channels[0].peakDb).toBeCloseTo(-24);
    act(() => result.current.mixer.setMuted("mono", true));
    advance(16);
    expect(latest(master).channels[0].peakDb).toBe(Number.NEGATIVE_INFINITY);
    act(() => {
      result.current.mixer.setMuted("mono", false);
      result.current.mixer.setMasterMuted(true);
    });
    advance(16);
    expect(latest(master).channels[0].peakDb).toBe(Number.NEGATIVE_INFINITY);
    expect(latest(mono).channels[0].peakDb).toBeCloseTo(-20);
    act(() => {
      result.current.mixer.setMasterMuted(false);
      result.current.mixer.setMasterGain(Number.NEGATIVE_INFINITY);
    });
    advance(16);
    expect(latest(master).channels[0].peakDb).toBe(Number.NEGATIVE_INFINITY);
  } finally {
    for (const stop of stops) {
      stop();
    }
    unmount();
  }
});

it("shares upstream subscriptions and stops and restarts with the last master subscriber", () => {
  const demo = createDemoMixer(CHANNELS);
  const first = vi.fn();
  const second = vi.fn();
  const stopFirst = demo.master.subscribe(first);
  const stopSecond = demo.master.subscribe(second);
  advance(32);
  expect(first).toHaveBeenCalledTimes(2);
  expect(second).toHaveBeenCalledTimes(2);
  stopFirst();
  advance(16);
  expect(second).toHaveBeenCalledTimes(3);
  stopSecond();
  expect(vi.getTimerCount()).toBe(0);
  const stopAgain = demo.master.subscribe(first);
  advance(16);
  expect(first).toHaveBeenCalledTimes(3);
  expect(first.mock.lastCall?.[0].channels[0].peakDb).toBeGreaterThan(-12);
  stopAgain();
  expect(vi.getTimerCount()).toBe(0);
});

it("emits stereo silence for an empty mixer", () => {
  const demo = createDemoMixer([]);
  const listener = vi.fn();
  const stop = demo.master.subscribe(listener);
  try {
    advance(16);
    expect(listener.mock.lastCall?.[0].channels).toEqual([
      { peakDb: Number.NEGATIVE_INFINITY, rmsDb: Number.NEGATIVE_INFINITY },
      { peakDb: Number.NEGATIVE_INFINITY, rmsDb: Number.NEGATIVE_INFINITY },
    ]);
  } finally {
    stop();
  }
});
