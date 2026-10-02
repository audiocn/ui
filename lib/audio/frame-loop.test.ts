import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createFrameTask,
  createPainterClock,
  MAX_FRAME_GAP_MS,
} from "@/lib/audio/frame-loop";
import { useFakeFrames } from "@/test/fake-frames";

const FRAME_MS = 16;

describe("createFrameTask", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("runs while the step needs frames, then requests none", () => {
    let remaining = 3;
    const step = vi.fn(() => {
      remaining -= 1;
      return remaining > 0;
    });
    const task = createFrameTask(step);

    vi.advanceTimersByTime(FRAME_MS * 10);
    expect(step).toHaveBeenCalledTimes(3);
    expect(vi.getTimerCount()).toBe(0);
    task.stop();
  });

  it("wakes a sleeping task, and ignores a wake while it runs", () => {
    const step = vi.fn(() => false);
    const task = createFrameTask(step);
    vi.advanceTimersByTime(FRAME_MS * 4);
    expect(step).toHaveBeenCalledTimes(1);

    task.wake();
    task.wake();
    vi.advanceTimersByTime(FRAME_MS * 4);
    expect(step).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
    task.stop();
  });

  it("never runs again once stopped", () => {
    const step = vi.fn(() => true);
    const task = createFrameTask(step);
    vi.advanceTimersByTime(FRAME_MS * 2);
    const calls = step.mock.calls.length;

    task.stop();
    task.wake();
    vi.advanceTimersByTime(FRAME_MS * 4);
    expect(step).toHaveBeenCalledTimes(calls);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps the shared loop alive only for tasks that are awake", () => {
    const sleeper = createFrameTask(() => false);
    const runner = vi.fn(() => true);
    const awake = createFrameTask(runner);
    vi.advanceTimersByTime(FRAME_MS * 3);
    expect(vi.getTimerCount()).toBe(1);

    awake.stop();
    vi.advanceTimersByTime(FRAME_MS * 2);
    expect(vi.getTimerCount()).toBe(0);
    sleeper.stop();
  });
});

describe("createPainterClock", () => {
  it("follows the frames", () => {
    const clock = createPainterClock();
    expect(clock(1000)).toBe(0);
    expect(clock(1016)).toBe(16);
    expect(clock(1032)).toBe(32);
  });

  it("never jumps more than the frame gap, after a sleep or a hidden tab", () => {
    const clock = createPainterClock();
    clock(0);
    clock(16);
    expect(clock(60_000)).toBe(16 + MAX_FRAME_GAP_MS);
  });

  it("never runs backwards", () => {
    const clock = createPainterClock();
    clock(500);
    expect(clock(400)).toBe(0);
    expect(clock(416)).toBe(16);
  });
});
