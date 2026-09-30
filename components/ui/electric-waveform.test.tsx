import { render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  ElectricWaveform,
  createElectricTrace,
} from "@/components/ui/electric-waveform";
import type {
  ElectricTraceOptions,
  ElectricWaveformActions,
} from "@/components/ui/electric-waveform";
import type { VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const FRAME_MS = 16;
const geometry = { height: 80, lineWidth: 3, width: 256 };

const frameOf = (bands: number[]): VisualFrame => ({
  bands: Float32Array.from(bands),
  history: new Float32Array(1),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
});

const options = (
  overrides: Partial<ElectricTraceOptions> = {}
): ElectricTraceOptions => ({
  arcs: true,
  intensity: 1,
  loading: false,
  mode: "wave",
  reducedMotion: false,
  seed: 5,
  sensitivity: 1,
  sparks: true,
  ...overrides,
});

const run = (
  trace: ReturnType<typeof createElectricTrace>,
  frameAt: (frame: number) => VisualFrame | null,
  frames: number
) => {
  let active = false;
  for (let frame = 1; frame <= frames; frame += 1) {
    active = trace.step(frame * FRAME_MS, frameAt(frame), geometry);
  }
  return active;
};

/** A 2D context that records the points it is asked to draw. */
const stubCanvas = () => {
  const recorder = { points: [] as [number, number][], strokes: 0 };
  const gradient = { addColorStop: vi.fn() };
  const context = {
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    closePath: vi.fn(),
    createLinearGradient: () => gradient,
    fill: vi.fn(),
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
  ).mockReturnValue(DOMRect.fromRect({ height: 80, width: 256 }));
  return recorder;
};

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("createElectricTrace", () => {
  it("crackles the same way for the same seed", () => {
    const first = createElectricTrace(options());
    const second = createElectricTrace(options());
    const other = createElectricTrace(options({ seed: 6 }));
    for (const trace of [first, second, other]) {
      run(trace, () => frameOf([0.8, 0.6, 0.4, 0.2]), 10);
    }
    expect([...first.crackle]).toEqual([...second.crackle]);
    expect([...first.crackle]).not.toEqual([...other.crackle]);
  });

  it("keeps the flicker within 12%", () => {
    const trace = createElectricTrace(options());
    for (let frame = 1; frame <= 60; frame += 1) {
      trace.step(frame * FRAME_MS, frameOf([1, 1, 1, 1]), geometry);
      expect(trace.flicker).toBeGreaterThanOrEqual(0.88);
      expect(trace.flicker).toBeLessThanOrEqual(1);
    }
  });

  it("forks only off points far from the middle", () => {
    const trace = createElectricTrace(options());
    let forks = 0;
    for (let frame = 1; frame <= 600; frame += 1) {
      trace.step(frame * FRAME_MS, frameOf([1, 1, 1, 1]), geometry);
      for (const branch of trace.branches) {
        if (branch.lifeMs > 0 && branch.bornMs === frame * FRAME_MS) {
          forks += 1;
          expect(Math.abs(trace.heights[branch.point] ?? 0)).toBeGreaterThan(
            0.25
          );
        }
      }
    }
    expect(forks).toBeGreaterThan(0);
  });

  it("throws sparks on a sudden rise", () => {
    const trace = createElectricTrace(options());
    run(trace, (frame) => (frame < 3 ? null : frameOf([1, 1, 1, 1])), 3);
    expect(trace.sparks.life.some((life) => life > 0)).toBe(true);
  });

  it("stays smooth and still with reduced motion", () => {
    const trace = createElectricTrace(options({ reducedMotion: true }));
    run(trace, (frame) => frameOf(frame % 2 === 0 ? [1, 1, 1, 1] : [0]), 60);
    expect(trace.crackle.every((offset) => offset === 0)).toBe(true);
    expect(trace.branches.every((branch) => branch.lifeMs === 0)).toBe(true);
    expect(trace.sparks.life.every((life) => life === 0)).toBe(true);
  });
});

describe("ElectricWaveform", () => {
  it("renders a labelled image over two hidden canvases", () => {
    stubCanvas();
    const { container, getByRole } = render(
      <ElectricWaveform aria-label="Voice" loading mode="scope" />
    );
    const root = getByRole("img", { name: "Voice" });
    expect(root).toHaveAttribute("data-slot", "electric-waveform");
    expect(root).toHaveAttribute("data-mode", "scope");
    expect(root).toHaveAttribute("data-loading");
    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(2);
    for (const canvas of canvases) {
      expect(canvas).toHaveAttribute("aria-hidden", "true");
    }
  });

  it("paints frames from its handle and marks an active signal", () => {
    const recorder = stubCanvas();
    const actions = createRef<ElectricWaveformActions>();
    const { getByRole } = render(<ElectricWaveform actionsRef={actions} />);
    advance(100);
    const root = getByRole("img");
    expect(recorder.strokes).toBeGreaterThan(0);
    expect(root).not.toHaveAttribute("data-active");
    actions.current?.paint(frameOf([1, 1, 1, 1]));
    advance(100);
    expect(root).toHaveAttribute("data-active");
    actions.current?.clear();
    advance(1000);
    expect(root).not.toHaveAttribute("data-active");
  });

  it("draws a smooth line with no intensity", () => {
    const smooth = stubCanvas();
    const { unmount } = render(<ElectricWaveform intensity={0} />);
    advance(100);
    unmount();
    expect(new Set(smooth.points.map(([, y]) => y.toFixed(3))).size).toBe(1);
  });

  it("stops painting once unmounted", () => {
    const recorder = stubCanvas();
    const { unmount } = render(<ElectricWaveform />);
    advance(100);
    unmount();
    const { strokes } = recorder;
    advance(200);
    expect(recorder.strokes).toBe(strokes);
  });
});
