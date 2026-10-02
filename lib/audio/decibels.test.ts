import { describe, expect, it } from "vitest";

import {
  clampDb,
  dbToGain,
  dbToLevel,
  formatDb,
  gainToDb,
  levelToDb,
  peakDb,
  rmsDb,
} from "@/lib/audio/decibels";

describe("dbToGain and gainToDb", () => {
  it("round-trips common values", () => {
    for (const db of [-60, -20, -6, 0, 6]) {
      expect(gainToDb(dbToGain(db))).toBeCloseTo(db, 6);
    }
  });

  it("maps silence both ways", () => {
    expect(dbToGain(Number.NEGATIVE_INFINITY)).toBe(0);
    expect(gainToDb(0)).toBe(Number.NEGATIVE_INFINITY);
    expect(gainToDb(-1)).toBe(Number.NEGATIVE_INFINITY);
  });

  it("puts −6 dB at about half gain", () => {
    expect(dbToGain(-6)).toBeCloseTo(0.501, 3);
  });
});

describe("dbToLevel and levelToDb", () => {
  it("is linear in dB over the range", () => {
    expect(dbToLevel(-60)).toBe(0);
    expect(dbToLevel(-30)).toBe(0.5);
    expect(dbToLevel(0)).toBe(1);
  });

  it("clamps outside the range and handles silence", () => {
    expect(dbToLevel(-90)).toBe(0);
    expect(dbToLevel(6)).toBe(1);
    expect(dbToLevel(Number.NEGATIVE_INFINITY)).toBe(0);
    expect(dbToLevel(Number.NaN)).toBe(0);
  });

  it("inverts dbToLevel", () => {
    for (const db of [-48, -24, -12, -3]) {
      expect(levelToDb(dbToLevel(db))).toBeCloseTo(db, 6);
    }
    expect(levelToDb(0.5, -40, 0)).toBe(-20);
  });
});

describe("clampDb", () => {
  it("clamps to the range", () => {
    expect(clampDb(-80, -60, 6)).toBe(-60);
    expect(clampDb(12, -60, 6)).toBe(6);
  });

  it("keeps silence only when allowed", () => {
    expect(clampDb(Number.NEGATIVE_INFINITY, -60, 6)).toBe(-60);
    expect(
      clampDb(Number.NEGATIVE_INFINITY, -60, 6, { allowSilence: true })
    ).toBe(Number.NEGATIVE_INFINITY);
  });
});

describe("formatDb", () => {
  it("formats negative, positive and zero values", () => {
    expect(formatDb(-12.34)).toBe("−12.3 dB");
    expect(formatDb(3)).toBe("+3.0 dB");
    expect(formatDb(0)).toBe("0.0 dB");
  });

  it("never shows negative zero", () => {
    expect(formatDb(-0.04)).toBe("0.0 dB");
  });

  it("shows two hyphens, never a dash, when there is no reading", () => {
    expect(formatDb(Number.NaN)).toBe("-- dB");
    expect(formatDb(Number.NaN, { unit: false })).toBe("--");
    expect(formatDb(Number.NaN)).not.toContain("\u2014");
  });

  it("formats silence and values below the floor", () => {
    expect(formatDb(Number.NEGATIVE_INFINITY)).toBe("−∞ dB");
    expect(formatDb(-70, { floorDb: -60 })).toBe("−∞ dB");
  });

  it("respects decimals, unit and sign options", () => {
    expect(formatDb(-6, { decimals: 0, unit: false })).toBe("−6");
    expect(formatDb(6, { sign: "negative" })).toBe("6.0 dB");
    expect(formatDb(-6, { sign: "never" })).toBe("6.0 dB");
  });
});

describe("peakDb and rmsDb", () => {
  it("measures a full-scale square wave as 0 dBFS", () => {
    const square = Float32Array.from({ length: 64 }, (_, index) =>
      index % 2 === 0 ? 1 : -1
    );
    expect(peakDb(square)).toBeCloseTo(0, 6);
    expect(rmsDb(square)).toBeCloseTo(0, 6);
  });

  it("measures a sine's RMS 3 dB below its peak", () => {
    const sine = Float32Array.from({ length: 4800 }, (_, index) =>
      Math.sin((index / 48) * Math.PI * 2)
    );
    expect(peakDb(sine)).toBeCloseTo(0, 2);
    expect(rmsDb(sine)).toBeCloseTo(-3.01, 1);
  });

  it("returns silence for empty or silent input", () => {
    expect(peakDb(new Float32Array(8))).toBe(Number.NEGATIVE_INFINITY);
    expect(rmsDb(new Float32Array(0))).toBe(Number.NEGATIVE_INFINITY);
  });
});
