import { describe, expect, it } from "vitest";

import type { VisualFrame } from "@/lib/audio/types";
import { createWaveLine, triggerIndex } from "@/lib/audio/wave-line";
import type { WaveLine, WaveLineOptions } from "@/lib/audio/wave-line";

const FRAME_MS = 16;
const POINTS = 65;

const frameOf = (bands: number[], timeDomain?: number[]): VisualFrame => ({
  bands: Float32Array.from(bands),
  history: new Float32Array(1),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
  timeDomain: timeDomain ? Float32Array.from(timeDomain) : undefined,
});

const sine = (length: number, cycles: number, phase: number, gain = 1) =>
  Array.from(
    { length },
    (_, index) =>
      gain * Math.sin((2 * Math.PI * cycles * index) / length + phase)
  );

const steadyTone = () => frameOf([0], sine(256, 4, 0, 0.9));

const options = (
  overrides: Partial<WaveLineOptions> = {}
): WaveLineOptions => ({
  loading: false,
  mode: "wave",
  reducedMotion: false,
  sensitivity: 1,
  ...overrides,
});

const run = (
  line: WaveLine,
  frameAt: (frame: number) => VisualFrame | null,
  frames: number
) => {
  let active = false;
  for (let frame = 1; frame <= frames; frame += 1) {
    active = line.step(frame * FRAME_MS, frameAt(frame), POINTS);
  }
  return active;
};

const heightsOf = (line: WaveLine) => line.heights.slice(0, line.count);

/** The scope line of a steady tone that starts at `phase`. */
const scopeShape = (phase: number) => {
  const line = createWaveLine(options({ mode: "scope", reducedMotion: true }));
  run(line, () => frameOf([0], sine(256, 8, phase, 0.9)), 1);
  return heightsOf(line);
};

/** Index of the highest point of a loading line after `frames` frames. */
const pulseAt = (frames: number) => {
  const line = createWaveLine(options({ loading: true }));
  run(line, () => null, frames);
  const heights = heightsOf(line).map(Math.abs);
  return heights.indexOf(Math.max(...heights));
};

describe("triggerIndex", () => {
  it("finds the first rising zero crossing, between samples", () => {
    const samples = [0.5, 0.2, -0.3, 0.1, 0.4, 0.6, 0.2, -0.2];
    expect(triggerIndex([...samples, ...samples])).toBeCloseTo(2.75, 5);
  });

  it("starts at 0 when nothing crosses early", () => {
    expect(triggerIndex([0.5, 0.4, 0.3, 0.2, 0.1, 0, -0.1, -0.2])).toBe(0);
  });
});

describe("createWaveLine", () => {
  it("uses the requested number of points", () => {
    const line = createWaveLine(options());
    line.step(FRAME_MS, null, 101);
    expect(line.count).toBe(101);
  });

  it("rests flat with no signal", () => {
    const line = createWaveLine(options());
    expect(run(line, () => null, 30)).toBe(false);
    expect(Math.max(...heightsOf(line).map(Math.abs))).toBe(0);
    expect(line.loudness).toBe(0);
  });

  it("shapes a wave from the bands and reports a signal", () => {
    const line = createWaveLine(options());
    expect(run(line, () => frameOf([1, 1, 1, 1, 1, 1, 1, 1]), 30)).toBe(true);
    const heights = heightsOf(line);
    expect(Math.max(...heights)).toBeGreaterThan(0.2);
    expect(Math.min(...heights)).toBeLessThan(-0.2);
    for (const height of heights) {
      expect(Math.abs(height)).toBeLessThanOrEqual(1);
    }
    expect(line.loudness).toBeGreaterThan(0.2);
  });

  it("eases toward a new shape, and jumps with reduced motion", () => {
    const eased = createWaveLine(options({ mode: "scope" }));
    const still = createWaveLine(
      options({ mode: "scope", reducedMotion: true })
    );
    run(eased, steadyTone, 1);
    run(still, steadyTone, 1);
    expect(eased.peak).toBeCloseTo(still.peak, 5);
    expect(Math.max(...heightsOf(eased))).toBeLessThan(
      Math.max(...heightsOf(still))
    );
  });

  it("holds a steady tone still in scope mode, whatever its phase", () => {
    const first = scopeShape(0);
    const shifted = scopeShape(1.3);
    for (const [index, height] of first.entries()) {
      expect(shifted[index]).toBeCloseTo(height, 1);
    }
  });

  it("lifts a quiet signal in scope mode", () => {
    const line = createWaveLine(
      options({ mode: "scope", reducedMotion: true })
    );
    run(line, () => frameOf([0], sine(256, 4, 0, 0.1)), 1);
    expect(Math.max(...heightsOf(line))).toBeGreaterThan(0.3);
  });

  it("runs a pulse left to right while loading, and reports no signal", () => {
    expect(pulseAt(40)).toBeGreaterThan(pulseAt(20));
    const line = createWaveLine(options({ loading: true }));
    expect(run(line, () => frameOf([1, 1, 1, 1]), 30)).toBe(false);
  });
});
