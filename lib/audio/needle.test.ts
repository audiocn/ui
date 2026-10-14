import { describe, expect, it } from "vitest";

import { createNeedle, resolveNeedle } from "@/lib/audio/needle";
import type { Needle } from "@/lib/audio/needle";

const FRAME_MS = 16;
const STOPS = { max: 1.04, min: -0.04 };

interface Swing {
  /** When the needle first reached 99% of the step, in ms. */
  settledMs: number;
  /** The furthest it went past the target, as a fraction of the step. */
  overshoot: number;
  /** Where it ended up. */
  final: number;
}

const swing = (
  needle: Needle,
  target: number,
  frameMs = FRAME_MS,
  durationMs = 1500
): Swing => {
  const start = needle.step(0, 0);
  let settledMs = Number.POSITIVE_INFINITY;
  let furthest = start;
  let position = start;
  for (let now = frameMs; now <= durationMs; now += frameMs) {
    position = needle.step(target, now);
    furthest = Math.max(furthest, position);
    if (settledMs === Number.POSITIVE_INFINITY && position >= target * 0.99) {
      settledMs = now;
    }
  }
  return {
    final: position,
    overshoot: (furthest - target) / (target - start),
    settledMs,
  };
};

describe("createNeedle", () => {
  it("rises to 99% in 300 ms with the vu preset", () => {
    const { settledMs } = swing(createNeedle("vu"), 0.6, 1);
    expect(settledMs).toBeGreaterThanOrEqual(285);
    expect(settledMs).toBeLessThanOrEqual(315);
  });

  it("overshoots a step by 1 to 1.5% and settles on it", () => {
    const { final, overshoot } = swing(createNeedle("vu"), 0.6);
    expect(overshoot).toBeGreaterThan(0.01);
    expect(overshoot).toBeLessThanOrEqual(0.015);
    expect(final).toBeCloseTo(0.6, 4);
  });

  it("moves the same with fast and slow frames", () => {
    const fast = swing(createNeedle("vu"), 0.6, 8);
    const slow = swing(createNeedle("vu"), 0.6, 33);
    expect(Math.abs(fast.settledMs - slow.settledMs)).toBeLessThanOrEqual(33);
    expect(slow.overshoot).toBeCloseTo(fast.overshoot, 2);
  });

  it("starts at rest against the low pin", () => {
    const needle = createNeedle("vu", STOPS);
    expect(needle.step(0.5, 0)).toBe(-0.04);
    expect(needle.step(0.5, FRAME_MS)).toBeGreaterThan(-0.04);
  });

  it("stops against the pins and loses its speed there", () => {
    const needle = createNeedle("vu", STOPS);
    let position = needle.step(5, 0);
    for (let now = FRAME_MS; now <= 1000; now += FRAME_MS) {
      position = needle.step(5, now);
      expect(position).toBeLessThanOrEqual(1.04);
    }
    expect(position).toBe(1.04);
    // Pinned with no speed left, it leaves the pin as soon as the level drops.
    expect(needle.step(0.5, 1000 + FRAME_MS)).toBeLessThan(1.04);
    for (let now = 1000; now <= 3000; now += FRAME_MS) {
      position = needle.step(-1, now);
      expect(position).toBeGreaterThanOrEqual(-0.04);
    }
    expect(position).toBe(-0.04);
  });

  it("jumps to the target after a long gap", () => {
    const needle = createNeedle("vu", STOPS);
    needle.step(0.2, 0);
    expect(needle.step(0.7, 2000)).toBe(0.7);
  });

  it("follows the target exactly with the instant preset", () => {
    const needle = createNeedle("instant", STOPS);
    expect(needle.step(0.3, 0)).toBe(0.3);
    expect(needle.step(0.8, FRAME_MS)).toBe(0.8);
    expect(needle.step(3, FRAME_MS * 2)).toBe(1.04);
  });

  it("goes back to rest on reset", () => {
    const needle = createNeedle("vu", STOPS);
    swing(needle, 0.8);
    needle.reset();
    expect(needle.step(0.8, 5000)).toBe(-0.04);
  });

  it("never overshoots with no overshoot asked for", () => {
    const { final, overshoot } = swing(createNeedle({ overshoot: 0 }), 0.5);
    expect(overshoot).toBeLessThanOrEqual(0);
    expect(final).toBeCloseTo(0.5, 3);
  });
});

describe("resolveNeedle", () => {
  it("merges partial options over the vu preset", () => {
    expect(resolveNeedle({ riseMs: 600 })).toEqual({
      overshoot: 0.015,
      riseMs: 600,
    });
  });
});
