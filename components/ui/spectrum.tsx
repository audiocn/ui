"use client";

import { createContext, useContext, useEffect, useMemo, useRef } from "react";
import type { ComponentProps } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { linearTaper, logTaper } from "@/lib/audio/taper";
import type { FrameSource, Taper, VisualFrame } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const COLOR_REFRESH_FRAMES = 30;
const PEAK_RELEASE_PER_FRAME = 0.985;
const REDUCED_MOTION_INTERVAL_MS = 250;

interface SpectrumContextValue {
  minDb: number;
  maxDb: number;
  minHz: number;
  maxHz: number;
  frequencyTaper: Taper;
  variant: "bars" | "line" | "area";
  peakHold: boolean;
  grid: boolean;
  frameRef: { current: VisualFrame | null };
  dirtyRef: { current: boolean };
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
  const dirtyRef = useRef(true);

  useFrameSource(source, (frame) => {
    frameRef.current = frame;
    dirtyRef.current = true;
  });

  const frequencyTaper = useMemo(
    () =>
      scale === "log" ? logTaper(minHz, maxHz) : linearTaper(minHz, maxHz),
    [maxHz, minHz, scale]
  );

  const contextValue = useMemo<SpectrumContextValue>(
    () => ({
      dirtyRef,
      frameRef,
      frequencyTaper,
      grid,
      maxDb,
      maxHz,
      minDb,
      minHz,
      peakHold,
      variant,
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

export const SpectrumCanvas = ({
  className,
  ...props
}: ComponentProps<"canvas">) => {
  const {
    dirtyRef,
    frameRef,
    frequencyTaper,
    grid,
    maxDb,
    maxHz,
    minDb,
    minHz,
    peakHold,
    variant,
  } = useSpectrum("SpectrumCanvas");
  const reducedMotion = useReducedMotion();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(canvas && context)) {
      return;
    }
    const size = { height: 0, ratio: 1, width: 0 };
    let colors = { grid: "", line: "", peak: "" };
    let framesSinceColor = COLOR_REFRESH_FRAMES;
    let peaks = new Float32Array(0);
    let lastPaintMs = 0;
    const gridDb = [
      maxDb,
      maxDb - (maxDb - minDb) / 3,
      maxDb - ((maxDb - minDb) * 2) / 3,
    ];
    const gridHz = [100, 1000, 10_000].filter((hz) => hz > minHz && hz < maxHz);

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      size.ratio = window.devicePixelRatio || 1;
      size.width = rect.width;
      size.height = rect.height;
      canvas.width = Math.max(1, Math.round(rect.width * size.ratio));
      canvas.height = Math.max(1, Math.round(rect.height * size.ratio));
      dirtyRef.current = true;
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const draw = (nowMs: number) => {
      framesSinceColor += 1;
      if (framesSinceColor >= COLOR_REFRESH_FRAMES) {
        framesSinceColor = 0;
        const style = getComputedStyle(canvas);
        colors = {
          grid: style.getPropertyValue("--spectrum-grid").trim() || "gray",
          line: style.getPropertyValue("--spectrum").trim() || style.color,
          peak: style.getPropertyValue("--spectrum-peak").trim() || style.color,
        };
        dirtyRef.current = true;
      }
      if (!dirtyRef.current || size.width === 0) {
        return;
      }
      if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
        return;
      }
      lastPaintMs = nowMs;
      dirtyRef.current = false;
      const { height, width } = size;
      context.setTransform(size.ratio, 0, 0, size.ratio, 0, 0);
      context.clearRect(0, 0, width, height);

      if (grid) {
        context.strokeStyle = colors.grid;
        context.lineWidth = 1;
        context.beginPath();
        for (const db of gridDb) {
          const y =
            Math.round((1 - (db - minDb) / (maxDb - minDb)) * height) + 0.5;
          context.moveTo(0, y);
          context.lineTo(width, y);
        }
        for (const hz of gridHz) {
          const x = Math.round(frequencyTaper.toPosition(hz) * width) + 0.5;
          context.moveTo(x, 0);
          context.lineTo(x, height);
        }
        context.stroke();
      }

      const bands = frameRef.current?.bands;
      if (!bands || bands.length === 0) {
        return;
      }
      if (peaks.length !== bands.length) {
        peaks = new Float32Array(bands.length);
      }
      const count = bands.length;
      const slot = width / count;
      context.fillStyle = colors.line;
      context.strokeStyle = colors.line;

      if (variant === "bars") {
        const gap = Math.min(2, slot * 0.25);
        for (let index = 0; index < count; index += 1) {
          const value = clamp(bands[index] ?? 0, 0, 1);
          const barHeight = value * height;
          context.fillRect(
            index * slot + gap / 2,
            height - barHeight,
            Math.max(1, slot - gap),
            barHeight
          );
        }
      } else {
        context.lineWidth = 1.5;
        context.lineJoin = "round";
        context.beginPath();
        for (let index = 0; index < count; index += 1) {
          const x = (index + 0.5) * slot;
          const y = height - clamp(bands[index] ?? 0, 0, 1) * height;
          if (index === 0) {
            context.moveTo(x, y);
          } else {
            context.lineTo(x, y);
          }
        }
        if (variant === "area") {
          context.lineTo((count - 0.5) * slot, height);
          context.lineTo(0.5 * slot, height);
          context.closePath();
          context.globalAlpha = 0.3;
          context.fill();
          context.globalAlpha = 1;
        }
        context.stroke();
      }

      if (peakHold) {
        context.fillStyle = colors.peak;
        for (let index = 0; index < count; index += 1) {
          const value = clamp(bands[index] ?? 0, 0, 1);
          const held = Math.max(
            value,
            (peaks[index] ?? 0) * PEAK_RELEASE_PER_FRAME
          );
          peaks[index] = held;
          context.fillRect(
            index * slot,
            height - held * height - 1,
            Math.max(1, slot - 1),
            2
          );
        }
        dirtyRef.current = true;
      }
    };

    const unsubscribe = subscribeFrame(draw);
    return () => {
      unsubscribe();
      observer.disconnect();
    };
  }, [
    dirtyRef,
    frameRef,
    frequencyTaper,
    grid,
    maxDb,
    maxHz,
    minDb,
    minHz,
    peakHold,
    reducedMotion,
    variant,
  ]);

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
  ticks = [100, 1000, 10_000],
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
            className="absolute top-0 -translate-x-1/2"
            key={hz}
            style={{ left: `${frequencyTaper.toPosition(hz) * 100}%` }}
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
  format = (db) => String(Math.round(db)),
  className,
  ...props
}: SpectrumLevelAxisProps) => {
  const { maxDb, minDb } = useSpectrum("SpectrumLevelAxis");
  const values =
    ticks ??
    Array.from(
      { length: Math.floor((maxDb - minDb) / 12) + 1 },
      (_, index) => maxDb - index * 12
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
          className="absolute right-0 translate-y-1/2"
          key={db}
          style={{ bottom: `${((db - minDb) / (maxDb - minDb)) * 100}%` }}
        >
          {format(db)}
        </span>
      ))}
    </div>
  );
};
