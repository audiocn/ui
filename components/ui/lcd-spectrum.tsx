"use client";

import { useEffect, useRef } from "react";
import type { ComponentProps } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { useVisibility } from "@/hooks/use-visibility";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, VisualFrame } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const DEFAULT_ROWS = 24;
const CELL_GAP = 2;
const COLOR_REFRESH_FRAMES = 30;
/** How long a peak cap sticks at the top before it starts to fall. */
const PEAK_HOLD_MS = 400;
/** Fall acceleration, in rows per second squared per row of height. */
const PEAK_GRAVITY_PER_ROW = 12;
/** Caps the step after a long pause, such as a background tab. */
const MAX_STEP_SECONDS = 0.1;

/** A falling peak cap, in rows from the floor. Mutated in place each frame. */
export interface PeakCap {
  /** Where the cap is now, in rows. */
  row: number;
  /** Where it was when it stopped sticking, in rows. */
  startRow: number;
  /** Fall speed, in rows per second. */
  velocity: number;
  /** Until this time it sticks at the top. */
  holdUntilMs: number;
}

export interface PeakCapStep {
  /** The cap's opacity: 1 when it starts to fall, 0 when it reaches the bar. */
  alpha: number;
}

export const createPeakCap = (): PeakCap => ({
  holdUntilMs: 0,
  row: 0,
  startRow: 0,
  velocity: 0,
});

/**
 * Advances one column's peak cap by `dtSeconds`. A rising level pushes the cap
 * up and makes it stick. Once the hold ends the cap accelerates down, and it
 * never falls below the level of the bar under it.
 */
export const stepPeakCap = (
  cap: PeakCap,
  level: number,
  nowMs: number,
  dtSeconds: number,
  rows: number
): PeakCapStep => {
  if (level >= cap.row) {
    cap.row = level;
    cap.startRow = level;
    cap.velocity = 0;
    cap.holdUntilMs = nowMs + PEAK_HOLD_MS;
    return { alpha: 0 };
  }
  if (nowMs >= cap.holdUntilMs) {
    cap.velocity += PEAK_GRAVITY_PER_ROW * rows * dtSeconds;
    cap.row = Math.max(level, cap.row - cap.velocity * dtSeconds);
  }
  const fallen = cap.startRow - cap.row;
  const toBar = cap.startRow - level;
  return {
    alpha: toBar > 0 ? clamp(1 - fallen / toBar, 0, 1) : 0,
  };
};

interface Size {
  width: number;
  height: number;
  ratio: number;
}

interface Colors {
  lit: string;
  unlit: string;
}

const readColors = (canvas: HTMLCanvasElement): Colors => {
  const style = getComputedStyle(canvas);
  return {
    lit: style.getPropertyValue("--lcd-display-lit").trim() || style.color,
    unlit:
      style.getPropertyValue("--lcd-display-ink-unlit").trim() || style.color,
  };
};

export interface LcdSpectrumProps extends ComponentProps<"canvas"> {
  source?: FrameSource<VisualFrame> | null;
  /** Lit segments per column. Default 24. */
  rows?: number;
  /** Sticky peak caps that fade and accelerate as they fall. Default true. */
  peakHold?: boolean;
}

/**
 * A spectrum analyzer drawn as LCD segments: one column per band, unlit dots
 * behind, and lit segments in the display's ink. A peak cap sticks at the top
 * of each column, then falls, fading out as it accelerates towards the bar.
 * The caps are painted first, so the bars always cover them.
 */
export const LcdSpectrum = ({
  source = null,
  rows = DEFAULT_ROWS,
  peakHold = true,
  className,
  ...props
}: LcdSpectrumProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef = useRef<VisualFrame | null>(null);
  const versionRef = useRef(0);
  const reducedMotion = useReducedMotion();
  const visibleRef = useVisibility(canvasRef);

  useFrameSource(source, (frame) => {
    frameRef.current = frame;
    versionRef.current += 1;
  });

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(canvas && context)) {
      return;
    }
    const size: Size = { height: 0, ratio: 1, width: 0 };
    let colors = readColors(canvas);
    let framesSinceColor = 0;
    let caps: PeakCap[] = [];
    let alphas = new Float64Array(0);
    let paintedVersion = -1;
    let capsVisible = false;
    let lastNowMs: number | null = null;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      size.ratio = window.devicePixelRatio || 1;
      size.width = rect.width;
      size.height = rect.height;
      canvas.width = Math.max(1, Math.round(rect.width * size.ratio));
      canvas.height = Math.max(1, Math.round(rect.height * size.ratio));
      paintedVersion = -1;
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const refreshColors = () => {
      framesSinceColor += 1;
      if (framesSinceColor < COLOR_REFRESH_FRAMES) {
        return;
      }
      framesSinceColor = 0;
      const next = readColors(canvas);
      if (next.lit !== colors.lit || next.unlit !== colors.unlit) {
        colors = next;
        paintedVersion = -1;
      }
    };

    const stepCaps = (
      bands: Float32Array,
      nowMs: number,
      dtSeconds: number
    ) => {
      if (caps.length !== bands.length) {
        caps = Array.from({ length: bands.length }, createPeakCap);
        alphas = new Float64Array(bands.length);
      }
      capsVisible = false;
      for (const [index, band] of bands.entries()) {
        const cap = caps[index];
        if (!cap) {
          continue;
        }
        const level = clamp(band, 0, 1) * rows;
        const { alpha } = stepPeakCap(cap, level, nowMs, dtSeconds, rows);
        alphas[index] = alpha;
        if (alpha > 0) {
          capsVisible = true;
        }
      }
    };

    const paint = (bands: Float32Array | undefined) => {
      context.setTransform(size.ratio, 0, 0, size.ratio, 0, 0);
      context.clearRect(0, 0, size.width, size.height);
      if (!bands || bands.length === 0) {
        return;
      }
      const slot = size.width / bands.length;
      const rowHeight = size.height / rows;
      const cellWidth = Math.max(1, slot - CELL_GAP);
      const cellHeight = Math.max(1, rowHeight - CELL_GAP);
      const cellX = (column: number) => column * slot + CELL_GAP / 2;
      const cellY = (row: number) =>
        size.height - (row + 1) * rowHeight + CELL_GAP / 2;

      context.fillStyle = colors.unlit;
      for (let column = 0; column < bands.length; column += 1) {
        for (let row = 0; row < rows; row += 1) {
          context.fillRect(cellX(column), cellY(row), cellWidth, cellHeight);
        }
      }

      context.fillStyle = colors.lit;
      if (peakHold && !reducedMotion) {
        for (const [column, cap] of caps.entries()) {
          const cellRow = Math.floor(cap.row);
          const alpha = alphas[column] ?? 0;
          if (cellRow < 0 || cellRow >= rows || alpha === 0) {
            continue;
          }
          context.globalAlpha = alpha;
          context.fillRect(
            cellX(column),
            cellY(cellRow),
            cellWidth,
            cellHeight
          );
        }
        context.globalAlpha = 1;
      }

      for (const [column, band] of bands.entries()) {
        const litRows = Math.floor(clamp(band, 0, 1) * rows);
        for (let row = 0; row < litRows; row += 1) {
          context.fillRect(cellX(column), cellY(row), cellWidth, cellHeight);
        }
      }
    };

    const draw = (nowMs: number) => {
      refreshColors();
      const bands = frameRef.current?.bands;
      const dtSeconds =
        lastNowMs === null
          ? 0
          : Math.min((nowMs - lastNowMs) / 1000, MAX_STEP_SECONDS);
      lastNowMs = nowMs;
      if (peakHold && !reducedMotion && bands) {
        stepCaps(bands, nowMs, dtSeconds);
      }
      const hasNewFrame = versionRef.current !== paintedVersion;
      if (
        size.width === 0 ||
        !visibleRef.current ||
        !(hasNewFrame || capsVisible)
      ) {
        return;
      }
      paintedVersion = versionRef.current;
      paint(bands);
    };

    const unsubscribe = subscribeFrame(draw);
    return () => {
      unsubscribe();
      observer.disconnect();
    };
  }, [peakHold, reducedMotion, rows, visibleRef]);

  return (
    <canvas
      aria-label="Frequency spectrum"
      className={cn("block h-32 min-h-0 w-full", className)}
      data-slot="lcd-spectrum"
      ref={canvasRef}
      role="img"
      {...props}
    />
  );
};
