import { describe, expect, it } from "vitest";

import { createBallistics, resolveBallistics } from "@/lib/audio/ballistics";

const FRAME_MS = 16;

const run = (
  ballistics: ReturnType<typeof createBallistics>,
  inputDb: number,
  fromMs: number,
  toMs: number
) => {
  let state = ballistics.step(inputDb, fromMs);
  for (let now = fromMs + FRAME_MS; now <= toMs; now += FRAME_MS) {
    state = ballistics.step(inputDb, now);
  }
  return state;
};

describe("createBallistics", () => {
  it("starts at the first reading", () => {
    const ballistics = createBallistics();
    expect(ballistics.step(-12, 0).db).toBeCloseTo(-12, 6);
  });

  it("rises quickly with the peak preset", () => {
    const ballistics = createBallistics("peak");
    ballistics.step(Number.NEGATIVE_INFINITY, 0);
    const state = run(ballistics, -6, FRAME_MS, 100);
    expect(state.db).toBeGreaterThan(-6.5);
  });

  it("falls at a steady rate in dB", () => {
    const ballistics = createBallistics("peak");
    run(ballistics, 0, 0, 100);
    const afterHalfSecond = run(ballistics, Number.NEGATIVE_INFINITY, 116, 600);
    const afterOneSecond = run(ballistics, Number.NEGATIVE_INFINITY, 616, 1100);
    const firstDrop = 0 - afterHalfSecond.db;
    const secondDrop = afterHalfSecond.db - afterOneSecond.db;
    expect(firstDrop).toBeGreaterThan(5);
    expect(secondDrop).toBeCloseTo(firstDrop, -1);
  });

  it("holds the peak before letting it fall", () => {
    const ballistics = createBallistics("peak");
    run(ballistics, -3, 0, 50);
    const held = run(ballistics, -40, 66, 1000);
    expect(held.holdDb).toBeCloseTo(-3, 1);
    const released = run(ballistics, -40, 1016, 4000);
    expect(released.holdDb).toBeLessThan(-30);
  });

  it("follows the input exactly with the instant preset", () => {
    const ballistics = createBallistics("instant");
    ballistics.step(-10, 0);
    expect(ballistics.step(-40, 16).db).toBeCloseTo(-40, 6);
    expect(ballistics.step(-40, 32).holdDb).toBeCloseTo(-40, 6);
  });

  it("forgets its state on reset", () => {
    const ballistics = createBallistics("peak");
    run(ballistics, 0, 0, 100);
    ballistics.reset();
    expect(ballistics.step(-30, 200).db).toBeCloseTo(-30, 6);
  });
});

describe("resolveBallistics", () => {
  it("merges partial options over the peak preset", () => {
    expect(resolveBallistics({ releaseMs: 100 })).toEqual({
      attackMs: 15,
      peakHoldMs: 1200,
      peakReleaseMs: 600,
      releaseMs: 100,
    });
  });
});
