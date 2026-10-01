import { render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LiveWaveform } from "@/components/ui/live-waveform";
import type { LiveWaveformActions } from "@/components/ui/live-waveform";
import { createDemoSignal } from "@/hooks/use-demo-signal";
import { createFrameEmitter, createFrameRelay } from "@/lib/audio/frame-source";
import type { VisualFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const frameOf = (): VisualFrame => ({
  bands: Float32Array.of(0.5),
  history: Float32Array.of(0.25, 0.5, 0.75, 1, 0, 0, 0, 0),
  historyIntervalMs: 50,
  historyLength: 4,
  historyStart: 0,
  historyUpdatedAt: 0,
  peakDb: -12,
});

const stubCanvas = (width = 24) => {
  const bars: {
    x: number;
    height: number;
    alpha: number;
    coversCenter: boolean;
  }[] = [];
  const points: { x: number; y: number }[] = [];
  let centerVisible = true;
  let pathContainsCenter = false;
  const visibilityStack: boolean[] = [];
  const context = {
    beginPath: () => {
      pathContainsCenter = false;
    },
    clearRect: vi.fn(() => {
      bars.length = 0;
      points.length = 0;
    }),
    clip: () => {
      centerVisible &&= pathContainsCenter;
    },
    closePath: vi.fn(),
    createLinearGradient: () => ({ addColorStop: vi.fn() }),
    fill: vi.fn(),
    fillRect: vi.fn(),
    globalAlpha: 1,
    lineTo: (x: number, y: number) => points.push({ x, y }),
    moveTo: (x: number, y: number) => points.push({ x, y }),
    rect: (x: number, _y: number, rectWidth: number) => {
      pathContainsCenter ||= x <= width / 2 && width / 2 < x + rectWidth;
    },
    restore: () => {
      centerVisible = visibilityStack.pop() ?? true;
    },
    roundRect: (x: number, _y: number, barWidth: number, height: number) =>
      bars.push({
        alpha: context.globalAlpha,
        coversCenter:
          centerVisible && x <= width / 2 && width / 2 < x + barWidth,
        height,
        x,
      }),
    save: () => {
      visibilityStack.push(centerVisible);
    },
    setTransform: vi.fn(),
    stroke: vi.fn(),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D
  );
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getBoundingClientRect"
  ).mockReturnValue(DOMRect.fromRect({ height: 80, width }));
  return { bars, context, points };
};

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("LiveWaveform scrolling", () => {
  it("moves bars between source frames at one pitch per history interval", () => {
    const { bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    actions.current?.paint(frameOf());
    advance(64);
    const firstX = bars.find((bar) => bar.height === 40)?.x ?? Number.NaN;
    advance(16);
    const nextX = bars.find((bar) => bar.height === 40)?.x ?? Number.NaN;
    expect(firstX - nextX).toBeCloseTo((4 * 16) / 50);
  });

  it("does not jump when the next sample arrives late on the same mutable frame", () => {
    const { bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    const frame = frameOf();
    actions.current?.paint(frame);
    advance(48);
    frame.history[4] = 0.125;
    frame.historyLength = 5;
    frame.historyUpdatedAt = 50;
    actions.current?.paint(frame);
    advance(48);
    const firstX = bars.find((bar) => bar.height === 40)?.x ?? Number.NaN;
    advance(16);
    expect(
      firstX - (bars.find((bar) => bar.height === 40)?.x ?? Number.NaN)
    ).toBeCloseTo((4 * 16) / 50);
  });

  it("keeps the extra bar at the left edge when the visible window is full", () => {
    const { bars } = stubCanvas(12);
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    actions.current?.paint(frameOf());
    advance(64);
    expect(bars).toHaveLength(4);
    expect(bars[0]?.x).toBeCloseTo(0.5 - (4 * 14) / 50);
  });

  it("keeps the outgoing bar visible after the source ring wraps", () => {
    const { bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    const frame = frameOf();
    frame.history = Float32Array.of(0.25, 0.5, 0.75, 1);
    actions.current?.paint(frame);
    advance(48);
    frame.history[0] = 0.125;
    frame.historyStart = 1;
    frame.historyUpdatedAt = 50;
    frame.historyPreviousLevel = 0.25;
    actions.current?.paint(frame);
    advance(48);
    const firstX = bars.find((bar) => bar.height === 20)?.x ?? Number.NaN;
    advance(16);
    expect(
      firstX - (bars.find((bar) => bar.height === 20)?.x ?? Number.NaN)
    ).toBeCloseTo((4 * 16) / 50);
  });

  it("moves a scrolling line between source frames", () => {
    const { points } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform
        actionsRef={actions}
        fadeEdges={false}
        mode="scrolling"
        variant="line"
      />
    );
    actions.current?.paint(frameOf());
    advance(64);
    const firstX = points[0]?.x ?? Number.NaN;
    advance(16);
    expect(firstX - (points[0]?.x ?? Number.NaN)).toBeCloseTo(
      ((24 / 5) * 16) / 50
    );
  });

  it.each([20, 24])(
    "moves mirrored history out from the center at width %i",
    (width) => {
      const { bars } = stubCanvas(width);
      const actions = createRef<LiveWaveformActions>();
      render(
        <LiveWaveform
          actionsRef={actions}
          fadeEdges={false}
          mode="scrolling"
          variant="mirror"
        />
      );
      actions.current?.paint(frameOf());
      advance(64);
      const first = bars
        .filter((bar) => bar.height === 60)
        .map((bar) => bar.x)
        .toSorted((a, b) => a - b);
      advance(16);
      const next = bars
        .filter((bar) => bar.height === 60)
        .map((bar) => bar.x)
        .toSorted((a, b) => a - b);
      expect(first).toHaveLength(2);
      expect((first[0] ?? Number.NaN) - (next[0] ?? Number.NaN)).toBeCloseTo(
        (4 * 16) / 50
      );
      expect((next[1] ?? Number.NaN) - (first[1] ?? Number.NaN)).toBeCloseTo(
        (4 * 16) / 50
      );
      expect((next[0] ?? Number.NaN) + (next[1] ?? Number.NaN) + 3).toBeCloseTo(
        width
      );
    }
  );

  it("keeps mirrored center opacity steady across sample boundaries", () => {
    const { bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform
        actionsRef={actions}
        fadeEdges={false}
        mode="scrolling"
        variant="mirror"
      />
    );
    const frame = frameOf();
    frame.history.fill(0.25);
    frame.historyUpdatedAt = 16;
    actions.current?.paint(frame);
    for (let index = 0; index < 8; index += 1) {
      advance(16);
      const opacity = bars
        .filter((bar) => bar.coversCenter)
        .reduce((alpha, bar) => alpha + (1 - alpha) * bar.alpha, 0);
      expect(opacity).toBeCloseTo(0.55);
      if (index === 3) {
        frame.historyUpdatedAt = 66;
        frame.historyLength += 1;
        actions.current?.paint(frame);
      }
    }
  });

  it("stops moving after one interval without new history, and clears", () => {
    const { context, bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    actions.current?.paint(frameOf());
    advance(160);
    const paints = context.clearRect.mock.calls.length;
    advance(160);
    expect(context.clearRect).toHaveBeenCalledTimes(paints);
    actions.current?.clear();
    advance(16);
    expect(bars).toHaveLength(0);
  });

  it("keeps scrolling continuous when drawing options change", () => {
    const { bars } = stubCanvas(240);
    const signal = createDemoSignal({ kind: "tone" });
    const { rerender } = render(
      <LiveWaveform fadeEdges={false} mode="scrolling" source={signal.visual} />
    );
    advance(272);
    const x = bars[0]?.x ?? Number.NaN;
    rerender(
      <LiveWaveform
        fadeEdges={false}
        mode="scrolling"
        sensitivity={2}
        source={signal.visual}
      />
    );
    advance(16);
    expect(x - (bars[0]?.x ?? Number.NaN)).toBeCloseTo(1);
  });

  it.each([0, 16])(
    "replaces same-timestamp history after a %i ms source delay",
    (delayMs) => {
      const { bars } = stubCanvas();
      const first = createFrameEmitter<VisualFrame>();
      const second = createFrameEmitter<VisualFrame>();
      const { rerender } = render(
        <LiveWaveform fadeEdges={false} mode="scrolling" source={first} />
      );
      const frame = frameOf();
      frame.history.fill(0.25);
      first.emit(frame);
      advance(256);
      expect(bars[0]?.height).toBe(20);
      rerender(
        <LiveWaveform fadeEdges={false} mode="scrolling" source={second} />
      );
      advance(delayMs);
      const replacement = frameOf();
      replacement.history.fill(0.75);
      second.emit(replacement);
      advance(16);
      expect(bars[0]?.height).toBe(60);
    }
  );

  it("clears buffered history before painting within the same frame", () => {
    const { bars } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    const frame = frameOf();
    frame.history.fill(0.25);
    actions.current?.paint(frame);
    advance(256);
    expect(bars[0]?.height).toBe(20);
    actions.current?.clear();
    frame.history.fill(0.75);
    actions.current?.paint(frame);
    advance(16);
    expect(bars[0]?.height).toBe(60);
  });

  it("keeps untimed custom sources and static mode still between frames", () => {
    const { context } = stubCanvas();
    const actions = createRef<LiveWaveformActions>();
    const { rerender } = render(
      <LiveWaveform actionsRef={actions} fadeEdges={false} mode="scrolling" />
    );
    const frame = frameOf();
    delete frame.historyUpdatedAt;
    delete frame.historyIntervalMs;
    actions.current?.paint(frame);
    advance(16);
    let paints = context.clearRect.mock.calls.length;
    advance(32);
    expect(context.clearRect).toHaveBeenCalledTimes(paints);
    rerender(<LiveWaveform actionsRef={actions} fadeEdges={false} />);
    actions.current?.paint(frameOf());
    advance(16);
    paints = context.clearRect.mock.calls.length;
    advance(32);
    expect(context.clearRect).toHaveBeenCalledTimes(paints);
  });

  it("keeps the reduced-motion paint limit", () => {
    vi.stubGlobal("matchMedia", () => ({
      addEventListener: vi.fn(),
      matches: true,
      removeEventListener: vi.fn(),
    }));
    const { context } = stubCanvas();
    const signal = createDemoSignal();
    render(
      <LiveWaveform fadeEdges={false} mode="scrolling" source={signal.visual} />
    );
    advance(1024);
    expect(context.clearRect).toHaveBeenCalledTimes(4);
  });

  it("moves steadily when a source connects after the painter", () => {
    const { bars } = stubCanvas(240);
    const relay = createFrameRelay<VisualFrame>();
    render(<LiveWaveform fadeEdges={false} mode="scrolling" source={relay} />);
    advance(16);
    relay.setSource(createDemoSignal({ kind: "tone" }).visual);
    advance(160);
    let previousX = bars[0]?.x ?? Number.NaN;
    for (let index = 0; index < 8; index += 1) {
      advance(16);
      const x = bars[0]?.x ?? Number.NaN;
      expect(previousX - x).toBeCloseTo((4 * 16) / 64);
      previousX = x;
    }
  });

  it("moves on each paint with the default demo source and stops on unmount", () => {
    const { bars, context } = stubCanvas(240);
    const signal = createDemoSignal({ kind: "tone" });
    const { unmount } = render(
      <LiveWaveform fadeEdges={false} mode="scrolling" source={signal.visual} />
    );
    advance(192);
    const firstX = bars[0]?.x ?? Number.NaN;
    advance(16);
    expect(firstX - (bars[0]?.x ?? Number.NaN)).toBeCloseTo((4 * 16) / 64);
    unmount();
    const paints = context.clearRect.mock.calls.length;
    advance(64);
    expect(context.clearRect).toHaveBeenCalledTimes(paints);
  });
});
