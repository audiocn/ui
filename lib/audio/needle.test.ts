import { describe, expect, it } from "vitest";

import { dbToGain } from "@/lib/audio/decibels";
import {
  NEEDLE_PIN,
  NEEDLE_REST,
  needleConstants,
  restingNeedle,
  stepNeedle,
  VU_BALLISTICS,
} from "@/lib/audio/needle";
import type { NeedleOptions, NeedleScale } from "@/lib/audio/needle";
import { vuTaper } from "@/lib/audio/taper";

const SAMPLE_RATE = 48_000;
const SAMPLES_PER_MS = SAMPLE_RATE / 1000;
const TONE_HZ = 1000;
const SCALE: NeedleScale = { maxDb: 3, minDb: -10, referenceDb: -18 };
const ZERO_VU = vuTaper(-10, 3).toPosition(0);
/** On the stop, each half-cycle of the tone tugs the needle off it by less than this. */
const STOP_CHATTER = 1e-3;

/** A sine at `rmsDb` (null for silence) for `ms`, sampled once per millisecond. */
interface Segment {
  rmsDb: number | null;
  ms: number;
}

const simulate = (
  segments: Segment[],
  options: Partial<NeedleOptions> = {},
  scale: NeedleScale = SCALE
) => {
  const constants = needleConstants({ ...VU_BALLISTICS, ...options }, scale);
  const state = restingNeedle();
  const positions: number[] = [];
  let sample = 0;
  for (const { ms, rmsDb } of segments) {
    const peak = rmsDb === null ? 0 : dbToGain(rmsDb) * Math.SQRT2;
    for (let i = 0; i < ms * SAMPLES_PER_MS; i += 1, sample += 1) {
      const x = peak * Math.sin((2 * Math.PI * TONE_HZ * sample) / SAMPLE_RATE);
      stepNeedle(state, Math.abs(x), constants, 1 / SAMPLE_RATE);
      if (sample % SAMPLES_PER_MS === 0) {
        positions.push(state.position);
      }
    }
  }
  return positions;
};

const settled = (positions: number[]) => positions.at(-1) ?? Number.NaN;

/** Lowest position after the needle first reaches the upper stop. */
const deepestBounce = (positions: number[]) => {
  const hit = positions.findIndex((p) => p >= NEEDLE_PIN - 0.005);
  return hit === -1 ? Number.NaN : Math.min(...positions.slice(hit));
};

describe("needleConstants", () => {
  it("puts the stops just outside the printed scale", () => {
    const constants = needleConstants(VU_BALLISTICS, SCALE);
    expect(constants.lower).toBe(NEEDLE_REST);
    expect(constants.upper).toBe(NEEDLE_PIN);
    expect(NEEDLE_REST).toBeCloseTo(-0.04, 6);
    expect(NEEDLE_PIN).toBeCloseTo(1.04, 6);
  });

  it("defaults to a 2 Hz low-pass at Q 0.6 with half the speed kept on impact", () => {
    expect(VU_BALLISTICS).toEqual({
      lowPassCutoffHz: 2,
      lowPassQ: 0.6,
      rebound: 0.5,
    });
  });
});

describe("stepNeedle", () => {
  it("starts at rest against the lower stop", () => {
    expect(restingNeedle()).toEqual({ position: NEEDLE_REST, velocity: 0 });
  });

  it.each([
    [1, 0.5],
    [2, 0.6],
    [5, Math.SQRT1_2],
    [10, 1],
  ])(
    "settles on 0 VU at the reference level, at %s Hz and Q %s",
    (lowPassCutoffHz, lowPassQ) => {
      const positions = simulate([{ ms: 3000, rmsDb: -18 }], {
        lowPassCutoffHz,
        lowPassQ,
      });
      expect(settled(positions)).toBeCloseTo(ZERO_VU, 2);
    }
  );

  it("reads relative to referenceDb", () => {
    const positions = simulate(
      [{ ms: 3000, rmsDb: -18 }],
      {},
      {
        ...SCALE,
        referenceDb: -20,
      }
    );
    expect(settled(positions)).toBeCloseTo(vuTaper(-10, 3).toPosition(2), 2);
  });

  it("rises like a 2nd-order low-pass: 99% in about 340 ms, about 1% over", () => {
    const positions = simulate([{ ms: 2000, rmsDb: -18 }]);
    const final = settled(positions);
    const travel = final - NEEDLE_REST;
    const rise = positions.findIndex((p) => p >= NEEDLE_REST + 0.99 * travel);
    const overshoot = (Math.max(...positions) - final) / travel;
    expect(rise).toBeGreaterThan(320);
    expect(rise).toBeLessThan(360);
    expect(overshoot).toBeGreaterThan(0.005);
    expect(overshoot).toBeLessThan(0.015);
  });

  it("never overshoots at Q 0.5", () => {
    const positions = simulate([{ ms: 3000, rmsDb: -18 }], { lowPassQ: 0.5 });
    expect(Math.max(...positions)).toBeLessThanOrEqual(
      settled(positions) + 1e-6
    );
  });

  it("holds still on a steady tone", () => {
    const tail = simulate([{ ms: 3000, rmsDb: -18 }]).slice(-500);
    expect(Math.max(...tail) - Math.min(...tail)).toBeLessThan(1e-4);
  });

  it("rests on the upper stop for a steady tone just over the top", () => {
    const positions = simulate([{ ms: 2000, rmsDb: -14.5 }]);
    expect(NEEDLE_PIN - settled(positions)).toBeLessThan(STOP_CHATTER);
    expect(Math.max(...positions)).toBeLessThanOrEqual(NEEDLE_PIN);
  });

  it("bounces further off the upper stop the harder it hits", () => {
    const hard = deepestBounce(simulate([{ ms: 1000, rmsDb: -6 }]));
    const soft = deepestBounce(simulate([{ ms: 1000, rmsDb: -12 }]));
    expect(hard).toBeLessThan(soft);
    expect(soft).toBeLessThan(NEEDLE_PIN - STOP_CHATTER);
  });

  it("comes to rest on the upper stop after bouncing", () => {
    const positions = simulate([{ ms: 1500, rmsDb: -6 }]);
    const tail = positions.slice(-500);
    expect(NEEDLE_PIN - Math.min(...tail)).toBeLessThan(STOP_CHATTER);
  });

  it("stops dead on the upper stop with no rebound", () => {
    const positions = simulate([{ ms: 1000, rmsDb: -6 }], { rebound: 0 });
    expect(NEEDLE_PIN - deepestBounce(positions)).toBeLessThan(STOP_CHATTER);
  });

  it("falls back to rest in silence, never past the lower stop", () => {
    const positions = simulate([
      { ms: 1500, rmsDb: -18 },
      { ms: 1500, rmsDb: null },
    ]);
    expect(Math.min(...positions)).toBeGreaterThanOrEqual(NEEDLE_REST);
    expect(settled(positions)).toBe(NEEDLE_REST);
  });
});
