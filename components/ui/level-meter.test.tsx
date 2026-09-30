import { act, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LevelMeter } from "@/components/ui/level-meter";
import type { LevelMeterActions } from "@/components/ui/level-meter";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const channelElement = (index = 0) =>
  document.querySelector<HTMLElement>(
    `[data-slot="level-meter-channel"][data-index="${index}"]`
  );

const levelOf = (index = 0) =>
  Number(channelElement(index)?.style.getPropertyValue("--meter-level"));

describe("LevelMeter", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders a labelled meter with its range", () => {
    render(<LevelMeter aria-label="Mic" peakDb={-12} />);
    const meter = screen.getByRole("meter", { name: "Mic" });
    expect(meter).toHaveAttribute("aria-valuemin", "-60");
    expect(meter).toHaveAttribute("aria-valuemax", "0");
  });

  it("paints declarative values and reports them to assistive technology", () => {
    render(<LevelMeter aria-label="Mic" peakDb={-6} />);
    advance(400);
    expect(levelOf()).toBeCloseTo(0.9, 2);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "-6.0");
    expect(channelElement()).toHaveAttribute("data-zone", "clip");
  });

  it("follows a frame source", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(<LevelMeter aria-label="Mic" source={emitter} />);
    act(() => {
      emitter.emit({ channels: [{ peakDb: -30 }] });
    });
    advance(400);
    expect(levelOf()).toBeCloseTo(0.5, 2);
  });

  it("adds tracks for every channel in the frames", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(<LevelMeter aria-label="Program" source={emitter} />);
    advance(20);
    act(() => {
      emitter.emit({ channels: [{ peakDb: -12 }, { peakDb: -24 }] });
    });
    advance(400);
    expect(channelElement(1)).not.toBeNull();
    expect(levelOf(1)).toBeCloseTo(0.6, 2);
  });

  it("paints through actionsRef", () => {
    const actions = createRef<LevelMeterActions>();
    render(<LevelMeter actionsRef={actions} aria-label="Mic" />);
    act(() => {
      actions.current?.paint({ channels: [{ peakDb: -18 }] });
    });
    advance(400);
    expect(levelOf()).toBeCloseTo(0.7, 2);
  });

  it("marks clipping on the root", () => {
    render(<LevelMeter aria-label="Mic" peakDb={0} />);
    advance(100);
    expect(screen.getByRole("meter")).toHaveAttribute("data-clipping");
  });

  it("uses the orientation it is given", () => {
    render(<LevelMeter aria-label="Mic" orientation="vertical" peakDb={-6} />);
    expect(screen.getByRole("meter")).toHaveAttribute(
      "data-orientation",
      "vertical"
    );
  });
});
