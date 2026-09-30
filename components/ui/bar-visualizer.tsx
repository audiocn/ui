"use client";

import { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import type { ComponentProps, CSSProperties, Ref, RefObject } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { resampleLevels } from "@/lib/audio/bands";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, Orientation, VisualFrame } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const DEFAULT_BAR_COUNT = 24;
const DEFAULT_MIN_LEVEL = 0.08;
const RELEASE_PER_FRAME = 0.86;
const FRAME_MS = 16.67;
const SIGNAL_THRESHOLD = 0.02;
const REDUCED_MOTION_INTERVAL_MS = 250;
const MS_PER_SECOND = 1000;

export interface BarVisualizerActions {
  /** Paint levels (0..1) directly. They are resampled to the bar count. */
  paint: (levels: ArrayLike<number>) => void;
}

export interface BarVisualizerProps extends ComponentProps<"div"> {
  /** A visual source; the bars follow its frequency bands. */
  source?: FrameSource<VisualFrame> | null;
  /** Levels, 0..1, for declarative use. */
  levels?: ArrayLike<number>;
  /** Number of bars. Bands are resampled to fit. Default 24. */
  barCount?: number;
  /** Where bars grow from. Default `center`. */
  align?: "center" | "start" | "end";
  /** Symmetric around the middle bar. Default false. */
  mirrored?: boolean;
  /** Resting bar size, 0..1. Default 0.08. */
  minLevel?: number;
  /** What the bars do with no signal. Default `static`. */
  idle?: "static" | "pulse" | "wave";
  /** Runs a sweep animation, for connecting or thinking states. Default false. */
  loading?: boolean;
  orientation?: Orientation;
  actionsRef?: Ref<BarVisualizerActions>;
}

const ALIGN_CLASS = {
  center: "items-center",
  end: "items-end",
  start: "items-start",
} as const;

const idleLevel = (
  idle: "static" | "pulse" | "wave",
  index: number,
  seconds: number
) => {
  if (idle === "pulse") {
    return 0.1 * (0.5 + 0.5 * Math.sin(seconds * Math.PI * 1.6));
  }
  if (idle === "wave") {
    return 0.16 * (0.5 + 0.5 * Math.sin(seconds * 4 - index * 0.55));
  }
  return 0;
};

const sweepLevel = (index: number, count: number, seconds: number) => {
  const span = count + 6;
  const position = ((seconds * count * 0.9) % span) - 3;
  return 0.55 * Math.exp(-((index - position) ** 2) / 3);
};

interface BarPainterOptions {
  barCount: number;
  bars: (HTMLSpanElement | null)[];
  idle: "static" | "pulse" | "wave";
  input: RefObject<ArrayLike<number> | null>;
  loading: boolean;
  minLevel: number;
  mirrored: boolean;
  reducedMotion: boolean;
  visible: RefObject<boolean>;
}

const targetIndexFor = (
  index: number,
  count: number,
  half: number,
  mirrored: boolean
) => {
  if (!mirrored) {
    return index;
  }
  return Math.min(half - 1, Math.floor(Math.abs(index - (count - 1) / 2)));
};

const loudestOf = (values: Float32Array) => {
  let loudest = 0;
  for (const value of values) {
    loudest = Math.max(loudest, value);
  }
  return loudest;
};

/** Paints bars outside React, with release smoothing and idle animations. */
const createBarPainter = (options: BarPainterOptions) => {
  const { barCount, bars, minLevel, mirrored, reducedMotion } = options;
  const half = mirrored ? Math.ceil(barCount / 2) : barCount;
  const targets = new Float32Array(half);
  const current = new Float32Array(barCount);
  const shown = new Float32Array(barCount).fill(-1);
  let lastMs = 0;
  let lastPaintMs = 0;

  const extraLevel = (index: number, seconds: number, quiet: boolean) => {
    if (options.loading && !reducedMotion) {
      return sweepLevel(index, barCount, seconds);
    }
    return quiet ? idleLevel(options.idle, index, seconds) : 0;
  };

  const readInput = () => {
    const input = options.input.current;
    if (input && input.length > 0) {
      resampleLevels(input, 0, input.length, targets);
    } else {
      targets.fill(0);
    }
  };

  return (nowMs: number) => {
    if (!options.visible.current) {
      return;
    }
    if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
      return;
    }
    lastPaintMs = nowMs;
    const elapsed = lastMs === 0 ? FRAME_MS : nowMs - lastMs;
    lastMs = nowMs;
    const release = reducedMotion
      ? 0
      : RELEASE_PER_FRAME ** (elapsed / FRAME_MS);
    const seconds = nowMs / MS_PER_SECOND;
    readInput();
    const quiet = !reducedMotion && loudestOf(targets) < SIGNAL_THRESHOLD;

    for (let index = 0; index < barCount; index += 1) {
      const target = Math.max(
        targets[targetIndexFor(index, barCount, half, mirrored)] ?? 0,
        extraLevel(index, seconds, quiet)
      );
      const previous = current[index] ?? 0;
      const next =
        target >= previous
          ? target
          : previous * release + target * (1 - release);
      current[index] = next;
      const value = clamp(Math.max(minLevel, next), 0, 1);
      if (Math.abs(value - (shown[index] ?? -1)) > 0.002) {
        shown[index] = value;
        bars[index]?.style.setProperty("--bar-level", value.toFixed(4));
      }
    }
  };
};

export const BarVisualizer = ({
  source,
  levels,
  barCount = DEFAULT_BAR_COUNT,
  align = "center",
  mirrored = false,
  minLevel = DEFAULT_MIN_LEVEL,
  idle = "static",
  loading = false,
  orientation: orientationProp,
  actionsRef,
  className,
  ref,
  ...props
}: BarVisualizerProps) => {
  const config = useAudioConfig();
  const orientation = orientationProp ?? config.orientation ?? "horizontal";
  const reducedMotion = useReducedMotion();
  const barsRef = useRef<(HTMLSpanElement | null)[]>([]);
  const inputRef = useRef<ArrayLike<number> | null>(null);
  const visibleRef = useRef(true);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const paint = useCallback((next: ArrayLike<number>) => {
    inputRef.current = next;
  }, []);

  useFrameSource(source, (frame) => {
    inputRef.current = frame.bands;
  });

  useEffect(() => {
    if (levels) {
      inputRef.current = levels;
    }
  }, [levels]);

  useImperativeHandle(actionsRef, () => ({ paint }), [paint]);

  useEffect(() => {
    const root = rootRef.current;
    if (!root || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        visibleRef.current = entry.isIntersecting;
      }
    });
    observer.observe(root);
    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(
    () =>
      subscribeFrame(
        createBarPainter({
          barCount,
          bars: barsRef.current,
          idle,
          input: inputRef,
          loading,
          minLevel,
          mirrored,
          reducedMotion,
          visible: visibleRef,
        })
      ),
    [barCount, idle, loading, minLevel, mirrored, reducedMotion]
  );

  const setRootRef = useCallback(
    (node: HTMLDivElement | null) => {
      rootRef.current = node;
      if (typeof ref === "function") {
        ref(node);
      } else if (ref) {
        ref.current = node;
      }
    },
    [ref]
  );

  const horizontal = orientation === "horizontal";

  return (
    <div
      aria-label="Audio visualizer"
      className={cn(
        "flex justify-center gap-(--bar-gap) [--bar-gap:0.1875rem] [--bar-radius:9999px] [--bar-width:0.375rem]",
        horizontal ? "h-16 w-full flex-row" : "h-full w-16 flex-col",
        ALIGN_CLASS[align],
        className
      )}
      data-loading={loading ? "" : undefined}
      data-orientation={orientation}
      data-slot="bar-visualizer"
      role="img"
      {...props}
      ref={setRootRef}
    >
      {Array.from({ length: barCount }, (_, index) => (
        <span
          className={cn(
            "rounded-(--bar-radius) bg-current",
            horizontal
              ? "h-[calc(var(--bar-level)*100%)] max-w-(--bar-width) min-w-0 flex-1"
              : "max-h-(--bar-width) min-h-0 w-[calc(var(--bar-level)*100%)] flex-1"
          )}
          data-index={index}
          data-slot="bar-visualizer-bar"
          key={`bar-${index}`}
          ref={(node) => {
            barsRef.current[index] = node;
          }}
          style={{ "--bar-level": minLevel } as CSSProperties}
        />
      ))}
    </div>
  );
};
