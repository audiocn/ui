import { describe, expect, it } from "vitest";

import {
  bandsFromSpectrum,
  logBandEdges,
  resampleLevels,
} from "@/lib/audio/bands";

describe("logBandEdges", () => {
  it("covers the range with no gaps", () => {
    const edges = logBandEdges(10, 20, 20_000);
    expect(edges).toHaveLength(11);
    expect(edges[0]).toBeCloseTo(20, 3);
    expect(edges[10]).toBeCloseTo(20_000, 0);
    for (let index = 1; index < edges.length; index += 1) {
      expect(edges[index]).toBeGreaterThan(edges[index - 1] ?? 0);
    }
  });

  it("spaces edges by a constant ratio", () => {
    const edges = logBandEdges(3, 100, 100_000);
    expect(edges[1]).toBeCloseTo(1000, 0);
    expect(edges[2]).toBeCloseTo(10_000, 0);
  });
});

describe("bandsFromSpectrum", () => {
  it("maps loud bins to 1 and quiet bins to 0", () => {
    const spectrum = new Float32Array(1024).fill(-120);
    for (let bin = 0; bin < 20; bin += 1) {
      spectrum[bin] = -20;
    }
    const edges = logBandEdges(4, 40, 16_000);
    const bands = bandsFromSpectrum(
      spectrum,
      48_000,
      edges,
      new Float32Array(4)
    );
    expect(bands[0]).toBe(1);
    expect(bands[3]).toBe(0);
  });
});

describe("resampleLevels", () => {
  it("takes the loudest value in each span", () => {
    const out = resampleLevels([0.1, 0.9, 0.2, 0.4], 0, 4, new Float32Array(2));
    expect([...out]).toEqual([expect.closeTo(0.9, 5), expect.closeTo(0.4, 5)]);
  });

  it("reads a ring buffer from its start", () => {
    const out = resampleLevels([0.5, 0.1, 0.3], 1, 3, new Float32Array(3));
    expect([...out]).toEqual([
      expect.closeTo(0.1, 5),
      expect.closeTo(0.3, 5),
      expect.closeTo(0.5, 5),
    ]);
  });

  it("repeats values when there are more bars than values", () => {
    const out = resampleLevels([0.2, 0.8], 0, 2, new Float32Array(4));
    expect([...out]).toEqual([
      expect.closeTo(0.2, 5),
      expect.closeTo(0.2, 5),
      expect.closeTo(0.8, 5),
      expect.closeTo(0.8, 5),
    ]);
  });

  it("clears the output for empty input", () => {
    const out = resampleLevels([], 0, 0, new Float32Array([1, 1]));
    expect([...out]).toEqual([0, 0]);
  });
});
