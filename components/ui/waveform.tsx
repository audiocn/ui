"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type {
  ComponentProps,
  CSSProperties,
  KeyboardEvent,
  PointerEvent,
} from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useVisibility } from "@/hooks/use-visibility";
import { resampleLevels } from "@/lib/audio/bands";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { formatTime } from "@/lib/audio/time";
import type { FrameSource } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const COLOR_REFRESH_FRAMES = 30;

interface WaveformContextValue {
  peaks: ArrayLike<number> | null;
  duration: number;
  variant: "bars" | "line" | "mirror";
  barWidth: number;
  barGap: number;
  barRadius: number;
  loading: boolean;
  /** Current progress, 0..1, read by the canvas and cursor on every frame. */
  progressRef: { current: number };
  /** The hovered time, or null. Read it with useSyncExternalStore. */
  getHover: () => number | null;
  subscribeHover: (listener: () => void) => () => void;
  /** Whether pointer seeking (and so the hover line) is on. */
  interactive: boolean;
  timeToPosition: (time: number) => number;
}

const WaveformContext = createContext<WaveformContextValue | null>(null);

const getNoHover = () => null;

const useWaveform = (part: string) => {
  const context = useContext(WaveformContext);
  if (!context) {
    throw new Error(`${part} must be used inside Waveform.`);
  }
  return context;
};

const seekTarget = (
  key: string,
  current: number,
  amount: number,
  duration: number
): number | null => {
  const targets: Record<string, number> = {
    ArrowDown: current - amount,
    ArrowLeft: current - amount,
    ArrowRight: current + amount,
    ArrowUp: current + amount,
    End: duration,
    Home: 0,
  };
  return targets[key] ?? null;
};

const defaultHoverFormat = (value: number) => formatTime(value);

const drawRoundedBar = (
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number
) => {
  if (typeof context.roundRect === "function") {
    context.beginPath();
    context.roundRect(
      x,
      y,
      width,
      height,
      Math.min(radius, width / 2, height / 2)
    );
    context.fill();
  } else {
    context.fillRect(x, y, width, height);
  }
};

export const WaveformCanvas = ({
  className,
  ...props
}: ComponentProps<"canvas">) => {
  const { barGap, barRadius, barWidth, loading, peaks, progressRef, variant } =
    useWaveform("WaveformCanvas");
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const visibleRef = useVisibility(canvasRef);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!(canvas && context)) {
      return;
    }
    const size = { height: 0, width: 0 };
    let ratio = 1;
    let colors = { played: "", unplayed: "" };
    let framesSinceColor = COLOR_REFRESH_FRAMES;
    let lastProgress = -1;
    let levels = new Float32Array(0);

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      ratio = window.devicePixelRatio || 1;
      size.width = rect.width;
      size.height = rect.height;
      canvas.width = Math.max(1, Math.round(size.width * ratio));
      canvas.height = Math.max(1, Math.round(size.height * ratio));
      const count = Math.max(
        1,
        Math.floor((size.width + barGap) / (barWidth + barGap))
      );
      levels = new Float32Array(count);
      if (peaks && peaks.length > 0) {
        resampleLevels(peaks, 0, peaks.length, levels);
      }
      lastProgress = -1;
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const drawPass = (color: string, clipWidth: number) => {
      context.save();
      context.beginPath();
      context.rect(0, 0, clipWidth, size.height);
      context.clip();
      context.fillStyle = color;
      context.strokeStyle = color;
      const pitch = barWidth + barGap;
      const middle = size.height / 2;
      if (variant === "line") {
        context.lineWidth = 1.5;
        context.beginPath();
        for (let index = 0; index < levels.length; index += 1) {
          const x = index * pitch + barWidth / 2;
          const y = middle - (levels[index] ?? 0) * (middle - 1);
          if (index === 0) {
            context.moveTo(x, y);
          } else {
            context.lineTo(x, y);
          }
        }
        for (let index = levels.length - 1; index >= 0; index -= 1) {
          const x = index * pitch + barWidth / 2;
          context.lineTo(x, middle + (levels[index] ?? 0) * (middle - 1));
        }
        context.closePath();
        context.fill();
      } else {
        for (let index = 0; index < levels.length; index += 1) {
          const level = levels[index] ?? 0;
          const barHeight = Math.max(2, level * size.height);
          const y =
            variant === "mirror"
              ? middle - barHeight / 2
              : size.height - barHeight;
          drawRoundedBar(
            context,
            index * pitch,
            y,
            barWidth,
            barHeight,
            barRadius
          );
        }
      }
      context.restore();
    };

    const draw = () => {
      framesSinceColor += 1;
      if (framesSinceColor >= COLOR_REFRESH_FRAMES) {
        framesSinceColor = 0;
        const style = getComputedStyle(canvas);
        const next = {
          played:
            style.getPropertyValue("--waveform-progress").trim() || style.color,
          unplayed: style.getPropertyValue("--waveform").trim() || style.color,
        };
        if (
          next.played !== colors.played ||
          next.unplayed !== colors.unplayed
        ) {
          colors = next;
          lastProgress = -1;
        }
      }
      const progress = progressRef.current;
      // Off screen, leave lastProgress alone so it repaints when it returns.
      if (
        progress === lastProgress ||
        size.width === 0 ||
        !visibleRef.current
      ) {
        return;
      }
      lastProgress = progress;
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, size.width, size.height);
      if (loading || !peaks) {
        return;
      }
      drawPass(colors.unplayed, size.width);
      drawPass(colors.played, progress * size.width);
    };

    const unsubscribe = subscribeFrame(draw);
    return () => {
      unsubscribe();
      observer.disconnect();
    };
  }, [
    barGap,
    barRadius,
    barWidth,
    loading,
    peaks,
    progressRef,
    variant,
    visibleRef,
  ]);

  return (
    <>
      {loading ? (
        <div
          aria-hidden
          className="bg-muted absolute inset-0 animate-pulse rounded-lg"
          data-slot="waveform-skeleton"
        />
      ) : null}
      <canvas
        aria-hidden
        className={cn("absolute inset-0 size-full", className)}
        data-slot="waveform-canvas"
        ref={canvasRef}
        {...props}
      />
    </>
  );
};

export const WaveformCursor = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    aria-hidden
    className={cn(
      "pointer-events-none absolute inset-y-0 left-[calc(var(--waveform-position)*100%)] w-0.5 -translate-x-1/2 rounded-full bg-(--waveform-cursor) group-data-loading/waveform:hidden",
      className
    )}
    data-slot="waveform-cursor"
    {...props}
  />
);

export interface WaveformHoverProps extends ComponentProps<"div"> {
  format?: (time: number) => string;
}

export const WaveformHover = ({
  format = defaultHoverFormat,
  className,
  style,
  ...props
}: WaveformHoverProps) => {
  const { getHover, interactive, subscribeHover, timeToPosition } =
    useWaveform("WaveformHover");
  const hover = useSyncExternalStore(subscribeHover, getHover, getNoHover);

  if (!interactive || hover === null) {
    return null;
  }

  return (
    <div
      aria-hidden
      className={cn(
        "bg-foreground/40 pointer-events-none absolute inset-y-0 left-(--waveform-hover) w-px",
        className
      )}
      data-slot="waveform-hover"
      style={
        {
          "--waveform-hover": `${timeToPosition(hover) * 100}%`,
          ...style,
        } as CSSProperties
      }
      {...props}
    >
      <span className="bg-foreground text-background absolute -top-6 left-1/2 -translate-x-1/2 rounded-md px-1.5 py-0.5 font-mono text-[0.625rem] whitespace-nowrap tabular-nums">
        {format(hover)}
      </span>
    </div>
  );
};

export interface WaveformRegionValue {
  start: number;
  end: number;
}

export interface WaveformRegionProps extends Omit<
  ComponentProps<"div">,
  "onChange" | "defaultValue"
> {
  start: number;
  end: number;
  onValueChange?: (value: WaveformRegionValue) => void;
  /** Drag the edges. Default true. */
  resizable?: boolean;
  /** Drag the whole region. Default true. */
  draggable?: boolean;
  /** Shortest region in seconds. Default 0.1. */
  minLength?: number;
}

interface RegionDrag {
  kind: "start" | "end" | "move";
  x: number;
  start: number;
  end: number;
}

export const WaveformRegion = ({
  start,
  end,
  onValueChange,
  resizable = true,
  draggable = true,
  minLength = 0.1,
  className,
  children,
  style,
  ...props
}: WaveformRegionProps) => {
  const { duration, timeToPosition } = useWaveform("WaveformRegion");
  const dragRef = useRef<RegionDrag | null>(null);
  const regionRef = useRef<HTMLDivElement>(null);

  const secondsPerPixel = () => {
    const width =
      regionRef.current?.parentElement?.getBoundingClientRect().width ?? 1;
    return duration / width;
  };

  const begin =
    (kind: RegionDrag["kind"]) => (event: PointerEvent<HTMLElement>) => {
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      dragRef.current = { end, kind, start, x: event.clientX };
    };

  const move = (event: PointerEvent<HTMLElement>) => {
    const drag = dragRef.current;
    if (!drag) {
      return;
    }
    event.stopPropagation();
    const delta = (event.clientX - drag.x) * secondsPerPixel();
    if (drag.kind === "move") {
      const length = drag.end - drag.start;
      const nextStart = clamp(drag.start + delta, 0, duration - length);
      onValueChange?.({ end: nextStart + length, start: nextStart });
    } else if (drag.kind === "start") {
      onValueChange?.({
        end: drag.end,
        start: clamp(drag.start + delta, 0, drag.end - minLength),
      });
    } else {
      onValueChange?.({
        end: clamp(drag.end + delta, drag.start + minLength, duration),
        start: drag.start,
      });
    }
  };

  const finish = (event: PointerEvent<HTMLElement>) => {
    event.stopPropagation();
    dragRef.current = null;
  };

  const nudge =
    (edge: "start" | "end") => (event: KeyboardEvent<HTMLSpanElement>) => {
      const amount = event.shiftKey ? 1 : 0.1;
      let delta = 0;
      if (event.key === "ArrowRight" || event.key === "ArrowUp") {
        delta = amount;
      } else if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
        delta = -amount;
      }
      if (delta === 0) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      if (edge === "start") {
        onValueChange?.({
          end,
          start: clamp(start + delta, 0, end - minLength),
        });
      } else {
        onValueChange?.({
          end: clamp(end + delta, start + minLength, duration),
          start,
        });
      }
    };

  const left = timeToPosition(start) * 100;
  const width = (timeToPosition(end) - timeToPosition(start)) * 100;
  const handleClass =
    "absolute inset-y-0 w-2 cursor-ew-resize rounded-sm bg-primary/60 after:absolute after:inset-y-0 after:-inset-x-1.5 pointer-coarse:after:-inset-x-3 outline-none focus-visible:bg-primary focus-visible:ring-3 focus-visible:ring-ring/30";

  return (
    <div
      className={cn(
        "bg-primary/15 ring-primary/40 absolute inset-y-0 left-(--region-start) w-(--region-size) rounded-sm ring-1",
        draggable && "cursor-grab active:cursor-grabbing",
        className
      )}
      data-slot="waveform-region"
      onLostPointerCapture={finish}
      onPointerDown={
        draggable ? begin("move") : (event) => event.stopPropagation()
      }
      onPointerMove={move}
      onPointerUp={finish}
      ref={regionRef}
      style={
        {
          "--region-size": `${width}%`,
          "--region-start": `${left}%`,
          ...style,
        } as CSSProperties
      }
      {...props}
    >
      {children}
      {resizable ? (
        <>
          <span
            aria-label="Region start"
            aria-valuemax={Math.round(end)}
            aria-valuemin={0}
            aria-valuenow={Math.round(start)}
            aria-valuetext={formatTime(start)}
            className={cn(handleClass, "-left-1")}
            data-slot="waveform-region-start"
            onKeyDown={nudge("start")}
            onPointerDown={begin("start")}
            role="slider"
            tabIndex={0}
          />
          <span
            aria-label="Region end"
            aria-valuemax={Math.round(duration)}
            aria-valuemin={Math.round(start)}
            aria-valuenow={Math.round(end)}
            aria-valuetext={formatTime(end)}
            className={cn(handleClass, "-right-1")}
            data-slot="waveform-region-end"
            onKeyDown={nudge("end")}
            onPointerDown={begin("end")}
            role="slider"
            tabIndex={0}
          />
        </>
      ) : null}
    </div>
  );
};

export interface WaveformMarkerProps extends ComponentProps<"div"> {
  time: number;
}

export const WaveformMarker = ({
  time,
  className,
  children,
  style,
  ...props
}: WaveformMarkerProps) => {
  const { timeToPosition } = useWaveform("WaveformMarker");
  return (
    <div
      className={cn(
        "bg-meter-warn pointer-events-none absolute inset-y-0 left-(--marker-position) w-px",
        className
      )}
      data-slot="waveform-marker"
      style={
        {
          "--marker-position": `${timeToPosition(time) * 100}%`,
          ...style,
        } as CSSProperties
      }
      {...props}
    >
      {children ? (
        <span className="bg-meter-warn/20 text-foreground absolute top-0 left-1 rounded-sm px-1 text-[0.625rem] font-medium whitespace-nowrap">
          {children}
        </span>
      ) : null}
    </div>
  );
};

export interface WaveformProps extends Omit<
  ComponentProps<"div">,
  "onSeeked" | "defaultValue"
> {
  /** Peaks, 0..1, from `useWaveformData`. */
  peaks: ArrayLike<number> | null;
  /** Length in seconds. */
  duration: number;
  /** Playhead position in seconds. */
  currentTime?: number;
  defaultCurrentTime?: number;
  /** A smooth playhead with no React renders. */
  time?: FrameSource<number> | null;
  /** Fires while seeking. */
  onSeek?: (time: number) => void;
  /** Fires when the pointer is released or after keyboard seeking. */
  onSeekCommitted?: (time: number) => void;
  /** Seconds per arrow key. Default 5. */
  step?: number;
  /** Seconds per Shift+arrow. Default 15. */
  largeStep?: number;
  /** Default `bars`. */
  variant?: "bars" | "line" | "mirror";
  /** Bar width in pixels. Default 2. */
  barWidth?: number;
  /** Gap in pixels. Default 1. */
  barGap?: number;
  /** Corner radius in pixels. Default 1. */
  barRadius?: number;
  /** When false, the waveform is display only. Default true. */
  interactive?: boolean;
  loading?: boolean;
  disabled?: boolean;
}

const isInteractive = (
  interactive: boolean,
  disabled: boolean,
  loading: boolean,
  duration: number
) => interactive && !disabled && !loading && duration > 0;

/** The hovered time, as a tiny external store. */
const createHoverChannel = () => {
  const listeners = new Set<() => void>();
  let hovered: number | null = null;
  return {
    get: () => hovered,
    set: (next: number | null) => {
      if (next === hovered) {
        return;
      }
      hovered = next;
      for (const listener of listeners) {
        listener();
      }
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
};

interface PointerOptions {
  setHover: (time: number | null) => void;
  seekTo: (time: number, commit: boolean) => void;
  timeAtPointer: (event: PointerEvent<HTMLDivElement>) => number;
}

/** Pointer seeking and hover tracking for the waveform root. */
const useWaveformPointer = ({
  setHover,
  seekTo,
  timeAtPointer,
}: PointerOptions) => {
  const [dragging, setDragging] = useState(false);
  return {
    dragging,
    handlers: {
      onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
        if (event.button !== 0) {
          return;
        }
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
        seekTo(timeAtPointer(event), false);
      },
      onPointerLeave: () => {
        setHover(null);
      },
      onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
        const at = timeAtPointer(event);
        setHover(at);
        if (dragging) {
          seekTo(at, false);
        }
      },
      onPointerUp: (event: PointerEvent<HTMLDivElement>) => {
        if (!dragging) {
          return;
        }
        setDragging(false);
        seekTo(timeAtPointer(event), true);
      },
    },
  };
};

export const Waveform = ({
  peaks,
  duration,
  currentTime,
  defaultCurrentTime = 0,
  time,
  onSeek,
  onSeekCommitted,
  step = 5,
  largeStep = 15,
  variant = "bars",
  barWidth = 2,
  barGap = 1,
  barRadius = 1,
  interactive = true,
  loading = false,
  disabled = false,
  className,
  children,
  ...props
}: WaveformProps) => {
  const [internalTime, setInternalTime] = useState(defaultCurrentTime);
  const shownTime = currentTime ?? internalTime;
  const rootRef = useRef<HTMLDivElement>(null);
  const progressRef = useRef(0);
  const hover = useMemo(() => createHoverChannel(), []);
  const latestTimeRef = useRef(shownTime);

  const timeToPosition = useCallback(
    (value: number) => (duration > 0 ? clamp(value / duration, 0, 1) : 0),
    [duration]
  );

  const writeProgress = useCallback(
    (value: number) => {
      latestTimeRef.current = value;
      progressRef.current = timeToPosition(value);
      rootRef.current?.style.setProperty(
        "--waveform-position",
        progressRef.current.toFixed(5)
      );
    },
    [timeToPosition]
  );

  useEffect(() => {
    writeProgress(shownTime);
  }, [shownTime, writeProgress]);

  useFrameSource(time, writeProgress);

  const seekTo = (value: number, commit: boolean) => {
    const next = clamp(value, 0, duration);
    writeProgress(next);
    if (currentTime === undefined) {
      setInternalTime(next);
    }
    onSeek?.(next);
    if (commit) {
      onSeekCommitted?.(next);
    }
  };

  const timeAtPointer = (event: PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const position = clamp((event.clientX - rect.left) / rect.width, 0, 1);
    return position * duration;
  };

  const active = isInteractive(interactive, disabled, loading, duration);

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!active) {
      return;
    }
    const next = seekTarget(
      event.key,
      latestTimeRef.current,
      event.shiftKey ? largeStep : step,
      duration
    );
    if (next !== null) {
      event.preventDefault();
      seekTo(next, true);
    }
  };

  const contextValue = useMemo<WaveformContextValue>(
    () => ({
      barGap,
      barRadius,
      barWidth,
      duration,
      getHover: hover.get,
      interactive: active,
      loading,
      peaks,
      progressRef,
      subscribeHover: hover.subscribe,
      timeToPosition,
      variant,
    }),
    [
      active,
      barGap,
      barRadius,
      barWidth,
      duration,
      hover,
      loading,
      peaks,
      timeToPosition,
      variant,
    ]
  );

  const pointer = useWaveformPointer({
    seekTo,
    setHover: hover.set,
    timeAtPointer,
  });

  // Leaving still clears the hover, so turning interactivity off mid-hover
  // (while a track loads) doesn't leave a stale line.
  const inactiveProps = { onPointerLeave: pointer.handlers.onPointerLeave };
  const interactiveProps = active
    ? {
        "aria-valuemax": Math.round(duration),
        "aria-valuemin": 0,
        "aria-valuenow": Math.round(shownTime),
        "aria-valuetext": `${formatTime(shownTime)} of ${formatTime(duration)}`,
        onKeyDown: handleKeyDown,
        ...pointer.handlers,
        role: "slider",
        tabIndex: 0,
      }
    : inactiveProps;

  return (
    <WaveformContext.Provider value={contextValue}>
      <div
        className={cn(
          "group/waveform focus-visible:ring-ring/30 relative h-20 w-full touch-none rounded-lg outline-none select-none [--waveform-cursor:var(--foreground)] [--waveform-position:0] [--waveform-progress:var(--primary)] [--waveform:var(--muted-foreground)] focus-visible:ring-3 data-disabled:opacity-50",
          className
        )}
        data-disabled={disabled ? "" : undefined}
        data-dragging={pointer.dragging ? "" : undefined}
        data-loading={loading ? "" : undefined}
        data-slot="waveform"
        data-variant={variant}
        ref={rootRef}
        {...interactiveProps}
        {...props}
      >
        {children ?? (
          <>
            <WaveformCanvas />
            <WaveformCursor />
          </>
        )}
      </div>
    </WaveformContext.Provider>
  );
};
