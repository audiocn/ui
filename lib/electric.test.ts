import { describe, expect, it } from "vitest";

import {
  createElectricSparks,
  createRandom,
  displace,
  emitSpark,
  moveSparks,
} from "@/lib/electric";

/** Total movement along a displaced profile. */
const spread = (roughness: number) => {
  const out = new Float32Array(257);
  displace(out, 0, 257, [0, 0], createRandom(9), roughness);
  let total = 0;
  for (let index = 1; index < out.length; index += 1) {
    total += Math.abs((out[index] ?? 0) - (out[index - 1] ?? 0));
  }
  return total;
};

describe("createRandom", () => {
  it("repeats for a seed and stays within 0..1", () => {
    const first = createRandom(3);
    const second = createRandom(3);
    const other = createRandom(4);
    const a = Array.from({ length: 50 }, first);
    expect(Array.from({ length: 50 }, second)).toEqual(a);
    expect(Array.from({ length: 50 }, other)).not.toEqual(a);
    for (const value of a) {
      expect(value).toBeGreaterThanOrEqual(0);
      expect(value).toBeLessThan(1);
    }
  });
});

describe("displace", () => {
  it("keeps the ends and bends everything between", () => {
    const out = new Float32Array(20);
    displace(out, 2, 17, [0.25, -0.5], createRandom(1), 0.6);
    expect(out[2]).toBe(0.25);
    expect(out[18]).toBe(-0.5);
    expect(out[0]).toBe(0);
    expect(out[19]).toBe(0);
    expect(out.slice(3, 18).some((value) => value !== 0)).toBe(true);
  });

  it("bends less with a lower roughness", () => {
    expect(spread(0.3)).toBeLessThan(spread(0.8));
  });
});

describe("electric sparks", () => {
  it("fly, fall and fade", () => {
    const sparks = createElectricSparks(4);
    emitSpark(sparks, { lifeMs: 100, vx: 10, vy: -50, x: 5, y: 5 });
    moveSparks(sparks, 0.05, 100);
    expect(sparks.x[0]).toBeCloseTo(5.5, 5);
    expect(sparks.vy[0]).toBeCloseTo(-45, 5);
    moveSparks(sparks, 0.06, 100);
    expect(sparks.life[0]).toBe(0);
  });

  it("replace the oldest when the pool is full", () => {
    const sparks = createElectricSparks(2);
    for (const x of [1, 2, 3]) {
      emitSpark(sparks, { lifeMs: 100, vx: 0, vy: 0, x, y: 0 });
    }
    expect([...sparks.x]).toEqual([3, 2]);
  });
});
