"use client";

import { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import type { ComponentProps, Ref } from "react";

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

  useEffect(() => {
    const half = mirrored ? Math.ceil(barCount / 2) : barCount;
    const targets = new Float32Array(half);
    const current = new Float32Array(barCount);
    const shown = new Float32Array(barCount).fill(-1);
    const property = orientation === "horizontal" ? "height" : "width";
    let lastMs = 0;
    let lastPaintMs = 0;

    const tick = (nowMs: number) => {
      if (!visibleRef.current) {
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

      const input = inputRef.current;
      if (input && input.length > 0) {
        resampleLevels(input, 0, input.length, targets);
      } else {
        targets.fill(0);
      }

      let signal = 0;
      for (const value of targets) {
        signal = Math.max(signal, value);
      }
      const animateIdle = !reducedMotion && signal < SIGNAL_THRESHOLD;

      for (let index = 0; index < barCount; index += 1) {
        const distance = mirrored
          ? Math.abs(index - (barCount - 1) / 2)
          : index;
        const targetIndex = mirrored
          ? Math.min(half - 1, Math.floor(distance))
          : index;
        let target = targets[targetIndex] ?? 0;
        if (loading && !reducedMotion) {
          target = Math.max(target, sweepLevel(index, barCount, seconds));
        } else if (animateIdle) {
          target = Math.max(target, idleLevel(idle, index, seconds));
        }
        const previous = current[index] ?? 0;
        const next =
          target >= previous
            ? target
            : previous * release + target * (1 - release);
        current[index] = next;

        const value = clamp(Math.max(minLevel, next), 0, 1);
        if (Math.abs(value - (shown[index] ?? -1)) > 0.002) {
          shown[index] = value;
          const bar = barsRef.current[index];
          if (bar) {
            bar.style[property] = `${(value * 100).toFixed(2)}%`;
          }
        }
      }
    };

    return subscribeFrame(tick);
  }, [barCount, idle, loading, minLevel, mirrored, orientation, reducedMotion]);

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
              ? "max-w-(--bar-width) min-w-0 flex-1"
              : "max-h-(--bar-width) min-h-0 flex-1"
          )}
          data-index={index}
          data-slot="bar-visualizer-bar"
          key={`bar-${index}`}
          ref={(node) => {
            barsRef.current[index] = node;
          }}
          style={{ [horizontal ? "height" : "width"]: `${minLevel * 100}%` }}
        />
      ))}
    </div>
  );
};
