import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  createPeakCap,
  LcdSpectrum,
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
  it("renders an accessible image canvas", () => {
    render(<LcdSpectrum />);

    const canvas = screen.getByRole("img", { name: "Frequency spectrum" });
    expect(canvas.tagName).toBe("CANVAS");
    expect(canvas.dataset.slot).toBe("lcd-spectrum");
  });
});
