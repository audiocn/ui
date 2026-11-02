import { act, render, screen } from "@testing-library/react";
import { createRef } from "react";
import type { CSSProperties } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  VuMeter,
  VuMeterFace,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import type { VuMeterActions } from "@/components/ui/vu-meter";
import type { AnalyserInput } from "@/hooks/use-audio-analyser";
import type { VuNeedleOptions } from "@/hooks/use-vu-needle";
import { NEEDLE_PIN, NEEDLE_REST } from "@/lib/audio/needle";
import { vuTaper } from "@/lib/audio/taper";

const { needle } = vi.hoisted(() => ({
  needle: {
    input: null as unknown,
    onPosition: (_position: number) => {
      // Replaced on every render.
    },
    options: {} as unknown,
    reset: vi.fn(),
    status: "running",
  },
}));

vi.mock("@/hooks/use-vu-needle", () => ({
  useVuNeedle: (
    input: AnalyserInput,
    onPosition: (position: number) => void,
    options: VuNeedleOptions
  ) => {
    needle.input = input;
    needle.onPosition = onPosition;
    needle.options = options;
    return { error: null, reset: needle.reset, status: needle.status };
  },
}));

const scale = vuTaper(-10, 3);
const ZERO_VU = scale.toPosition(0);
const PLUS_TWO_VU = scale.toPosition(2);
const MINUS_THREE_VU = scale.toPosition(-3);

const meter = () => screen.getByRole("meter");
const levelOf = () => Number(meter().style.getPropertyValue("--vu-level"));
const post = (position: number) => act(() => needle.onPosition(position));
const inputNode = () => ({}) as AudioNode;

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

class FakeIntersectionObserver {
  static last: FakeIntersectionObserver | null = null;
  readonly notify: IntersectionObserverCallback;
  constructor(notify: IntersectionObserverCallback) {
    this.notify = notify;
    FakeIntersectionObserver.last = this;
  }
  observe = vi.fn();
  disconnect = vi.fn();
  show(visible: boolean) {
    act(() =>
      this.notify(
        [{ isIntersecting: visible } as IntersectionObserverEntry],
        this as unknown as IntersectionObserver
      )
    );
  }
}

describe("VuMeter", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    reduceMotion(false);
    needle.reset.mockReset();
    vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("renders a labelled meter over the printed scale", () => {
    render(<VuMeter aria-label="Program" />);
    const root = screen.getByRole("meter", { name: "Program" });
    expect(root).toHaveAttribute("aria-valuemin", "-10");
    expect(root).toHaveAttribute("aria-valuemax", "3");
    expect(root).toHaveAttribute("data-variant", "classic");
  });

  it("runs the needle on its input, with the scale and ballistics", () => {
    const input = inputNode();
    render(
      <VuMeter
        aria-label="Program"
        ballistics={{ lowPassCutoffHz: 3 }}
        channel={1}
        input={input}
        maxDb={6}
        minDb={-20}
        referenceDb={-20}
      />
    );
    expect(needle.input).toBe(input);
    expect(needle.options).toEqual({
      channel: 1,
      lowPassCutoffHz: 3,
      maxDb: 6,
      minDb: -20,
      referenceDb: -20,
    });
  });

  it("paints the position the audio thread sends, and reads it in VU", () => {
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    post(ZERO_VU);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 4);
    expect(meter()).toHaveAttribute("aria-valuenow", "0.0");
    expect(meter()).toHaveAttribute("aria-valuetext", "0.0 VU");
    vi.advanceTimersByTime(300);
    post(PLUS_TWO_VU);
    expect(meter()).toHaveAttribute("aria-valuetext", "+2.0 VU");
  });

  it("marks the zone at the needle, and when it leaves rest or hits the stop", () => {
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    post(MINUS_THREE_VU);
    expect(meter()).toHaveAttribute("data-zone", "ok");
    expect(meter()).toHaveAttribute("data-active");
    expect(meter()).not.toHaveAttribute("data-pinned");

    post(PLUS_TWO_VU);
    expect(meter()).toHaveAttribute("data-zone", "clip");

    post(NEEDLE_PIN);
    expect(meter()).toHaveAttribute("data-pinned");

    post(NEEDLE_REST);
    expect(meter()).not.toHaveAttribute("data-active");
    expect(meter()).not.toHaveAttribute("data-pinned");
  });

  it("updates ARIA at most four times a second", () => {
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    const writes = vi.spyOn(meter(), "setAttribute");
    for (let step = 0; step < 120; step += 1) {
      post(MINUS_THREE_VU + step / 1000);
      vi.advanceTimersByTime(1000 / 120);
    }
    const valueWrites = writes.mock.calls.filter(
      ([name]) => name === "aria-valuenow"
    );
    expect(valueWrites.length).toBeGreaterThan(1);
    expect(valueWrites.length).toBeLessThanOrEqual(5);
  });

  it("paints four times a second with reduced motion", () => {
    reduceMotion(true);
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    const writes = vi.spyOn(meter().style, "setProperty");
    for (let step = 0; step < 120; step += 1) {
      post(MINUS_THREE_VU + step / 1000);
      vi.advanceTimersByTime(1000 / 120);
    }
    expect(writes.mock.calls.length).toBeGreaterThan(1);
    expect(writes.mock.calls.length).toBeLessThanOrEqual(5);
  });

  it("skips painting while off screen", () => {
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    FakeIntersectionObserver.last?.show(false);
    post(ZERO_VU);
    expect(meter().style.getPropertyValue("--vu-level")).toBe("");
    FakeIntersectionObserver.last?.show(true);
    post(ZERO_VU);
    expect(levelOf()).toBeCloseTo(ZERO_VU, 4);
  });

  it("drops back to rest when the input goes away", () => {
    const { rerender } = render(
      <VuMeter aria-label="Program" input={inputNode()} />
    );
    post(ZERO_VU);
    rerender(<VuMeter aria-label="Program" input={null} />);
    expect(levelOf()).toBeCloseTo(NEEDLE_REST, 4);
    expect(meter()).not.toHaveAttribute("data-active");
    expect(meter()).toHaveAttribute("aria-valuenow", "-10.0");
  });

  it("leaves a needle set with CSS alone when it has no input", () => {
    render(
      <VuMeter
        aria-label="Program"
        style={{ "--vu-level": ZERO_VU } as CSSProperties}
      />
    );
    expect(levelOf()).toBeCloseTo(ZERO_VU, 4);
  });

  it("shows the needle's status on the root", () => {
    needle.status = "suspended";
    render(<VuMeter aria-label="Program" input={inputNode()} />);
    expect(meter()).toHaveAttribute("data-status", "suspended");
    needle.status = "running";
  });

  it("resets the needle through actionsRef", () => {
    const actions = createRef<VuMeterActions>();
    render(
      <VuMeter actionsRef={actions} aria-label="Program" input={inputNode()} />
    );
    post(ZERO_VU);
    act(() => actions.current?.reset());
    expect(needle.reset).toHaveBeenCalledOnce();
    // Painted at once: a suspended context sends no more positions.
    expect(levelOf()).toBeCloseTo(NEEDLE_REST, 4);
    expect(meter()).not.toHaveAttribute("data-active");
    expect(meter()).toHaveAttribute("aria-valuenow", "-10.0");
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
