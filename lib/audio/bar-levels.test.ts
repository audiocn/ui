import { describe, expect, it } from "vitest";

import { createBarLevels } from "@/lib/audio/bar-levels";
import type { BarLevelsOptions } from "@/lib/audio/bar-levels";

const FRAME_MS = 16.67;

const options = (overrides: Partial<BarLevelsOptions> = {}) => ({
  barCount: 4,
  idle: "static" as const,
  loading: false,
  minLevel: 0,
  mirrored: false,
  reducedMotion: false,
  ...overrides,
});

describe("createBarLevels", () => {
  it("resamples input to the bar count", () => {
    const bars = createBarLevels(options({ barCount: 2 }));
    bars.step(FRAME_MS, [0.2, 0.6, 1, 0.4]);
    expect([...bars.levels]).toEqual([
      expect.closeTo(0.6, 5),
      expect.closeTo(1, 5),
    ]);
  });

  it("rises instantly and releases smoothly", () => {
    const bars = createBarLevels(options({ barCount: 1 }));
    bars.step(FRAME_MS, [1]);
    expect(bars.levels[0]).toBe(1);
    bars.step(FRAME_MS * 2, [0]);
    expect(bars.levels[0]).toBeCloseTo(0.86, 2);
  });

  it("settles once every bar reaches its target", () => {
    const bars = createBarLevels(options({ barCount: 1 }));
    bars.step(FRAME_MS, [1]);
    expect(bars.settled).toBe(true);

    let nowMs = FRAME_MS;
    nowMs += FRAME_MS;
    bars.step(nowMs, [0]);
    expect(bars.settled).toBe(false);
    for (let frame = 0; frame < 120; frame += 1) {
      nowMs += FRAME_MS;
      bars.step(nowMs, [0]);
    }
    expect(bars.settled).toBe(true);
  });

  it("never settles while an idle animation or the loading sweep runs", () => {
    const pulsing = createBarLevels(options({ idle: "pulse" }));
    const loading = createBarLevels(options({ loading: true }));
    for (let frame = 1; frame <= 120; frame += 1) {
      pulsing.step(frame * FRAME_MS, null);
      loading.step(frame * FRAME_MS, [0.5]);
    }
    expect(pulsing.settled).toBe(false);
    expect(loading.settled).toBe(false);

    // A signal replaces the idle animation, so live bars can settle.
    pulsing.step(121 * FRAME_MS, [0.5, 0.5, 0.5, 0.5]);
    expect(pulsing.settled).toBe(true);
  });

  it("drops at once with reduced motion", () => {
    const bars = createBarLevels(options({ barCount: 1, reducedMotion: true }));
    bars.step(FRAME_MS, [1]);
    bars.step(FRAME_MS * 2, [0]);
    expect(bars.levels[0]).toBe(0);
  });

  it("never shows less than the resting level", () => {
    const bars = createBarLevels(options({ minLevel: 0.1 }));
    bars.step(FRAME_MS, null);
    expect([...bars.levels]).toEqual([0.1, 0.1, 0.1, 0.1].map(Math.fround));
  });

  it("reports whether the input is above the signal floor", () => {
    const bars = createBarLevels(options());
    expect(bars.step(FRAME_MS, [0, 0.01, 0, 0])).toBe(false);
    expect(bars.step(FRAME_MS * 2, [0, 0.5, 0, 0])).toBe(true);
  });

  it("mirrors lows into the centre", () => {
    const bars = createBarLevels(options({ barCount: 5, mirrored: true }));
    bars.step(FRAME_MS, [1, 0.5, 0]);
    const [first, second, middle, fourth, last] = bars.levels;
    expect(middle).toBe(1);
    expect(second).toBe(fourth);
    expect(first).toBe(last);
  });

  it("animates the idle wave only without a signal", () => {
    const bars = createBarLevels(options({ idle: "wave" }));
    bars.step(1000, null);
    expect(Math.max(...bars.levels)).toBeGreaterThan(0);
    const loud = createBarLevels(options({ idle: "wave" }));
    loud.step(1000, [0.5, 0.5, 0.5, 0.5]);
    expect([...loud.levels]).toEqual([0.5, 0.5, 0.5, 0.5]);
  });

  it("sweeps while loading", () => {
    const bars = createBarLevels(options({ barCount: 8, loading: true }));
    bars.step(1000, null);
    expect(Math.max(...bars.levels)).toBeGreaterThan(0.1);
  });
});
