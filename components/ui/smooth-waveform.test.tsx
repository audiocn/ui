import { render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SmoothWaveform } from "@/components/ui/smooth-waveform";
import type { SmoothWaveformActions } from "@/components/ui/smooth-waveform";
import type { VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const frameOf = (bands: number[]): VisualFrame => ({
  bands: Float32Array.from(bands),
  history: new Float32Array(1),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
});

/** A 2D context that records the points it is asked to draw. */
const stubCanvas = () => {
  const recorder = { points: [] as [number, number][], strokes: 0 };
  const gradient = { addColorStop: vi.fn() };
  const context = {
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    createLinearGradient: () => gradient,
    fillRect: vi.fn(),
    lineTo: (x: number, y: number) => {
      recorder.points.push([x, y]);
    },
    moveTo: (x: number, y: number) => {
      recorder.points.push([x, y]);
    },
    setTransform: vi.fn(),
    stroke: () => {
      recorder.strokes += 1;
    },
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D
  );
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getBoundingClientRect"
  ).mockReturnValue(DOMRect.fromRect({ height: 80, width: 240 }));
  return recorder;
};

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("SmoothWaveform", () => {
  it("renders a labelled image over one hidden canvas", () => {
    stubCanvas();
    const { container, getByRole } = render(
      <SmoothWaveform aria-label="Voice" loading mode="scope" />
    );
    const root = getByRole("img", { name: "Voice" });
    expect(root).toHaveAttribute("data-slot", "smooth-waveform");
    expect(root).toHaveAttribute("data-mode", "scope");
    expect(root).toHaveAttribute("data-loading");
    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(1);
    expect(canvases[0]).toHaveAttribute("aria-hidden", "true");
  });

  it("draws a flat line across the width with no signal", () => {
    const recorder = stubCanvas();
    render(<SmoothWaveform />);
    advance(100);
    const xs = recorder.points.map(([x]) => x);
    expect(Math.min(...xs)).toBe(0);
    expect(Math.max(...xs)).toBe(240);
    expect(new Set(recorder.points.map(([, y]) => y)).size).toBe(1);
  });

  it("follows frames from its handle and marks an active signal", () => {
    const recorder = stubCanvas();
    const actions = createRef<SmoothWaveformActions>();
    const { getByRole } = render(<SmoothWaveform actionsRef={actions} />);
    advance(100);
    const root = getByRole("img");
    expect(root).not.toHaveAttribute("data-active");
    actions.current?.paint(frameOf([1, 1, 1, 1]));
    recorder.points.length = 0;
    advance(300);
    expect(root).toHaveAttribute("data-active");
    expect(new Set(recorder.points.map(([, y]) => y)).size).toBeGreaterThan(1);
    actions.current?.clear();
    advance(1000);
    expect(root).not.toHaveAttribute("data-active");
  });

  it("stops painting once unmounted", () => {
    const recorder = stubCanvas();
    const { unmount } = render(<SmoothWaveform />);
    advance(100);
    unmount();
    const { strokes } = recorder;
    advance(200);
    expect(recorder.strokes).toBe(strokes);
  });
});
