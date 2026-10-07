import { describe, expect, it } from "vitest";

import { createWheelStepper } from "@/lib/audio/wheel";

const DOM_DELTA_LINE = 1;

interface Sample {
  t: number;
  deltaY?: number;
  deltaX?: number;
  deltaMode?: number;
  wheelDeltaY?: number;
  wheelDeltaX?: number;
}

const play = (trace: Sample[]) => {
  const step = createWheelStepper();
  return trace.map(({ t, deltaY = 0, deltaX = 0, deltaMode = 0, ...legacy }) =>
    step({ deltaMode, deltaX, deltaY, timeStamp: t, ...legacy })
  );
};

/** Pixel deltas down the page, `every` ms apart, as a trackpad sends them. */
const stream = (deltas: number[], every = 8): Sample[] =>
  deltas.map((deltaY, index) => ({ deltaY, t: index * every }));

const repeat = (delta: number, times: number) =>
  Array.from({ length: times }, () => delta);

const notchAt = (t: number, deltaY = 3): Sample => ({
  deltaMode: DOM_DELTA_LINE,
  deltaY,
  t,
});

const turns = (steps: number[]) => steps.filter((step) => step !== 0).length;

describe("createWheelStepper", () => {
  it("steps up for an upward wheel and down for a downward one", () => {
    expect(play([{ deltaY: -100, t: 0 }])).toEqual([1]);
    expect(play([{ deltaY: 100, t: 0 }])).toEqual([-1]);
  });

  it("steps once per notch reported in lines, however small", () => {
    const notches = [0, 100, 200].map((t) => notchAt(t, 1));
    expect(play(notches)).toEqual([-1, -1, -1]);
  });

  it("steps once per legacy notch of 120, however small the pixel delta", () => {
    const notches = [0, 60, 120].map((t) => ({
      deltaY: -4,
      t,
      wheelDeltaY: 120,
    }));
    expect(play(notches)).toEqual([1, 1, 1]);
  });

  it("steps once for a notch, however large", () => {
    expect(play([{ deltaY: 300, t: 0, wheelDeltaY: -360 }])).toEqual([-1]);
  });

  it("reads a large delta after a pause as a notch, which leaves nothing in hand", () => {
    const afterPause = [
      { deltaY: 20, t: 0 },
      { deltaY: 50, t: 60 },
      { deltaY: 20, t: 70 },
    ];
    const inStream = [
      { deltaY: 20, t: 0 },
      { deltaY: 50, t: 30 },
      { deltaY: 20, t: 40 },
    ];
    expect(play(afterPause)).toEqual([0, -1, 0]);
    expect(play(inStream)).toEqual([0, -1, -1]);
  });

  it("reads a macOS trackpad delta whose legacy value lands on 120 as smooth", () => {
    // Chrome and Safari report trackpads as wheelDeltaY = -3 × deltaY.
    const trackpad = [0, 10, 20, 30, 40].map((t) => ({
      deltaY: -40,
      t,
      wheelDeltaY: 120,
    }));
    const wheel = trackpad.map((sample) => ({ ...sample, deltaY: -100 }));
    expect(play(trackpad)).toEqual([1, 1, 1, 1, 1]);
    expect(play(wheel)).toEqual([1, 1, 1, 0, 0]);
  });

  it("reads the horizontal axis when there is no vertical delta", () => {
    expect(play([{ deltaX: -4, t: 0, wheelDeltaX: 120 }])).toEqual([1]);
    expect(play(stream([10, 10, 10]))).toEqual(
      play([10, 10, 10].map((deltaX, index) => ({ deltaX, t: index * 8 })))
    );
  });

  it("adds up trackpad deltas to one step per 30 px", () => {
    expect(play(stream(repeat(10, 6), 16))).toEqual([0, 0, -1, 0, 0, -1]);
  });

  it("takes at most one step per event and keeps less than a step in hand", () => {
    expect(play(stream([10, 100, 1, 1], 10))).toEqual([0, -1, -1, 0]);
  });

  it("forgets a partial step after 150 ms without events", () => {
    expect(play(stream([20, 20], 200))).toEqual([0, 0]);
    expect(play(stream([20, 20], 100))).toEqual([0, -1]);
  });

  it("forgets a partial step when the wheel turns back", () => {
    expect(play(stream([25, -10, 25], 10))).toEqual([0, 0, 0]);
    expect(play(stream([25, 10, 25], 10))).toEqual([0, -1, -1]);
  });

  it("stops early in a dense decaying momentum stream", () => {
    const swipe = [4, 12, 24, 36, 40, 38, 35, 31, 27, 23, 19, 16, 13, 11, 9];
    const tail = [7, 6, 5, 4, 3, 2, 2, 1, 1];
    const steps = play(stream([...swipe, ...tail]));
    const peak = swipe.indexOf(40);
    expect(turns(steps.slice(0, peak + 1))).toBe(3);
    expect(turns(steps.slice(peak + 1))).toBe(2);
  });

  it("ignores the plateaued tail of a real trackpad swipe", () => {
    const gesture = [3, 6, 8, 8, 7, 7, 6, 6, 5];
    const tail = [4, 4, 4, ...repeat(3, 11), ...repeat(2, 6)];
    const steps = play(stream([...gesture, ...tail]));
    expect(turns(steps.slice(0, gesture.length))).toBe(1);
    expect(turns(steps.slice(gesture.length))).toBe(0);
  });

  it("ignores a jittery tail at half the peak or less", () => {
    const gesture = [4, 8, 10, 10, 8, 9, 6, 7, 5, 6, 4, 5, 3, 4];
    const tail = Array.from({ length: 8 }, () => [3, 2]).flat();
    const steps = play(stream([...gesture, ...tail]));
    expect(turns(steps.slice(gesture.length))).toBe(0);
  });

  it("never locks events 40 ms or more apart", () => {
    expect(turns(play(stream([40, 38, 35, 31, 27, 23], 45)))).toBe(6);
  });

  it("keeps stepping when a hand slows down sharply", () => {
    const deltas = [30, 30, 30, ...repeat(10, 12)];
    expect(turns(play(stream(deltas, 16)))).toBe(7);
  });

  it("never locks a slow steady drag, even as it eases off", () => {
    const wobbly = Array.from({ length: 12 }, () => [3, 3, 4, 3, 2]).flat();
    const easing = [...repeat(3, 20), ...repeat(2, 30)];
    expect(turns(play(stream(wobbly, 16)))).toBe(6);
    expect(turns(play(stream(easing, 16)))).toBe(4);
  });

  it("steps on every notch of fast notch-by-notch scrolling", () => {
    const notches = [0, 30, 60, 90, 120, 150].map((t) => notchAt(t));
    expect(play(notches)).toEqual(repeat(-1, 6));
  });

  describe("a free-spinning wheel", () => {
    const spin = [0, 20, 40, 60, 80].map((t) => notchAt(t));

    it("locks after three notches under 25 ms apart", () => {
      expect(play(spin)).toEqual([-1, -1, -1, 0, 0]);
    });

    it("unlocks after 120 ms of silence", () => {
      expect(play([...spin, notchAt(180)]).at(-1)).toBe(0);
      expect(play([...spin, notchAt(210)]).at(-1)).toBe(-1);
    });

    it("unlocks when the wheel turns back", () => {
      expect(play([...spin, notchAt(100, -3)]).at(-1)).toBe(1);
    });
  });

  it("counts afresh when the hand pushes again after momentum", () => {
    const fading = [20, 30, 40, 30, 22, 16, 12, 9];
    const coasting = repeat(9, 4);
    const push = [13, 20];
    const steps = play(stream([...fading, ...coasting, ...push]));
    expect(turns(steps.slice(fading.length, -push.length))).toBe(0);
    expect(steps.slice(-push.length)).toEqual([0, -1]);
  });
});
