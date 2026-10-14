import { act, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  VuMeter,
  VuMeterFace,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import type { VuMeterActions } from "@/components/ui/vu-meter";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import { vuTaper } from "@/lib/audio/taper";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const SETTLE_MS = 1500;
const scale = vuTaper(-10, 3);
const ZERO_VU = scale.toPosition(0);
const PLUS_TWO_VU = scale.toPosition(2);

const meter = () => screen.getByRole("meter");
const levelOf = () => Number(meter().style.getPropertyValue("--vu-level"));

const reduceMotion = (reduce: boolean) => {
  Object.defineProperty(globalThis, "matchMedia", {
    configurable: true,
    value: (query: string) => ({
      addEventListener: vi.fn(),
      matches: reduce && query.includes("reduce"),
      media: query,
      removeEventListener: vi.fn(),
    }),
    writable: true,
  });
};

describe("VuMeter", () => {
  beforeEach(() => {
    useFakeFrames();
    reduceMotion(false);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders a labelled meter over the printed scale", () => {
    render(<VuMeter aria-label="Program" rmsDb={-18} />);
    const root = screen.getByRole("meter", { name: "Program" });
    expect(root).toHaveAttribute("aria-valuemin", "-10");
    expect(root).toHaveAttribute("aria-valuemax", "3");
    expect(root).toHaveAttribute("data-variant", "classic");
  });

  it("reads 0 VU at the reference level", () => {
    render(<VuMeter aria-label="Program" rmsDb={-18} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 3);
    expect(meter()).toHaveAttribute("aria-valuenow", "0.0");
    expect(meter()).toHaveAttribute("aria-valuetext", "0.0 VU");
  });

  it("reads relative to referenceDb", () => {
    render(<VuMeter aria-label="Program" referenceDb={-20} rmsDb={-18} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(PLUS_TWO_VU, 3);
    expect(meter()).toHaveAttribute("aria-valuetext", "+2.0 VU");
  });

  it("swings up from rest instead of jumping", () => {
    render(<VuMeter aria-label="Program" rmsDb={-18} />);
    advance(100);
    expect(levelOf()).toBeGreaterThan(0);
    expect(levelOf()).toBeLessThan(ZERO_VU * 0.9);
  });

  it("follows a frame source", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(<VuMeter aria-label="Program" source={emitter} />);
    act(() => {
      emitter.emit({ channels: [{ peakDb: -9, rmsDb: -16 }] });
    });
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(PLUS_TWO_VU, 3);
  });

  it("paints through actionsRef, and falls back to rest on reset", () => {
    const actions = createRef<VuMeterActions>();
    render(<VuMeter actionsRef={actions} aria-label="Program" />);
    act(() => {
      actions.current?.paint({ channels: [{ peakDb: -9, rmsDb: -18 }] });
    });
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 3);
    act(() => {
      actions.current?.reset();
    });
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(-0.04, 4);
    expect(meter()).not.toHaveAttribute("data-active");
  });

  it("shows the chosen channel and measure", () => {
    const channels = [
      { peakDb: -12, rmsDb: -21 },
      { peakDb: -9, rmsDb: -16 },
    ];
    const { rerender } = render(
      <VuMeter aria-label="Left" channel={0} channels={channels} />
    );
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(scale.toPosition(-3), 3);

    rerender(<VuMeter aria-label="Left" channel={1} channels={channels} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(PLUS_TWO_VU, 3);

    rerender(<VuMeter aria-label="Left" channel="max" channels={channels} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(PLUS_TWO_VU, 3);

    rerender(
      <VuMeter
        aria-label="Left"
        channel={0}
        channels={channels}
        measure="peak"
      />
    );
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(1.04, 4);
  });

  it("falls back to the peak level when a frame has no RMS", () => {
    render(<VuMeter aria-label="Program" peakDb={-18} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 3);
  });

  it("marks the zone at the needle and the right pin", () => {
    const { rerender } = render(<VuMeter aria-label="Program" rmsDb={-21} />);
    advance(SETTLE_MS);
    expect(meter()).toHaveAttribute("data-zone", "ok");
    expect(meter()).toHaveAttribute("data-active");
    expect(meter()).not.toHaveAttribute("data-pinned");

    rerender(<VuMeter aria-label="Program" rmsDb={-16} />);
    advance(SETTLE_MS);
    expect(meter()).toHaveAttribute("data-zone", "clip");

    rerender(<VuMeter aria-label="Program" rmsDb={-6} />);
    advance(SETTLE_MS);
    expect(meter()).toHaveAttribute("data-pinned");
    expect(levelOf()).toBeCloseTo(1.04, 4);
  });

  it("rests against the left pin at silence", () => {
    render(<VuMeter aria-label="Program" />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(-0.04, 4);
    expect(meter()).not.toHaveAttribute("data-active");
    expect(meter()).toHaveAttribute("aria-valuenow", "-10.0");
  });

  it("updates ARIA at most four times a second", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(<VuMeter aria-label="Program" source={emitter} />);
    const writes = vi.spyOn(meter(), "setAttribute");
    for (let step = 0; step < 60; step += 1) {
      act(() => {
        emitter.emit({ channels: [{ peakDb: -6, rmsDb: -30 + step / 4 }] });
      });
      advance(16);
    }
    const valueWrites = writes.mock.calls.filter(
      ([name]) => name === "aria-valuenow"
    );
    expect(valueWrites.length).toBeGreaterThan(1);
    expect(valueWrites.length).toBeLessThanOrEqual(5);
  });

  it("jumps straight to the level with reduced motion", () => {
    reduceMotion(true);
    const { rerender } = render(<VuMeter aria-label="Program" rmsDb={-18} />);
    advance(300);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 4);
    rerender(<VuMeter aria-label="Program" rmsDb={-16} />);
    advance(300);
    expect(levelOf()).toBeCloseTo(PLUS_TWO_VU, 4);
  });

  it("stops its frame loop and source subscription on unmount", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    let subscribers = 0;
    const source: FrameSource<MeterFrame> = {
      subscribe: (listener) => {
        subscribers += 1;
        const unsubscribe = emitter.subscribe(listener);
        return () => {
          subscribers -= 1;
          unsubscribe();
        };
      },
    };
    const { unmount } = render(
      <VuMeter aria-label="Program" source={source} />
    );
    advance(100);
    expect(subscribers).toBe(1);
    unmount();
    expect(subscribers).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("draws the default scale with the red section from 0 VU", () => {
    render(
      <VuMeter aria-label="Program">
        <VuMeterFace>
          <VuMeterScale />
          <VuMeterNeedle />
        </VuMeterFace>
      </VuMeter>
    );
    const ticks = document.querySelectorAll('[data-slot="vu-meter-tick"]');
    const labels = document.querySelectorAll(
      '[data-slot="vu-meter-scale-label"]'
    );
    expect(ticks).toHaveLength(12);
    expect(labels).toHaveLength(10);
    expect([...labels].map((label) => label.textContent)).toEqual([
      "−10",
      "−7",
      "−5",
      "−3",
      "−2",
      "−1",
      "0",
      "+1",
      "+2",
      "+3",
    ]);
    expect(
      document.querySelectorAll(
        '[data-slot="vu-meter-tick"][data-major][data-zone="clip"]'
      )
    ).toHaveLength(4);
    expect(
      document.querySelectorAll('[data-slot="vu-meter-arc"]')
    ).toHaveLength(2);
  });

  it("pivots from the bottom by default, with the scale arcing over it", () => {
    render(<VuMeter aria-label="Program" />);
    expect(meter()).toHaveAttribute("data-pivot", "bottom");
    const zero = document.querySelector(
      '[data-slot="vu-meter-tick"][data-major][data-zone="clip"]'
    );
    const label = [
      ...document.querySelectorAll('[data-slot="vu-meter-scale-label"]'),
    ].find((element) => element.textContent === "0");
    // Numbers sit outside the arc, away from the pivot: above it here.
    expect(Number(label?.getAttribute("y"))).toBeLessThan(
      Number(zero?.getAttribute("y2"))
    );
    expect(Number(zero?.getAttribute("y2"))).toBeLessThan(
      Number(zero?.getAttribute("y1"))
    );
    expect(
      document.querySelector('[data-slot="vu-meter-cover"]')
    ).not.toBeNull();
  });

  it("hangs the needle from the top with pivot=top", () => {
    render(<VuMeter aria-label="Program" pivot="top" />);
    expect(meter()).toHaveAttribute("data-pivot", "top");
    const zero = document.querySelector(
      '[data-slot="vu-meter-tick"][data-major][data-zone="clip"]'
    );
    const label = [
      ...document.querySelectorAll('[data-slot="vu-meter-scale-label"]'),
    ].find((element) => element.textContent === "0");
    expect(Number(label?.getAttribute("y"))).toBeGreaterThan(
      Number(zero?.getAttribute("y2"))
    );
    expect(Number(zero?.getAttribute("y2"))).toBeGreaterThan(
      Number(zero?.getAttribute("y1"))
    );
    expect(document.querySelector('[data-slot="vu-meter-cover"]')).toBeNull();
  });

  it("reads the same on either pivot", () => {
    const { rerender } = render(
      <VuMeter aria-label="Program" pivot="top" rmsDb={-16} />
    );
    advance(SETTLE_MS);
    const top = levelOf();
    rerender(<VuMeter aria-label="Program" pivot="bottom" rmsDb={-16} />);
    advance(SETTLE_MS);
    expect(levelOf()).toBeCloseTo(top, 4);
    expect(top).toBeCloseTo(PLUS_TWO_VU, 3);
  });

  it("keeps ticks to the range it is given", () => {
    render(
      <VuMeter aria-label="Program" minDb={-20}>
        <VuMeterFace>
          <VuMeterScale ticks={[-20, -10, 0, 3, 6]} />
        </VuMeterFace>
      </VuMeter>
    );
    const labels = document.querySelectorAll(
      '[data-slot="vu-meter-scale-label"]'
    );
    expect([...labels].map((label) => label.textContent)).toEqual([
      "−20",
      "−10",
      "0",
      "+3",
    ]);
  });

  it("requires a VuMeter around its parts", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {
      // React reports the thrown error; the assertion below checks it.
    });
    expect(() => render(<VuMeterScale />)).toThrow(
      "VuMeterScale must be used inside VuMeter."
    );
    error.mockRestore();
  });
});
