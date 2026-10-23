import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  createPeakCap,
  LcdSpectrum,
  LcdSpectrumCanvas,
  stepPeakCap,
} from "@/components/ui/lcd-spectrum";

const ROWS = 24;
const FRAME_SECONDS = 1 / 60;

describe("stepPeakCap", () => {
  it("sticks at the top of a rising level", () => {
    const cap = createPeakCap();
    stepPeakCap(cap, 10, 0, FRAME_SECONDS, ROWS);
    const step = stepPeakCap(cap, 10, 100, FRAME_SECONDS, ROWS);

    expect(cap.row).toBe(10);
    expect(cap.velocity).toBe(0);
    expect(step.alpha).toBe(0);
  });

  it("holds still until the hold ends, then falls", () => {
    const cap = createPeakCap();
    stepPeakCap(cap, 10, 0, FRAME_SECONDS, ROWS);
    stepPeakCap(cap, 4, 200, FRAME_SECONDS, ROWS);
    expect(cap.row).toBe(10);

    stepPeakCap(cap, 4, 1000, FRAME_SECONDS, ROWS);
    expect(cap.row).toBeLessThan(10);
  });

  it("accelerates as it falls", () => {
    const cap = createPeakCap();
    stepPeakCap(cap, 20, 0, FRAME_SECONDS, ROWS);
    stepPeakCap(cap, 0, 1000, FRAME_SECONDS, ROWS);
    const firstVelocity = cap.velocity;
    stepPeakCap(cap, 0, 1100, FRAME_SECONDS, ROWS);

    expect(cap.velocity).toBeGreaterThan(firstVelocity);
  });

  it("fades out as it reaches the bar and never falls below it", () => {
    const cap = createPeakCap();
    stepPeakCap(cap, 10, 0, FRAME_SECONDS, ROWS);
    let alpha = 1;
    for (let frame = 1; frame < 2000; frame += 1) {
      ({ alpha } = stepPeakCap(cap, 4, 1000 + frame * 16, FRAME_SECONDS, ROWS));
    }

    expect(cap.row).toBe(4);
    expect(alpha).toBe(0);
  });

  it("starts at full opacity when the hold ends", () => {
    const cap = createPeakCap();
    stepPeakCap(cap, 10, 0, FRAME_SECONDS, ROWS);
    const step = stepPeakCap(cap, 2, 1000, FRAME_SECONDS, ROWS);

    expect(step.alpha).toBeGreaterThan(0);
    expect(step.alpha).toBeLessThan(1);
  });
});

describe("LcdSpectrum", () => {
  it("renders an accessible image with the canvas and both axes", () => {
    const { container } = render(<LcdSpectrum />);

    const root = screen.getByRole("img", { name: "Frequency spectrum" });
    expect(root.dataset.slot).toBe("lcd-spectrum");
    for (const slot of [
      "spectrum-level-axis",
      "lcd-spectrum-canvas",
      "spectrum-frequency-axis",
    ]) {
      expect(container.querySelector(`[data-slot="${slot}"]`)).not.toBeNull();
    }
  });

  it("labels the axes from the same ranges as the Spectrum", () => {
    const { container } = render(<LcdSpectrum maxDb={0} minDb={-60} />);

    const level = container.querySelector('[data-slot="spectrum-level-axis"]');
    expect(level?.textContent).toBe("0-12-24-36-48-60");
    const frequency = container.querySelector(
      '[data-slot="spectrum-frequency-axis"]'
    );
    expect(frequency?.textContent).toBe("1001k10k");
  });

  it("draws the labels in the display's dim ink", () => {
    const { container } = render(<LcdSpectrum />);

    for (const slot of ["spectrum-level-axis", "spectrum-frequency-axis"]) {
      const axis = container.querySelector(`[data-slot="${slot}"]`);
      expect(axis).toHaveClass("text-(color:--lcd-display-ink-dim)");
      expect(axis).not.toHaveClass("text-muted-foreground");
    }
  });

  it("lets a composed canvas take its rows from the root", () => {
    const { container } = render(
      <LcdSpectrum rows={12}>
        <LcdSpectrumCanvas className="opacity-50" />
      </LcdSpectrum>
    );

    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(1);
    expect(canvases[0]).toHaveClass("opacity-50");
    expect(
      container.querySelector('[data-slot="spectrum-level-axis"]')
    ).toBeNull();
  });
});
