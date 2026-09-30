"use client";

import { createContext, useContext, useMemo } from "react";
import type { ComponentProps, CSSProperties } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import { DEFAULT_MAX_DB, DEFAULT_MIN_DB, formatDb } from "@/lib/audio/decibels";
import { resolveTaper } from "@/lib/audio/taper";
import type { TaperInput } from "@/lib/audio/taper";
import type { Orientation, Taper } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const DEFAULT_TICKS = [12, 6, 0, -6, -12, -18, -24, -36, -48, -60, -72, -90];
const EDGE = 0.02;

const defaultFormat = (db: number) =>
  formatDb(db, { decimals: 0, unit: false });

interface DbScaleContextValue {
  orientation: Orientation;
  side: "start" | "end";
  labels: boolean;
  taper: Taper;
  format: (db: number) => string;
}

const DbScaleContext = createContext<DbScaleContextValue | null>(null);

const useDbScale = () => {
  const context = useContext(DbScaleContext);
  if (!context) {
    throw new Error("DbScaleTick must be used inside DbScale.");
  }
  return context;
};

export interface DbScaleProps extends ComponentProps<"div"> {
  /** Bottom of the range. Default −60, or the surrounding meter's range. */
  minDb?: number;
  /** Top of the range. Default 0, or the surrounding meter's range. */
  maxDb?: number;
  /** Tick values. Default: common values inside the range. */
  ticks?: number[];
  /** Position law, so ticks line up with a fader or meter using the same taper. */
  taper?: TaperInput;
  orientation?: Orientation;
  /** Which side of the tick marks the labels sit on. Default `end`. */
  side?: "start" | "end";
  /** Show labels. Default true. */
  labels?: boolean;
  format?: (db: number) => string;
}

export const DbScale = ({
  minDb: minDbProp,
  maxDb: maxDbProp,
  ticks,
  taper = "linear",
  orientation: orientationProp,
  side = "end",
  labels = true,
  format = defaultFormat,
  className,
  children,
  ...props
}: DbScaleProps) => {
  const config = useAudioConfig();
  const minDb = minDbProp ?? config.minDb ?? DEFAULT_MIN_DB;
  const maxDb = maxDbProp ?? config.maxDb ?? DEFAULT_MAX_DB;
  const orientation = orientationProp ?? config.orientation ?? "horizontal";

  const value = useMemo<DbScaleContextValue>(
    () => ({
      format,
      labels,
      orientation,
      side,
      taper: resolveTaper(taper, minDb, maxDb),
    }),
    [format, labels, maxDb, minDb, orientation, side, taper]
  );

  const values =
    ticks ?? DEFAULT_TICKS.filter((tick) => tick >= minDb && tick <= maxDb);

  return (
    <DbScaleContext.Provider value={value}>
      <div
        aria-hidden
        className={cn(
          "text-muted-foreground relative shrink-0 text-[0.625rem] leading-none tabular-nums select-none",
          orientation === "horizontal" ? "h-4 w-full" : "h-full w-7",
          className
        )}
        data-orientation={orientation}
        data-side={side}
        data-slot="db-scale"
        {...props}
      >
        {children ??
          values.map((tick) => <DbScaleTick key={tick} value={tick} />)}
      </div>
    </DbScaleContext.Provider>
  );
};

export interface DbScaleTickProps extends ComponentProps<"div"> {
  value: number;
  /** Major ticks are longer. Default true. */
  major?: boolean;
}

const labelTransform = (orientation: Orientation, position: number) => {
  if (orientation === "vertical") {
    if (position < EDGE) {
      return "translateY(0)";
    }
    if (position > 1 - EDGE) {
      return "translateY(100%)";
    }
    return "translateY(50%)";
  }
  if (position < EDGE) {
    return "translateX(0)";
  }
  if (position > 1 - EDGE) {
    return "translateX(-100%)";
  }
  return "translateX(-50%)";
};

export const DbScaleTick = ({
  value,
  major = true,
  className,
  style,
  children,
  ...props
}: DbScaleTickProps) => {
  const { format, labels, orientation, side, taper } = useDbScale();
  const position = taper.toPosition(value);
  const horizontal = orientation === "horizontal";
  const placement: CSSProperties = horizontal
    ? { left: `${position * 100}%` }
    : { bottom: `${position * 100}%` };

  return (
    <div
      className={cn(
        "absolute flex items-center gap-0.5",
        horizontal ? "top-0 h-full flex-col" : "left-0 w-full flex-row",
        side === "start" &&
          (horizontal ? "flex-col-reverse" : "flex-row-reverse"),
        className
      )}
      data-major={major ? "" : undefined}
      data-slot="db-scale-tick"
      style={{
        ...placement,
        transform: labelTransform(orientation, position),
        ...style,
      }}
      {...props}
    >
      <span
        className={cn(
          "bg-border shrink-0",
          horizontal ? "w-px" : "h-px",
          horizontal && (major ? "h-1.5" : "h-1"),
          !horizontal && (major ? "w-1.5" : "w-1")
        )}
        data-slot="db-scale-mark"
      />
      {labels ? (
        <span data-slot="db-scale-label">{children ?? format(value)}</span>
      ) : null}
    </div>
  );
};
