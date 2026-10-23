import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Mock } from "vitest";

import {
  DemoSignalProvider,
  createDemoSignal,
  useDemoSignal,
} from "@/hooks/use-demo-signal";
import { dbToGain, gainToDb } from "@/lib/audio/decibels";
import type { MeterFrame, VisualFrame } from "@/lib/audio/types";
import { createFakeInput } from "@/test/fake-audio";
import { advance, useFakeFrames } from "@/test/fake-frames";

const LIVE_SAMPLE = 0.5;
const TONE_DB = -12;

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

it("meters a live input through one gain stage, connected only while subscribed", () => {
  const { audio, input, node } = createFakeInput(LIVE_SAMPLE);
  const signal = createDemoSignal({ channels: 2, gainDb: -6, input: node });
  expect(input.connect).not.toHaveBeenCalled();

  const meter = vi.fn<(frame: MeterFrame) => void>();
  const visual = vi.fn<(frame: VisualFrame) => void>();
  const stopMeter = signal.meter.subscribe(meter);
  const stopVisual = signal.visual.subscribe(visual);
  expect(audio.gains).toHaveLength(1);
  const [stage] = audio.gains;
  expect(input.connect).toHaveBeenCalledWith(stage);
  expect(stage.gain.value).toBeCloseTo(dbToGain(-6));
  // A mono microphone reaches both sides of a stereo meter.
  expect(stage).toMatchObject({
    channelCount: 2,
    channelCountMode: "explicit",
    channelInterpretation: "speakers",
  });

  advance(32);
  expect(latest(meter).channels).toHaveLength(2);
  expect(latest(meter).channels[1].peakDb).toBeCloseTo(gainToDb(LIVE_SAMPLE));
  expect(latest(visual).peakDb).toBeCloseTo(gainToDb(LIVE_SAMPLE));

  signal.configure({ gainDb: 3 });
  expect(stage.gain.value).toBeCloseTo(dbToGain(3));
  signal.configure({ playing: false });
  expect(stage.gain.value).toBe(0);
  expect(audio.gains).toHaveLength(1);

  stopMeter();
  expect(input.disconnect).not.toHaveBeenCalled();
  stopVisual();
  expect(input.disconnect).toHaveBeenCalledWith(stage);
});

it("switches to a live input and back without dropping subscribers", () => {
  const { audio, input, node } = createFakeInput(LIVE_SAMPLE);
  const signal = createDemoSignal({ kind: "tone" });
  const meter = vi.fn<(frame: MeterFrame) => void>();
  const stop = signal.meter.subscribe(meter);
  try {
    advance(32);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(TONE_DB);

    signal.configure({ input: node });
    advance(32);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(gainToDb(LIVE_SAMPLE));

    // A new analysis shape rebuilds the nodes and releases the old ones.
    signal.configure({ bands: 16 });
    expect(audio.gains).toHaveLength(2);
    expect(input.disconnect).toHaveBeenCalledWith(audio.gains[0]);

    signal.configure({ input: null });
    expect(input.disconnect).toHaveBeenCalledWith(audio.gains[1]);
    advance(32);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(TONE_DB);
  } finally {
    stop();
  }
});

it("useDemoSignal meters the provider's input unless it passes its own", () => {
  const { input, node } = createFakeInput(LIVE_SAMPLE);
  const provided: { input: AudioNode | null } = { input: node };
  const wrapper = ({ children }: { children: ReactNode }) => (
    <DemoSignalProvider input={provided.input}>{children}</DemoSignalProvider>
  );
  const { result, rerender, unmount } = renderHook(
    ({ own }: { own?: AudioNode | null }) =>
      useDemoSignal({ input: own, kind: "tone" }),
    { initialProps: {}, wrapper }
  );
  const signal = result.current;
  const meter = vi.fn<(frame: MeterFrame) => void>();
  const stop = signal.meter.subscribe(meter);
  try {
    advance(32);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(gainToDb(LIVE_SAMPLE));

    rerender({ own: null });
    advance(32);
    expect(input.disconnect).toHaveBeenCalledTimes(1);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(TONE_DB);

    rerender({});
    advance(32);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(gainToDb(LIVE_SAMPLE));

    provided.input = null;
    rerender({});
    advance(32);
    expect(result.current).toBe(signal);
    expect(input.disconnect).toHaveBeenCalledTimes(2);
    expect(latest(meter).channels[0].peakDb).toBeCloseTo(TONE_DB);
  } finally {
    stop();
    unmount();
  }
});
