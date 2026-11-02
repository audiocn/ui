import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { DbReadout } from "@/components/ui/db-readout";
import { LevelMeter } from "@/components/ui/level-meter";
import { LiveWaveform } from "@/components/ui/live-waveform";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame, VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

// −12 and −40 dBFS on the default linear −60..0 range.
const AT_MINUS_12 = 0.8;
const AT_MINUS_40 = 1 / 3;
// A big drop settles in about 6 s: the peak hold waits 1.2 s, then falls.
const SETTLE_MS = 10_000;

const meterLevel = () =>
  Number(
    document
      .querySelector<HTMLElement>("[data-slot='level-meter-channel']")
      ?.style.getPropertyValue("--meter-level")
  );

const barLevel = (index = 0) =>
  Number(
    document
      .querySelector<HTMLElement>(`[data-index="${index}"]`)
      ?.style.getPropertyValue("--bar-level")
  );

const entries = (visible: boolean) => [
  { isIntersecting: visible } as IntersectionObserverEntry,
];

/** IntersectionObserver that the test drives by hand. */
const stubVisibility = () => {
  const observers: IntersectionObserverCallback[] = [];
  class ObserverStub {
    constructor(notify: IntersectionObserverCallback) {
      observers.push(notify);
    }
    observe = vi.fn();
    disconnect = vi.fn();
  }
  vi.stubGlobal("IntersectionObserver", ObserverStub);
  return (visible: boolean) => {
    act(() => {
      for (const notify of observers) {
        notify(entries(visible), {} as IntersectionObserver);
      }
    });
  };
};

const stubCanvas = () => {
  const clearRect = vi.fn();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    beginPath: vi.fn(),
    clearRect,
    createLinearGradient: () => ({ addColorStop: vi.fn() }),
    fill: vi.fn(),
    fillRect: vi.fn(),
    lineTo: vi.fn(),
    moveTo: vi.fn(),
    rect: vi.fn(),
    roundRect: vi.fn(),
    setTransform: vi.fn(),
    stroke: vi.fn(),
  } as unknown as CanvasRenderingContext2D);
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getBoundingClientRect"
  ).mockReturnValue(DOMRect.fromRect({ height: 40, width: 200 }));
  return clearRect;
};

const visualFrame = (level: number): VisualFrame => ({
  bands: Float32Array.from([level, level, level]),
  history: Float32Array.from([level, level, level]),
  historyLength: 3,
  historyStart: 0,
  peakDb: -12,
});

describe("settled painters request no frames", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("level meter: sleeps once settled, and a new value wakes it", () => {
    const { rerender } = render(<LevelMeter aria-label="Mic" peakDb={-12} />);
    advance(100);
    expect(meterLevel()).toBeCloseTo(AT_MINUS_12, 3);
    expect(vi.getTimerCount()).toBe(0);

    rerender(<LevelMeter aria-label="Mic" peakDb={-40} />);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    advance(SETTLE_MS);
    expect(meterLevel()).toBeCloseTo(AT_MINUS_40, 2);
    expect(screen.getByRole("meter")).toHaveAttribute(
      "aria-valuetext",
      "−40.0 dB"
    );
    expect(vi.getTimerCount()).toBe(0);
  });

  it("level meter: falls smoothly after a long sleep instead of jumping", () => {
    const { rerender } = render(<LevelMeter aria-label="Mic" peakDb={-12} />);
    advance(SETTLE_MS);

    rerender(<LevelMeter aria-label="Mic" peakDb={-40} />);
    advance(20);
    expect(meterLevel()).toBeGreaterThan(0.6);
    expect(meterLevel()).toBeLessThan(AT_MINUS_12);
  });

  it("level meter: sleeps off screen and catches up when it comes back", () => {
    const setVisible = stubVisibility();
    const { rerender } = render(<LevelMeter aria-label="Mic" peakDb={-12} />);
    advance(100);

    setVisible(false);
    rerender(<LevelMeter aria-label="Mic" peakDb={-40} />);
    advance(SETTLE_MS);
    expect(meterLevel()).toBeCloseTo(AT_MINUS_12, 3);
    expect(vi.getTimerCount()).toBe(0);

    setVisible(true);
    advance(SETTLE_MS);
    expect(meterLevel()).toBeCloseTo(AT_MINUS_40, 2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("level meter: holds a steady clip without frames, and counts the light down after", () => {
    const { rerender } = render(<LevelMeter aria-label="Mic" peakDb={0} />);
    advance(100);
    const meter = screen.getByRole("meter");
    expect(meter).toHaveAttribute("data-clipping");
    expect(vi.getTimerCount()).toBe(0);

    rerender(<LevelMeter aria-label="Mic" peakDb={-30} />);
    advance(1000);
    expect(meter).toHaveAttribute("data-clipping");
    advance(SETTLE_MS);
    expect(meter).not.toHaveAttribute("data-clipping");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("bar visualizer: static bars settle, and new levels wake them", () => {
    const { rerender } = render(
      <BarVisualizer barCount={3} levels={[0.6, 0.6, 0.6]} />
    );
    advance(100);
    expect(barLevel()).toBeCloseTo(0.6, 3);
    expect(vi.getTimerCount()).toBe(0);

    rerender(<BarVisualizer barCount={3} levels={[0.1, 0.1, 0.1]} />);
    advance(50);
    expect(barLevel()).toBeGreaterThan(0.1);
    advance(SETTLE_MS);
    expect(barLevel()).toBeCloseTo(0.1, 2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("bar visualizer: an idle animation keeps running", () => {
    render(<BarVisualizer barCount={3} idle="pulse" />);
    advance(SETTLE_MS);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
  });

  it("bar visualizer: a source frame wakes sleeping bars", () => {
    const source = createFrameEmitter<VisualFrame>();
    render(<BarVisualizer barCount={3} source={source} />);
    advance(200);
    expect(vi.getTimerCount()).toBe(0);

    act(() => {
      source.emit(visualFrame(0.7));
    });
    advance(100);
    expect(barLevel()).toBeCloseTo(0.7, 2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("live waveform: draws, sleeps, and redraws on the next frame", () => {
    const clearRect = stubCanvas();
    const source = createFrameEmitter<VisualFrame>();
    render(<LiveWaveform mode="scrolling" source={source} />);
    advance(100);
    const drawn = clearRect.mock.calls.length;
    expect(drawn).toBeGreaterThan(0);
    expect(vi.getTimerCount()).toBe(0);

    act(() => {
      source.emit(visualFrame(0.5));
    });
    advance(50);
    expect(clearRect.mock.calls.length).toBe(drawn + 1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("live waveform: a theme change makes a sleeping waveform re-read its colour", async () => {
    stubCanvas();
    render(<LiveWaveform source={createFrameEmitter<VisualFrame>()} />);
    advance(100);
    const styleReads = vi.spyOn(window, "getComputedStyle");

    await act(async () => {
      document.documentElement.classList.add("dark");
      await Promise.resolve();
    });
    advance(50);
    expect(styleReads).toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
    document.documentElement.classList.remove("dark");
  });

  it("dB readout: its ticker stops when the source goes quiet and restarts on a frame", () => {
    const source = createFrameEmitter<MeterFrame>();
    render(<DbReadout source={source} />);
    const readout = document.querySelector("[data-slot='db-readout']");

    act(() => {
      source.emit({ channels: [{ peakDb: -12 }] });
    });
    advance(300);
    expect(readout).toHaveTextContent("−12.0 dB");

    advance(2000);
    expect(readout).toHaveTextContent("−∞ dB");
    expect(vi.getTimerCount()).toBe(0);

    act(() => {
      source.emit({ channels: [{ peakDb: -6 }] });
    });
    advance(300);
    expect(readout).toHaveTextContent("−6.0 dB");
  });
});
