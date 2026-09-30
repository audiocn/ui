import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  ElectricBarVisualizer,
  createElectricScene,
  layoutElectricBars,
} from "@/components/ui/electric-bar-visualizer";
import type {
  ElectricLayout,
  ElectricSceneOptions,
} from "@/components/ui/electric-bar-visualizer";
import { advance, useFakeFrames } from "@/test/fake-frames";

const FRAME_MS = 16;
const CANVAS_WIDTH = 200;
const CANVAS_HEIGHT = 80;

/** A 2D context that records the points it is asked to draw. */
const createRecorder = () => {
  const recorder = {
    points: [] as [number, number][],
    strokes: 0,
  };
  const context = {
    arc: vi.fn(),
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    fill: vi.fn(),
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
  return { context, recorder };
};

const stubCanvas = () => {
  const { context, recorder } = createRecorder();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D
  );
  vi.spyOn(
    HTMLCanvasElement.prototype,
    "getBoundingClientRect"
  ).mockReturnValue(
    DOMRect.fromRect({ height: CANVAS_HEIGHT, width: CANVAS_WIDTH })
  );
  return recorder;
};

const sceneOptions = (
  overrides: Partial<ElectricSceneOptions> = {}
): ElectricSceneOptions => ({
  arcs: true,
  barCount: 4,
  intensity: 1,
  loading: false,
  reducedMotion: false,
  seed: 7,
  sparks: true,
  ...overrides,
});

const layout = (overrides: Partial<ElectricLayout> = {}): ElectricLayout => ({
  ...layoutElectricBars(CANVAS_WIDTH, CANVAS_HEIGHT, {
    align: "end",
    barCount: 4,
    barGap: 4,
    barWidth: 6,
    orientation: "horizontal",
  }),
  ...overrides,
});

/** Steps a scene through `frames` frames with the given levels. */
const run = (
  scene: ReturnType<typeof createElectricScene>,
  levelsAt: (frame: number) => number[],
  frames: number
) => {
  for (let frame = 1; frame <= frames; frame += 1) {
    scene.step(frame * FRAME_MS, Float32Array.from(levelsAt(frame)), layout());
  }
};

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("layoutElectricBars", () => {
  it("centres the row of bars", () => {
    const bars = layoutElectricBars(200, 80, {
      align: "end",
      barCount: 4,
      barGap: 4,
      barWidth: 6,
      orientation: "horizontal",
    });
    expect(bars.pitch).toBe(10);
    expect(bars.first).toBe(85);
    expect(bars.base).toBe(74);
    expect(bars.span).toBe(68);
  });

  it("narrows bars and gaps together when space is short", () => {
    const bars = layoutElectricBars(52, 80, {
      align: "center",
      barCount: 8,
      barGap: 4,
      barWidth: 6,
      orientation: "horizontal",
    });
    expect(bars.pitch).toBe(5);
    expect(bars.barWidth).toBe(3);
    expect(bars.barGap).toBe(2);
    expect(bars.base).toBe(40);
  });

  it("swaps axes when vertical", () => {
    const bars = layoutElectricBars(80, 200, {
      align: "start",
      barCount: 4,
      barGap: 4,
      barWidth: 6,
      orientation: "vertical",
    });
    expect(bars.horizontal).toBe(false);
    expect(bars.first).toBe(85);
    expect(bars.base).toBe(6);
  });
});

describe("createElectricScene", () => {
  it("crackles the same way for the same seed", () => {
    const first = createElectricScene(sceneOptions());
    const second = createElectricScene(sceneOptions());
    const other = createElectricScene(sceneOptions({ seed: 8 }));
    for (const scene of [first, second, other]) {
      run(scene, () => [0.5, 0.5, 0.5, 0.5], 10);
    }
    expect([...first.jitter]).toEqual([...second.jitter]);
    expect([...first.jitter]).not.toEqual([...other.jitter]);
  });

  it("pins the base of each filament and frees its tip", () => {
    const scene = createElectricScene(sceneOptions());
    run(scene, () => [0.5, 0.5, 0.5, 0.5], 1);
    const points = scene.jitter.length / 4;
    expect(scene.jitter[0]).toBe(0);
    expect(scene.jitter[points - 1]).not.toBe(0);
  });

  it("keeps the flicker within 12%", () => {
    const scene = createElectricScene(sceneOptions());
    run(scene, () => [1, 1, 1, 1], 60);
    for (const brightness of scene.flicker) {
      expect(brightness).toBeGreaterThanOrEqual(0.88);
      expect(brightness).toBeLessThanOrEqual(1);
    }
  });

  it("jumps arcs only between loud bars, one or two apart", () => {
    const levels = [1, 0, 1, 0.9, 0, 0];
    const scene = createElectricScene(sceneOptions({ barCount: 6 }));
    const seen = new Set<string>();
    for (let frame = 1; frame <= 600; frame += 1) {
      scene.step(frame * FRAME_MS, Float32Array.from(levels), layout());
      for (const arc of scene.arcs) {
        if (arc.lifeMs > 0) {
          seen.add(`${arc.index}-${arc.index + arc.reach}`);
        }
      }
    }
    expect([...seen].toSorted()).toEqual(["0-2", "2-3"]);
  });

  it("runs one arc along the loading sweep", () => {
    const scene = createElectricScene(
      sceneOptions({ barCount: 8, loading: true })
    );
    scene.step(
      FRAME_MS,
      Float32Array.from([0, 0, 0, 0.1, 0.5, 0.3, 0, 0]),
      layout()
    );
    const alive = scene.arcs.filter((arc) => arc.lifeMs > 0);
    expect(alive).toHaveLength(1);
    expect(alive[0]?.index).toBe(4);
  });

  it("throws sparks on a sudden rise, and they fall and fade", () => {
    const scene = createElectricScene(sceneOptions());
    run(scene, (frame) => (frame < 3 ? [0, 0, 0, 0] : [1, 0, 0, 0]), 3);
    const alive = () => scene.sparks.life.filter((life) => life > 0).length;
    expect(alive()).toBeGreaterThan(0);
    const [firstVelocity] = scene.sparks.vy;
    run(scene, () => [1, 0, 0, 0], 2);
    expect(scene.sparks.vy[0]).toBeGreaterThan(firstVelocity ?? 0);
    run(scene, () => [1, 0, 0, 0], 60);
    expect(alive()).toBe(0);
  });

  it("never holds more than 64 sparks", () => {
    const scene = createElectricScene(sceneOptions({ barCount: 32 }));
    run(
      scene,
      (frame) => Array.from({ length: 32 }, () => (frame % 2 === 0 ? 1 : 0)),
      40
    );
    expect(scene.sparks.life).toHaveLength(64);
    expect(
      scene.sparks.life.filter((life) => life > 0).length
    ).toBeLessThanOrEqual(64);
  });

  it("stays straight and still with reduced motion", () => {
    const scene = createElectricScene(sceneOptions({ reducedMotion: true }));
    run(scene, (frame) => (frame % 2 === 0 ? [1, 1, 1, 1] : [0, 0, 0, 0]), 60);
    expect(scene.jitter.every((offset) => offset === 0)).toBe(true);
    expect(scene.arcs.every((arc) => arc.lifeMs === 0)).toBe(true);
    expect(scene.sparks.life.every((life) => life === 0)).toBe(true);
  });

  it("draws no arcs or sparks when they are turned off", () => {
    const scene = createElectricScene(
      sceneOptions({ arcs: false, sparks: false })
    );
    run(scene, (frame) => (frame % 4 === 0 ? [0, 0, 0, 0] : [1, 1, 1, 1]), 120);
    expect(scene.arcs.every((arc) => arc.lifeMs === 0)).toBe(true);
    expect(scene.sparks.life.every((life) => life === 0)).toBe(true);
  });
});

/** Distinct x positions drawn, rounded to a tenth of a pixel. */
const distinctX = (points: [number, number][]) =>
  new Set(points.map(([x]) => Math.round(x * 10))).size;

describe("ElectricBarVisualizer", () => {
  it("renders a labelled image over two hidden canvases", () => {
    stubCanvas();
    const { container, getByRole } = render(
      <ElectricBarVisualizer align="end" aria-label="Voice" loading />
    );
    const root = getByRole("img", { name: "Voice" });
    expect(root).toHaveAttribute("data-slot", "electric-bar-visualizer");
    expect(root).toHaveAttribute("data-orientation", "horizontal");
    expect(root).toHaveAttribute("data-align", "end");
    expect(root).toHaveAttribute("data-loading");
    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(2);
    for (const canvas of canvases) {
      expect(canvas).toHaveAttribute("aria-hidden", "true");
    }
  });

  it("paints declarative levels and marks an active signal", () => {
    const recorder = stubCanvas();
    const { getByRole, rerender } = render(
      <ElectricBarVisualizer barCount={4} levels={[0, 0, 0, 0]} />
    );
    advance(100);
    const root = getByRole("img");
    expect(recorder.strokes).toBeGreaterThan(0);
    expect(root).not.toHaveAttribute("data-active");
    rerender(
      <ElectricBarVisualizer barCount={4} levels={[0.8, 0.5, 0.2, 1]} />
    );
    advance(100);
    expect(root).toHaveAttribute("data-active");
  });

  it("bends the filaments more as intensity rises", () => {
    const quiet = stubCanvas();
    const { unmount } = render(
      <ElectricBarVisualizer
        barCount={4}
        intensity={0}
        levels={[0.2, 0.2, 0.2, 0.2]}
      />
    );
    advance(100);
    unmount();
    expect(distinctX(quiet.points)).toBe(4);

    vi.restoreAllMocks();
    const charged = stubCanvas();
    render(
      <ElectricBarVisualizer
        barCount={4}
        intensity={1}
        levels={[0.2, 0.2, 0.2, 0.2]}
      />
    );
    advance(100);
    expect(distinctX(charged.points)).toBeGreaterThan(4);
  });

  it("stops painting once unmounted", () => {
    const recorder = stubCanvas();
    const { unmount } = render(<ElectricBarVisualizer levels={[0.5]} />);
    advance(100);
    unmount();
    const { strokes } = recorder;
    advance(200);
    expect(recorder.strokes).toBe(strokes);
  });
});
