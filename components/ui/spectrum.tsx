"use client";

import { createContext, useContext, useEffect, useMemo, useRef } from "react";
import type { ComponentProps, CSSProperties, RefObject } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { useVisibility } from "@/hooks/use-visibility";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { linearTaper, logTaper } from "@/lib/audio/taper";
import type { FrameSource, Taper, VisualFrame } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const COLOR_REFRESH_FRAMES = 30;
const PEAK_RELEASE_PER_FRAME = 0.985;
/** The frame length PEAK_RELEASE_PER_FRAME is tuned for. */
const FRAME_MS = 1000 / 60;
const REDUCED_MOTION_INTERVAL_MS = 250;
const LEVEL_TICK_DB = 12;
const DEFAULT_FREQUENCY_TICKS = [100, 1000, 10_000];

interface SpectrumContextValue {
  minDb: number;
  maxDb: number;
  minHz: number;
  maxHz: number;
  frequencyTaper: Taper;
  variant: "bars" | "line" | "area";
  peakHold: boolean;
  grid: boolean;
  frameRef: RefObject<VisualFrame | null>;
  /** Bumped on every frame, so each canvas knows whether it has painted it. */
  versionRef: RefObject<number>;
}

const SpectrumContext = createContext<SpectrumContextValue | null>(null);

const useSpectrum = (part: string) => {
  const context = useContext(SpectrumContext);
  if (!context) {
    throw new Error(`${part} must be used inside Spectrum.`);
  }
  return context;
};

const formatHz = (hz: number) => (hz >= 1000 ? `${hz / 1000}k` : String(hz));

const formatLevel = (db: number) => String(Math.round(db));

interface Size {
  width: number;
  height: number;
  ratio: number;
}

interface Colors {
  grid: string;
  line: string;
  peak: string;
}

const readColors = (canvas: HTMLCanvasElement): Colors => {
  const style = getComputedStyle(canvas);
  return {
    grid: style.getPropertyValue("--spectrum-grid").trim() || style.color,
    line: style.getPropertyValue("--spectrum").trim() || style.color,
    peak: style.getPropertyValue("--spectrum-peak").trim() || style.color,
  };
};

const drawGrid = (
  context: CanvasRenderingContext2D,
  size: Size,
  settings: SpectrumContextValue,
  color: string
) => {
  const { frequencyTaper, maxDb, maxHz, minDb, minHz } = settings;
  const span = maxDb - minDb;
  context.strokeStyle = color;
  context.lineWidth = 1;
  context.beginPath();
  for (const db of [maxDb, maxDb - span / 3, maxDb - (span * 2) / 3]) {
    const y = Math.round((1 - (db - minDb) / span) * size.height) + 0.5;
    context.moveTo(0, y);
    context.lineTo(size.width, y);
  }
  for (const hz of DEFAULT_FREQUENCY_TICKS) {
    if (hz > minHz && hz < maxHz) {
      const x = Math.round(frequencyTaper.toPosition(hz) * size.width) + 0.5;
      context.moveTo(x, 0);
      context.lineTo(x, size.height);
    }
  }
  context.stroke();
};

const drawBars = (
  context: CanvasRenderingContext2D,
  size: Size,
  bands: Float32Array
) => {
  const slot = size.width / bands.length;
  const gap = Math.min(2, slot * 0.25);
  for (const [index, band] of bands.entries()) {
    const barHeight = clamp(band, 0, 1) * size.height;
    context.fillRect(
      index * slot + gap / 2,
      size.height - barHeight,
      Math.max(1, slot - gap),
      barHeight
    );
  }
};

const drawCurve = (
  context: CanvasRenderingContext2D,
  size: Size,
  bands: Float32Array,
  filled: boolean
) => {
  const slot = size.width / bands.length;
  context.lineWidth = 1.5;
  context.lineJoin = "round";
  context.beginPath();
  for (const [index, band] of bands.entries()) {
    const x = (index + 0.5) * slot;
    const y = size.height - clamp(band, 0, 1) * size.height;
    if (index === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
  if (filled) {
    context.lineTo((bands.length - 0.5) * slot, size.height);
    context.lineTo(0.5 * slot, size.height);
    context.closePath();
    context.globalAlpha = 0.3;
    context.fill();
    context.globalAlpha = 1;
  }
  context.stroke();
};

/** Draws held peaks and reports whether any are still falling. */
const drawPeaks = (
  context: CanvasRenderingContext2D,
  size: Size,
  bands: Float32Array,
  peaks: Float32Array,
  release: number
) => {
  const slot = size.width / bands.length;
  let falling = false;
  for (const [index, band] of bands.entries()) {
    const level = clamp(band, 0, 1);
    const held = Math.max(level, (peaks[index] ?? 0) * release);
    peaks[index] = held;
    if (held > level + 0.001) {
      falling = true;
    }
    context.fillRect(
      index * slot,
      size.height - held * size.height - 1,
      Math.max(1, slot - 1),
      2
    );
  }
  return falling;
};

export const SpectrumCanvas = ({
  className,
  ...props
}: ComponentProps<"canvas">) => {
  const settings = useSpectrum("SpectrumCanvas");
  const reducedMotion = useReducedMotion();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const visibleRef = useVisibility(canvasRef);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(canvas && context)) {
      return;
    }
    const { frameRef, grid, peakHold, variant, versionRef } = settings;
    const size: Size = { height: 0, ratio: 1, width: 0 };
    let colors = readColors(canvas);
    let framesSinceColor = 0;
    let peaks = new Float32Array(0);
    let lastPaintMs = 0;
    // Each canvas keeps its own state, so two in one Spectrum both paint.
    let dirty = true;
    let paintedVersion = -1;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      size.ratio = window.devicePixelRatio || 1;
      size.width = rect.width;
      size.height = rect.height;
      canvas.width = Math.max(1, Math.round(rect.width * size.ratio));
      canvas.height = Math.max(1, Math.round(rect.height * size.ratio));
      dirty = true;
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const shouldPaint = (nowMs: number) => {
      framesSinceColor += 1;
      if (framesSinceColor >= COLOR_REFRESH_FRAMES) {
        framesSinceColor = 0;
        const next = readColors(canvas);
        if (
          next.grid !== colors.grid ||
          next.line !== colors.line ||
          next.peak !== colors.peak
        ) {
          colors = next;
          dirty = true;
        }
      }
      const stale = dirty || versionRef.current !== paintedVersion;
      if (!stale || size.width === 0 || !visibleRef.current) {
        return false;
      }
      return (
        !reducedMotion || nowMs - lastPaintMs >= REDUCED_MOTION_INTERVAL_MS
      );
    };

    const draw = (nowMs: number) => {
      if (!shouldPaint(nowMs)) {
        return;
      }
      const elapsedMs = lastPaintMs === 0 ? 0 : nowMs - lastPaintMs;
      lastPaintMs = nowMs;
      dirty = false;
      paintedVersion = versionRef.current;
      context.setTransform(size.ratio, 0, 0, size.ratio, 0, 0);
      context.clearRect(0, 0, size.width, size.height);
      if (grid) {
        drawGrid(context, size, settings, colors.grid);
      }
      const bands = frameRef.current?.bands;
      if (!bands || bands.length === 0) {
        return;
      }
      context.fillStyle = colors.line;
      context.strokeStyle = colors.line;
      if (variant === "bars") {
        drawBars(context, size, bands);
      } else {
        drawCurve(context, size, bands, variant === "area");
      }
      if (peakHold) {
        if (peaks.length !== bands.length) {
          peaks = new Float32Array(bands.length);
        }
        context.fillStyle = colors.peak;
        // Decay by elapsed time, not per paint, so peaks fall at the same
        // speed at 60 Hz, at 120 Hz and with reduced motion.
        dirty = drawPeaks(
          context,
          size,
          bands,
          peaks,
          PEAK_RELEASE_PER_FRAME ** (elapsedMs / FRAME_MS)
        );
      }
    };

    const unsubscribe = subscribeFrame(draw);
    return () => {
      unsubscribe();
      observer.disconnect();
    };
  }, [reducedMotion, settings, visibleRef]);

  return (
    <canvas
      aria-hidden
      className={cn(
        "[grid-column:2] [grid-row:1] size-full min-h-0",
        className
      )}
      data-slot="spectrum-canvas"
      ref={canvasRef}
      {...props}
    />
  );
};

export interface SpectrumFrequencyAxisProps extends ComponentProps<"div"> {
  /** Default 100 Hz, 1 kHz and 10 kHz. */
  ticks?: number[];
  format?: (hz: number) => string;
}

export const SpectrumFrequencyAxis = ({
  ticks = DEFAULT_FREQUENCY_TICKS,
  format = formatHz,
  className,
  ...props
}: SpectrumFrequencyAxisProps) => {
  const { frequencyTaper, maxHz, minHz } = useSpectrum("SpectrumFrequencyAxis");
  return (
    <div
      aria-hidden
      className={cn(
        "text-muted-foreground relative [grid-column:2] [grid-row:2] h-4 text-[0.625rem] tabular-nums",
        className
      )}
      data-slot="spectrum-frequency-axis"
      {...props}
    >
      {ticks
        .filter((hz) => hz >= minHz && hz <= maxHz)
        .map((hz) => (
          <span
            className="absolute top-0 left-(--tick-position) -translate-x-1/2"
            key={hz}
            style={
              {
                "--tick-position": `${frequencyTaper.toPosition(hz) * 100}%`,
              } as CSSProperties
            }
          >
            {format(hz)}
          </span>
        ))}
    </div>
  );
};

export interface SpectrumLevelAxisProps extends ComponentProps<"div"> {
  /** Default: every 12 dB inside the range. */
  ticks?: number[];
  format?: (db: number) => string;
}

export const SpectrumLevelAxis = ({
  ticks,
  format = formatLevel,
  className,
  ...props
}: SpectrumLevelAxisProps) => {
  const { maxDb, minDb } = useSpectrum("SpectrumLevelAxis");
  const values =
    ticks ??
    Array.from(
      { length: Math.floor((maxDb - minDb) / LEVEL_TICK_DB) + 1 },
      (_, index) => maxDb - index * LEVEL_TICK_DB
    );

  return (
    <div
      aria-hidden
      className={cn(
        "text-muted-foreground relative [grid-column:1] [grid-row:1] w-7 text-[0.625rem] tabular-nums",
        className
      )}
      data-slot="spectrum-level-axis"
      {...props}
    >
      {values.map((db) => (
        <span
          className="absolute right-0 bottom-(--tick-position) translate-y-1/2"
          key={db}
          style={
            {
              "--tick-position": `${((db - minDb) / (maxDb - minDb)) * 100}%`,
            } as CSSProperties
          }
        >
          {format(db)}
        </span>
      ))}
    </div>
  );
};

export interface SpectrumProps extends ComponentProps<"div"> {
  source?: FrameSource<VisualFrame> | null;
  /** Default `bars`. */
  variant?: "bars" | "line" | "area";
  /** The level range the bands cover. Match your analyser. Default −100 / −30. */
  minDb?: number;
  maxDb?: number;
  /** The frequency range the bands cover. Match your analyser. Default 40 / 16000. */
  minHz?: number;
  maxHz?: number;
  /** Frequency axis law. Default `log`. */
  scale?: "log" | "linear";
  /** Keep falling peak markers. Default false. */
  peakHold?: boolean;
  /** Draw grid lines. Default true. */
  grid?: boolean;
}

export const Spectrum = ({
  source,
  variant = "bars",
  minDb = -100,
  maxDb = -30,
  minHz = 40,
  maxHz = 16_000,
  scale = "log",
  peakHold = false,
  grid = true,
  className,
  children,
  ...props
}: SpectrumProps) => {
  const frameRef = useRef<VisualFrame | null>(null);
  const versionRef = useRef(0);

  useFrameSource(source, (frame) => {
    frameRef.current = frame;
    versionRef.current += 1;
  });

  const frequencyTaper = useMemo(
    () =>
      scale === "log" ? logTaper(minHz, maxHz) : linearTaper(minHz, maxHz),
    [maxHz, minHz, scale]
  );

  const contextValue = useMemo<SpectrumContextValue>(
    () => ({
      frameRef,
      frequencyTaper,
      grid,
      maxDb,
      maxHz,
      minDb,
      minHz,
      peakHold,
      variant,
      versionRef,
    }),
    [frequencyTaper, grid, maxDb, maxHz, minDb, minHz, peakHold, variant]
  );

  return (
    <SpectrumContext.Provider value={contextValue}>
      <div
        aria-label="Frequency spectrum"
        className={cn(
          "grid h-40 w-full grid-cols-[auto_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] gap-1 [--spectrum-grid:var(--border)] [--spectrum-peak:var(--foreground)] [--spectrum:var(--primary)]",
          className
        )}
        data-slot="spectrum"
        data-variant={variant}
        role="img"
        {...props}
      >
        {children ?? (
          <>
            <SpectrumLevelAxis />
            <SpectrumCanvas />
            <SpectrumFrequencyAxis />
          </>
        )}
      </div>
    </SpectrumContext.Provider>
  );
};
