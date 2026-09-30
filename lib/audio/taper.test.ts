import { describe, expect, it } from "vitest";

import {
  audioTaper,
  linearTaper,
  logTaper,
  resolveTaper,
} from "@/lib/audio/taper";

const positions = [0, 0.1, 0.25, 0.5, 0.74, 0.75, 0.9, 1];

describe("linearTaper", () => {
  it("maps the range evenly and round-trips", () => {
    const taper = linearTaper(-60, 6);
    expect(taper.toPosition(-60)).toBe(0);
    expect(taper.toPosition(6)).toBe(1);
    for (const position of positions) {
      expect(taper.toPosition(taper.toValue(position))).toBeCloseTo(
        position,
        6
      );
    }
  });

  it("puts silence at the bottom", () => {
    expect(linearTaper(-60, 6).toPosition(Number.NEGATIVE_INFINITY)).toBe(0);
  });
});

describe("audioTaper", () => {
  const taper = audioTaper({ maxDb: 6, minDb: -60, unityPosition: 0.75 });

  it("puts 0 dB at the unity position", () => {
    expect(taper.toPosition(0)).toBeCloseTo(0.75, 6);
    expect(taper.toValue(0.75)).toBeCloseTo(0, 6);
  });

  it("covers the full range", () => {
    expect(taper.toValue(0)).toBeCloseTo(-60, 6);
    expect(taper.toValue(1)).toBeCloseTo(6, 6);
  });

  it("is monotonic and invertible", () => {
    let previous = Number.NEGATIVE_INFINITY;
    for (const position of positions) {
      const value = taper.toValue(position);
      expect(value).toBeGreaterThan(previous);
      expect(taper.toPosition(value)).toBeCloseTo(position, 6);
      previous = value;
    }
  });

  it("gives more travel near unity than near the bottom", () => {
    const nearUnity = taper.toPosition(0) - taper.toPosition(-6);
    const nearBottom = taper.toPosition(-54) - taper.toPosition(-60);
    expect(nearUnity).toBeGreaterThan(nearBottom);
  });

  it("uses the whole travel below 0 dB when there is no headroom", () => {
    const noHeadroom = audioTaper({ maxDb: 0, minDb: -60 });
    expect(noHeadroom.toValue(1)).toBeCloseTo(0, 6);
    expect(noHeadroom.toPosition(0)).toBe(1);
  });
});

describe("logTaper", () => {
  it("spaces decades evenly", () => {
    const taper = logTaper(20, 20_000);
    expect(taper.toPosition(200)).toBeCloseTo(1 / 3, 6);
    expect(taper.toPosition(2000)).toBeCloseTo(2 / 3, 6);
    expect(taper.toValue(0.5)).toBeCloseTo(632.46, 1);
  });
});

describe("resolveTaper", () => {
  it("resolves names and passes objects through", () => {
    const custom = linearTaper(0, 1);
    expect(resolveTaper(custom, -60, 6)).toBe(custom);
    expect(resolveTaper("linear", -60, 6).toPosition(-27)).toBeCloseTo(0.5, 6);
    expect(resolveTaper("audio", -60, 6).toPosition(0)).toBeCloseTo(0.75, 6);
  });
});
