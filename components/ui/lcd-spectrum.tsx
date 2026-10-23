"use client";

import { createContext, useContext, useEffect, useMemo, useRef } from "react";
import type { ComponentProps } from "react";

import {
  drawSpectrumGrid,
  Spectrum,
  SpectrumFrequencyAxis,
  SpectrumLevelAxis,
  useSpectrum,
} from "@/components/ui/spectrum";
import type {
  SpectrumFrequencyAxisProps,
  SpectrumLevelAxisProps,
  SpectrumProps,
  SpectrumSize,
} from "@/components/ui/spectrum";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { useVisibility } from "@/hooks/use-visibility";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
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

interface Colors {
  grid: string;
  lit: string;
  unlit: string;
}

const readColors = (canvas: HTMLCanvasElement): Colors => {
  const style = getComputedStyle(canvas);
  return {
    grid: style.getPropertyValue("--lcd-spectrum-grid").trim() || style.color,
    lit: style.getPropertyValue("--lcd-display-lit").trim() || style.color,
    unlit:
      style.getPropertyValue("--lcd-display-ink-unlit").trim() || style.color,
  };
};

/** `bar` lights every segment up to the level; `dot` lights only the top one. */
export type LcdSpectrumMode = "bar" | "dot";

interface LcdSpectrumContextValue {
  mode: LcdSpectrumMode;
  rows: number;
}

/** The surrounding LcdSpectrum's mode and rows, so a composed canvas follows it. */
const LcdSpectrumContext = createContext<LcdSpectrumContextValue>({
  mode: "bar",
  rows: DEFAULT_ROWS,
});

export interface LcdSpectrumCanvasProps extends ComponentProps<"canvas"> {
  /** Lit segments per column. Default: the LcdSpectrum's `rows`. */
  rows?: number;
}

/**
 * The LCD segments: one column per band, unlit dots behind, and lit segments
 * in the display's ink. A peak cap sticks at the top of each column, then
 * falls, fading out as it accelerates towards the bar. The grid and the caps
 * are painted first, so the bars always cover them.
 */
export const LcdSpectrumCanvas = ({
  rows: rowsProp,
  className,
  ...props
}: LcdSpectrumCanvasProps) => {
  const settings = useSpectrum("LcdSpectrumCanvas");
  const { mode, rows: contextRows } = useContext(LcdSpectrumContext);
  const rows = rowsProp ?? contextRows;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const reducedMotion = useReducedMotion();
  const visibleRef = useVisibility(canvasRef);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(canvas && context)) {
      return;
    }
    const { frameRef, grid, peakHold, versionRef } = settings;
    const size: SpectrumSize = { height: 0, ratio: 1, width: 0 };
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
      if (
        next.grid !== colors.grid ||
        next.lit !== colors.lit ||
        next.unlit !== colors.unlit
      ) {
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
        if (grid) {
          drawSpectrumGrid(context, size, settings, colors.grid);
        }
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
      if (grid) {
        drawSpectrumGrid(context, size, settings, colors.grid);
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
        // Dot mode lights only the top segment, like the LCD Meter's.
        const firstRow = mode === "dot" ? Math.max(0, litRows - 1) : 0;
        for (let row = firstRow; row < litRows; row += 1) {
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
  }, [mode, reducedMotion, rows, settings, visibleRef]);

  return (
    <canvas
      aria-hidden
      className={cn(
        "[grid-column:2] [grid-row:1] size-full min-h-0",
        className
      )}
      data-slot="lcd-spectrum-canvas"
      ref={canvasRef}
      {...props}
    />
  );
};

/** The dB labels down the left, in the display's dim ink. */
export const LcdSpectrumLevelAxis = ({
  className,
  ...props
}: SpectrumLevelAxisProps) => (
  <SpectrumLevelAxis
    className={cn(
      "w-[2.5em] text-[0.625em] text-(color:--lcd-display-ink-dim)",
      className
    )}
    {...props}
  />
);

/** The frequency labels along the bottom, in the display's dim ink. */
export const LcdSpectrumFrequencyAxis = ({
  className,
  ...props
}: SpectrumFrequencyAxisProps) => (
  <SpectrumFrequencyAxis
    className={cn(
      "h-[1.5em] text-[0.625em] text-(color:--lcd-display-ink-dim)",
      className
    )}
    {...props}
  />
);

export interface LcdSpectrumProps extends Omit<SpectrumProps, "variant"> {
  /** Lit segments per column. Default 24. */
  rows?: number;
  /** `bar` lights every segment up to the level, `dot` only the top one. Default `bar`. */
  mode?: LcdSpectrumMode;
}

/**
 * A Spectrum drawn as LCD segments, with the same ranges, axes, grid and
 * defaults as the Spectrum. `peakHold` adds sticky peak caps, as it adds peak
 * markers to the Spectrum. Put it in an `LcdDisplay` with `matrix={false}`.
 */
export const LcdSpectrum = ({
  rows = DEFAULT_ROWS,
  mode = "bar",
  className,
  children,
  ...props
}: LcdSpectrumProps) => {
  const context = useMemo(() => ({ mode, rows }), [mode, rows]);
  return (
    <LcdSpectrumContext.Provider value={context}>
      <Spectrum
        className={cn(
          "pt-[0.75em] [--lcd-spectrum-grid:color-mix(in_oklab,var(--lcd-display-lit)_20%,transparent)]",
          className
        )}
        data-lcd-mode={mode}
        data-slot="lcd-spectrum"
        {...props}
      >
        {children ?? (
          <>
            <LcdSpectrumLevelAxis />
            <LcdSpectrumCanvas />
            <LcdSpectrumFrequencyAxis />
          </>
        )}
      </Spectrum>
    </LcdSpectrumContext.Provider>
  );
};
