import { act, fireEvent, render, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { Spectrum, SpectrumCanvas } from "@/components/ui/spectrum";
import { Waveform, WaveformHover } from "@/components/ui/waveform";
import { useWaveformData } from "@/hooks/use-waveform-data";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const PEAKS = [0.2, 0.6, 1, 0.6, 0.2];

const frameOf = (bands: number[]): VisualFrame => ({
  bands: Float32Array.from(bands),
  history: new Float32Array(1),
  historyLength: 0,
  historyStart: 0,
  peakDb: -12,
});

/** One recording 2D context per canvas, so canvases can be told apart. */
const stubCanvases = () => {
  const clears = new Map<HTMLCanvasElement, number>();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
    function getContext(this: HTMLCanvasElement) {
      return {
        arc: vi.fn(),
        beginPath: vi.fn(),
        clearRect: () => {
          clears.set(this, (clears.get(this) ?? 0) + 1);
        },
        clip: vi.fn(),
        closePath: vi.fn(),
        createLinearGradient: () => ({ addColorStop: vi.fn() }),
        fill: vi.fn(),
        fillRect: vi.fn(),
        fillText: vi.fn(),
        lineTo: vi.fn(),
        moveTo: vi.fn(),
        rect: vi.fn(),
        restore: vi.fn(),
        save: vi.fn(),
        setTransform: vi.fn(),
        stroke: vi.fn(),
      } as unknown as CanvasRenderingContext2D;
    } as unknown as HTMLCanvasElement["getContext"]
  );
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getBoundingClientRect"
  ).mockReturnValue(DOMRect.fromRect({ height: 80, width: 240 }));
  return clears;
};

const barLevel = (container: HTMLElement, index = 2) =>
  Number(
    container
      .querySelector<HTMLElement>(`[data-index="${index}"]`)
      ?.style.getPropertyValue("--bar-level")
  );

const view = (loading: boolean) => (
  <Waveform duration={100} loading={loading} peaks={PEAKS}>
    <WaveformHover />
  </Waveform>
);

describe("visualizers", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("waveform: leaving clears the hover line even while seeking is off", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ height: 80, width: 200 })
    );
    const { container, rerender } = render(view(false));
    const root = container.querySelector<HTMLElement>("[data-slot='waveform']");
    const hoverLine = () =>
      container.querySelector("[data-slot='waveform-hover']");

    fireEvent.pointerMove(root as HTMLElement, { clientX: 100 });
    expect(hoverLine()).not.toBeNull();
    // A new track starts loading under the pointer: seeking turns off.
    rerender(view(true));
    expect(hoverLine()).toBeNull();
    fireEvent.pointerLeave(root as HTMLElement);
    rerender(view(false));
    expect(hoverLine()).toBeNull();
  });

  it("spectrum: every canvas repaints on a new frame", () => {
    const clears = stubCanvases();
    const source = createFrameEmitter<VisualFrame>();
    const { container } = render(
      <Spectrum source={source}>
        <SpectrumCanvas />
        <SpectrumCanvas />
      </Spectrum>
    );
    advance(100);
    const canvases = [...container.querySelectorAll("canvas")];
    expect(canvases).toHaveLength(2);
    const before = canvases.map((canvas) => clears.get(canvas) ?? 0);
    act(() => {
      source.emit(frameOf([0.5, 0.8, 0.3]));
    });
    advance(50);
    for (const [index, canvas] of canvases.entries()) {
      expect(clears.get(canvas) ?? 0).toBeGreaterThan(before[index] ?? 0);
    }
  });

  it("bar visualizer: bars fall when levels go away, and data-active follows", () => {
    const { container, rerender } = render(
      <BarVisualizer aria-label="Bands" barCount={5} levels={[1, 1, 1, 1, 1]} />
    );
    advance(500);
    const root = container.querySelector("[data-slot='bar-visualizer']");
    expect(barLevel(container)).toBeGreaterThan(0.8);
    expect(root).toHaveAttribute("data-active");

    rerender(<BarVisualizer aria-label="Bands" barCount={5} />);
    advance(3000);
    expect(barLevel(container)).toBeLessThan(0.3);
    expect(root).not.toHaveAttribute("data-active");
  });
});

describe("useWaveformData", () => {
  it("reports an error, not endless loading, without Web Audio", () => {
    const { result } = renderHook(() => useWaveformData("song.mp3"));
    expect(result.current.status).toBe("error");
  });
});
