"use client";

import {
  useCallback,
  useEffect,
  useEffectEvent,
  useImperativeHandle,
  useRef,
} from "react";
import type { ComponentProps, Ref } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { useVisibility } from "@/hooks/use-visibility";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, VisualFrame } from "@/lib/audio/types";
import { createWaveLine } from "@/lib/audio/wave-line";
import type { WaveLine, WaveLineMode } from "@/lib/audio/wave-line";
import { cn } from "@/lib/utils";

const DEFAULT_LINE_WIDTH = 2;
const REDUCED_MOTION_INTERVAL_MS = 250;
const COLOR_REFRESH_FRAMES = 30;
/** One point every this many pixels along the line. */
const POINT_SPACING_PX = 3;
/** Width of the faded ends, in pixels. */
const FADE_PX = 24;

export type SmoothWaveformMode = WaveLineMode;

export interface SmoothWaveformActions {
  /** Paint a frame directly. */
  paint: (frame: VisualFrame) => void;
  /** Forget the last frame, so the line falls flat. */
  clear: () => void;
}

export interface SmoothWaveformProps extends ComponentProps<"div"> {
  source?: FrameSource<VisualFrame> | null;
  /**
   * `wave` draws a smooth wave shaped by the frequency bands. `scope` draws
   * the signal itself, like an oscilloscope. Default `wave`.
   */
  mode?: SmoothWaveformMode;
  /** Runs a pulse along the line, for connecting or thinking states. Default false. */
  loading?: boolean;
  /** Visual gain. Default 1. */
  sensitivity?: number;
  /** Line width in pixels. Default 2. */
  lineWidth?: number;
  /** Fade the left and right ends. Default true. */
  fadeEdges?: boolean;
  actionsRef?: Ref<SmoothWaveformActions>;
}

interface Size {
  width: number;
  height: number;
  ratio: number;
}

const drawLine = (
  context: CanvasRenderingContext2D,
  line: WaveLine,
  size: Size,
  lineWidth: number
) => {
  const middle = size.height / 2;
  const reach = Math.max(0, middle - lineWidth);
  context.lineWidth = lineWidth;
  context.lineCap = "round";
  context.lineJoin = "round";
  context.beginPath();
  for (let point = 0; point < line.count; point += 1) {
    const x = (point / (line.count - 1)) * size.width;
    const y = middle - (line.heights[point] ?? 0) * reach;
    if (point === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
  context.stroke();
};

/** Cuts the ends away with a gradient, so the line fades in and out. */
const fadeEnds = (context: CanvasRenderingContext2D, size: Size) => {
  const edge = Math.min(FADE_PX, size.width / 2);
  context.globalCompositeOperation = "destination-out";
  const left = context.createLinearGradient(0, 0, edge, 0);
  left.addColorStop(0, "rgba(0, 0, 0, 1)");
  left.addColorStop(1, "rgba(0, 0, 0, 0)");
  context.fillStyle = left;
  context.fillRect(0, 0, edge, size.height);
  const right = context.createLinearGradient(
    size.width - edge,
    0,
    size.width,
    0
  );
  right.addColorStop(0, "rgba(0, 0, 0, 0)");
  right.addColorStop(1, "rgba(0, 0, 0, 1)");
  context.fillStyle = right;
  context.fillRect(size.width - edge, 0, edge, size.height);
  context.globalCompositeOperation = "source-over";
};

export const SmoothWaveform = ({
  source,
  mode = "wave",
  loading = false,
  sensitivity = 1,
  lineWidth = DEFAULT_LINE_WIDTH,
  fadeEdges = true,
  actionsRef,
  className,
  ref,
  ...props
}: SmoothWaveformProps) => {
  const reducedMotion = useReducedMotion();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const visibleRef = useVisibility(rootRef);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<VisualFrame | null>(null);

  const paint = useCallback((frame: VisualFrame) => {
    frameRef.current = frame;
  }, []);

  useFrameSource(source, paint);

  useImperativeHandle(
    actionsRef,
    () => ({
      clear: () => {
        frameRef.current = null;
      },
      paint,
    }),
    [paint]
  );

  // Stroke options are read each paint, so changing them doesn't rebuild the
  // line and snap it flat.
  const readStroke = useEffectEvent(() => ({
    fade: fadeEdges,
    width: lineWidth,
  }));

  useEffect(() => {
    const root = rootRef.current;
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(root && canvas && context)) {
      return;
    }
    const line = createWaveLine({ loading, mode, reducedMotion, sensitivity });
    const size: Size = { height: 0, ratio: 1, width: 0 };
    let { color } = getComputedStyle(canvas);
    let framesSinceColor = 0;
    let lastPaintMs = 0;
    let active = false;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      size.ratio = window.devicePixelRatio || 1;
      size.width = rect.width;
      size.height = rect.height;
      canvas.width = Math.max(1, Math.round(rect.width * size.ratio));
      canvas.height = Math.max(1, Math.round(rect.height * size.ratio));
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const tick = (nowMs: number) => {
      if (!visibleRef.current || size.width === 0) {
        return;
      }
      framesSinceColor += 1;
      if (framesSinceColor >= COLOR_REFRESH_FRAMES) {
        framesSinceColor = 0;
        ({ color } = getComputedStyle(canvas));
      }
      if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
        return;
      }
      lastPaintMs = nowMs;
      const count = Math.round(size.width / POINT_SPACING_PX) + 1;
      const nextActive = line.step(nowMs, frameRef.current, count);
      if (nextActive !== active) {
        active = nextActive;
        root.toggleAttribute("data-active", active);
      }
      context.setTransform(size.ratio, 0, 0, size.ratio, 0, 0);
      context.clearRect(0, 0, size.width, size.height);
      context.strokeStyle = color;
      const stroke = readStroke();
      drawLine(context, line, size, clamp(stroke.width, 0.5, size.height / 2));
      if (stroke.fade) {
        fadeEnds(context, size);
      }
    };

    const unsubscribe = subscribeFrame(tick);
    return () => {
      unsubscribe();
      observer.disconnect();
      delete root.dataset.active;
    };
  }, [loading, mode, reducedMotion, sensitivity, visibleRef]);

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

  return (
    <div
      aria-label="Audio waveform"
      className={cn(
        "relative h-24 w-full [--waveform:currentColor]",
        className
      )}
      data-loading={loading ? "" : undefined}
      data-mode={mode}
      data-slot="smooth-waveform"
      role="img"
      {...props}
      ref={setRootRef}
    >
      <canvas
        aria-hidden
        className="absolute inset-0 size-full text-(--waveform)"
        data-slot="smooth-waveform-canvas"
        ref={canvasRef}
      />
    </div>
  );
};
