"use client";

import { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import type { ComponentProps, CSSProperties, Ref, RefObject } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { createBarLevels } from "@/lib/audio/bar-levels";
import type { BarIdle, BarLevelsOptions } from "@/lib/audio/bar-levels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, Orientation, VisualFrame } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const DEFAULT_BAR_COUNT = 24;
const DEFAULT_MIN_LEVEL = 0.08;
const REDUCED_MOTION_INTERVAL_MS = 250;

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
  idle?: BarIdle;
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

interface BarPainterOptions extends BarLevelsOptions {
  bars: (HTMLSpanElement | null)[];
  input: RefObject<ArrayLike<number> | null>;
  visible: RefObject<boolean>;
}

/** Paints bars outside React, with release smoothing and idle animations. */
const createBarPainter = (options: BarPainterOptions) => {
  const { bars, reducedMotion } = options;
  const barLevels = createBarLevels(options);
  const shown = new Float32Array(options.barCount).fill(-1);
  let lastPaintMs = 0;

  return (nowMs: number) => {
    if (!options.visible.current) {
      return;
    }
    if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
      return;
    }
    lastPaintMs = nowMs;
    barLevels.step(nowMs, options.input.current);

    for (const [index, value] of barLevels.levels.entries()) {
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
