"use client";

import { mergeProps } from "@base-ui/react/merge-props";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { useRender } from "@base-ui/react/use-render";
import { cva } from "class-variance-authority";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ComponentProps, CSSProperties, KeyboardEvent } from "react";

import { DbScale } from "@/components/ui/db-scale";
import type { DbScaleProps } from "@/components/ui/db-scale";
import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { clamp, formatDb, SILENCE_DB } from "@/lib/audio/decibels";
import { resolveTaper } from "@/lib/audio/taper";
import type { TaperInput } from "@/lib/audio/taper";
import type { Orientation, Taper } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const DEFAULT_MIN_DB = -60;
const DEFAULT_MAX_DB = 6;
const POSITION_STEP = 0.0005;
const DETENT_SNAP = 0.012;
const PRECISION = 1e6;

export type FaderChangeReason =
  | "drag"
  | "track-press"
  | "keyboard"
  | "wheel"
  | "reset"
  | "input";

export interface FaderChangeDetails {
  reason: FaderChangeReason;
  event?: Event;
}

interface FaderContextValue {
  value: number;
  position: number;
  originPosition: number;
  min: number;
  max: number;
  resetValue: number;
  orientation: Orientation;
  variant: "default" | "console";
  size: AudioSize;
  disabled: boolean;
  taper: Taper;
  format: (db: number) => string;
  change: (db: number, details: FaderChangeDetails) => void;
  commit: (db: number) => void;
  handleKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
}

const FaderContext = createContext<FaderContextValue | null>(null);

const useFader = (part: string) => {
  const context = useContext(FaderContext);
  if (!context) {
    throw new Error(`${part} must be used inside Fader.`);
  }
  return context;
};

const defaultFormat = (db: number) =>
  db === SILENCE_DB ? "Silent" : formatDb(db);

const roundValue = (value: number) => Math.round(value * PRECISION) / PRECISION;

const faderVariants = cva(
  "group/fader relative flex touch-none select-none gap-2 data-disabled:opacity-50",
  {
    defaultVariants: {
      orientation: "horizontal",
      size: "default",
    },
    variants: {
      orientation: {
        horizontal: "w-full flex-col",
        vertical: "h-full min-h-32 flex-row justify-center",
      },
      size: {
        default: "[--fader-thumb-size:1rem] [--fader-track-size:0.25rem]",
        lg: "[--fader-thumb-size:1.25rem] [--fader-track-size:0.375rem]",
        sm: "[--fader-thumb-size:0.75rem] [--fader-track-size:0.1875rem]",
      },
    },
  }
);

type SliderRootProps = SliderPrimitive.Root.Props<number>;

export interface FaderProps
  extends Omit<
    SliderRootProps,
    | "value"
    | "defaultValue"
    | "onValueChange"
    | "onValueCommitted"
    | "min"
    | "max"
    | "step"
    | "largeStep"
    | "format"
    | "orientation"
  > {
  /** The value in dB. */
  value?: number;
  defaultValue?: number;
  onValueChange?: (value: number, details: FaderChangeDetails) => void;
  /** Fires when a drag ends, after keyboard input, and on reset. */
  onValueCommitted?: (value: number) => void;
  /** Bottom of the range in dB. Default −60. */
  min?: number;
  /** Top of the range in dB. Default +6. */
  max?: number;
  /** Arrow keys and drag resolution in dB. Default 0.5. */
  step?: number;
  /** Shift+arrow and Page Up/Down, in dB. Default 6. */
  largeStep?: number;
  /** Alt+arrow and Alt+drag resolution, in dB. Default 0.1. */
  fineStep?: number;
  /** Value restored by double-clicking the thumb. Default 0. */
  resetValue?: number;
  /** Position law. Default `linear`. */
  taper?: TaperInput;
  /** Where the range fill starts. Set 0 for a bipolar gain. Default `min`. */
  origin?: number;
  /** Values the thumb snaps to while dragging. Default `[0]`. */
  detents?: number[];
  /** The bottom position reports `-Infinity`. Default false. */
  silenceAtMin?: boolean;
  /** The mouse wheel adjusts the value while the fader is focused. Default false. */
  allowWheel?: boolean;
  orientation?: Orientation;
  /** `console` has a wide cap thumb. Default `default`. */
  variant?: "default" | "console";
  size?: AudioSize;
  /** Formats the value for `FaderValue` and assistive technology. */
  format?: (db: number) => string;
}

export const Fader = ({
  value: valueProp,
  defaultValue,
  onValueChange,
  onValueCommitted,
  min = DEFAULT_MIN_DB,
  max = DEFAULT_MAX_DB,
  step = 0.5,
  largeStep = 6,
  fineStep = 0.1,
  resetValue = 0,
  taper = "linear",
  origin,
  detents,
  silenceAtMin = false,
  allowWheel = false,
  orientation: orientationProp,
  variant = "default",
  size: sizeProp,
  format = defaultFormat,
  disabled: disabledProp,
  className,
  children,
  ref,
  ...props
}: FaderProps) => {
  const config = useAudioConfig();
  const orientation = orientationProp ?? config.orientation ?? "horizontal";
  const size = sizeProp ?? config.size ?? "default";
  const disabled = disabledProp ?? config.disabled ?? false;
  const [uncontrolled, setUncontrolled] = useState(
    defaultValue ?? clamp(resetValue, min, max)
  );
  const value = valueProp ?? uncontrolled;
  const latestRef = useRef(value);
  latestRef.current = value;

  const taperFn = useMemo(() => resolveTaper(taper, min, max), [taper, min, max]);
  const snapPoints = useMemo(() => detents ?? [0], [detents]);

  const toPosition = useCallback(
    (db: number) => (db === SILENCE_DB ? 0 : taperFn.toPosition(db)),
    [taperFn]
  );

  const quantize = useCallback(
    (db: number, increment: number) => {
      if (db === SILENCE_DB) {
        return silenceAtMin ? SILENCE_DB : min;
      }
      const stepped = min + Math.round((db - min) / increment) * increment;
      return roundValue(clamp(stepped, min, max));
    },
    [max, min, silenceAtMin]
  );

  const change = useCallback(
    (db: number, details: FaderChangeDetails) => {
      if (db === latestRef.current) {
        return;
      }
      latestRef.current = db;
      if (valueProp === undefined) {
        setUncontrolled(db);
      }
      onValueChange?.(db, details);
    },
    [onValueChange, valueProp]
  );

  const commit = useCallback(
    (db: number) => {
      onValueCommitted?.(db);
    },
    [onValueCommitted]
  );

  const fromPosition = useCallback(
    (position: number, fine: boolean) => {
      if (silenceAtMin && position <= 0) {
        return SILENCE_DB;
      }
      for (const detent of snapPoints) {
        if (Math.abs(position - toPosition(detent)) < DETENT_SNAP) {
          return detent;
        }
      }
      return quantize(taperFn.toValue(position), fine ? fineStep : step);
    },
    [fineStep, quantize, silenceAtMin, snapPoints, step, taperFn, toPosition]
  );

  const nudge = useCallback(
    (direction: number, increment: number) => {
      const current = latestRef.current;
      if (current === SILENCE_DB) {
        return direction > 0 ? min : SILENCE_DB;
      }
      const next = quantize(current + direction * increment, Math.min(increment, step));
      if (silenceAtMin && direction < 0 && current <= min) {
        return SILENCE_DB;
      }
      return next;
    },
    [min, quantize, silenceAtMin, step]
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      let increment = step;
      if (event.altKey) {
        increment = fineStep;
      } else if (event.shiftKey) {
        increment = largeStep;
      }
      let next: number | null = null;
      switch (event.key) {
        case "ArrowUp":
        case "ArrowRight": {
          next = nudge(1, increment);
          break;
        }
        case "ArrowDown":
        case "ArrowLeft": {
          next = nudge(-1, increment);
          break;
        }
        case "PageUp": {
          next = nudge(1, largeStep);
          break;
        }
        case "PageDown": {
          next = nudge(-1, largeStep);
          break;
        }
        case "Home": {
          next = silenceAtMin ? SILENCE_DB : min;
          break;
        }
        case "End": {
          next = max;
          break;
        }
        default: {
          break;
        }
      }
      if (next === null) {
        return;
      }
      event.preventDefault();
      change(next, { event: event.nativeEvent, reason: "keyboard" });
      commit(next);
    },
    [change, commit, fineStep, largeStep, max, min, nudge, silenceAtMin, step]
  );

  const rootRef = useRef<HTMLDivElement | null>(null);
  const wheelRef = useRef({ change, commit, disabled, fineStep, nudge, step });
  wheelRef.current = { change, commit, disabled, fineStep, nudge, step };

  useEffect(() => {
    const root = rootRef.current;
    if (!(root && allowWheel)) {
      return;
    }
    const onWheel = (event: globalThis.WheelEvent) => {
      const current = wheelRef.current;
      const focused = root.contains(document.activeElement);
      if (current.disabled || !focused || event.deltaY === 0) {
        return;
      }
      event.preventDefault();
      const next = current.nudge(
        event.deltaY < 0 ? 1 : -1,
        event.altKey ? current.fineStep : current.step
      );
      current.change(next, { event, reason: "wheel" });
      current.commit(next);
    };
    root.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      root.removeEventListener("wheel", onWheel);
    };
  }, [allowWheel]);

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

  const position = toPosition(value);
  const originPosition = toPosition(clamp(origin ?? min, min, max));
  const atDetent = snapPoints.includes(value);

  const contextValue = useMemo<FaderContextValue>(
    () => ({
      change,
      commit,
      disabled,
      format,
      handleKeyDown,
      max,
      min,
      orientation,
      originPosition,
      position,
      resetValue,
      size,
      taper: taperFn,
      value,
      variant,
    }),
    [change, commit, disabled, format, handleKeyDown, max, min, orientation, originPosition, position, resetValue, size, taperFn, value, variant]
  );

  return (
    <FaderContext.Provider value={contextValue}>
      <SliderPrimitive.Root
        className={cn(faderVariants({ orientation, size }), className)}
        data-at-detent={atDetent ? "" : undefined}
        data-silent={value === SILENCE_DB ? "" : undefined}
        data-size={size}
        data-slot="fader"
        data-variant={variant}
        disabled={disabled}
        max={1}
        min={0}
        onValueChange={(next, details) => {
          const fine =
            "altKey" in details.event &&
            Boolean((details.event as MouseEvent).altKey);
          const reason: FaderChangeReason =
            details.reason === "track-press" ? "track-press" : "drag";
          change(fromPosition(next, fine), { event: details.event, reason });
        }}
        onValueCommitted={() => {
          commit(latestRef.current);
        }}
        orientation={orientation}
        ref={setRootRef}
        step={POSITION_STEP}
        value={position}
        {...props}
      >
        {children ?? (
          <FaderTrack>
            <FaderRange />
            <FaderThumb />
          </FaderTrack>
        )}
      </SliderPrimitive.Root>
    </FaderContext.Provider>
  );
};

export const FaderLabel = ({
  className,
  ...props
}: SliderPrimitive.Label.Props) => (
  <SliderPrimitive.Label
    className={cn("font-medium text-sm", className)}
    data-slot="fader-label"
    {...props}
  />
);

export const FaderTrack = ({
  className,
  children,
  ...props
}: SliderPrimitive.Control.Props) => {
  const { orientation } = useFader("FaderTrack");
  const horizontal = orientation === "horizontal";

  return (
    <SliderPrimitive.Control
      className={cn(
        "relative flex min-h-0 min-w-0 items-center",
        horizontal
          ? "h-(--fader-thumb-size) w-full px-[calc(var(--fader-thumb-size)/2)]"
          : "h-full w-(--fader-thumb-size) flex-col py-[calc(var(--fader-thumb-size)/2)]"
      )}
      data-slot="fader-control"
      {...props}
    >
      <SliderPrimitive.Track
        className={cn(
          "relative grow rounded-full bg-input/90",
          horizontal
            ? "h-(--fader-track-size) w-full"
            : "h-full w-(--fader-track-size)",
          className
        )}
        data-slot="fader-track"
      >
        {children}
      </SliderPrimitive.Track>
    </SliderPrimitive.Control>
  );
};

export const FaderRange = ({
  className,
  style,
  ...props
}: ComponentProps<"div">) => {
  const { orientation, originPosition, position } = useFader("FaderRange");
  const start = Math.min(originPosition, position) * 100;
  const end = Math.max(originPosition, position) * 100;
  const placement: CSSProperties =
    orientation === "horizontal"
      ? { left: `${start}%`, width: `${end - start}%` }
      : { bottom: `${start}%`, height: `${end - start}%` };

  return (
    <div
      className={cn(
        "absolute rounded-full bg-primary",
        orientation === "horizontal" ? "inset-y-0" : "inset-x-0",
        className
      )}
      data-slot="fader-range"
      style={{ ...placement, ...style }}
      {...props}
    />
  );
};

const thumbVariants = cva(
  "block shrink-0 bg-background shadow-sm outline-hidden ring-1 ring-foreground/15 transition-[box-shadow] hover:ring-4 hover:ring-ring/30 focus-visible:ring-4 focus-visible:ring-ring/40 data-dragging:ring-4 data-dragging:ring-ring/30 data-disabled:pointer-events-none",
  {
    defaultVariants: {
      orientation: "horizontal",
      variant: "default",
    },
    variants: {
      orientation: {
        horizontal: "",
        vertical: "",
      },
      variant: {
        console: "rounded-sm border border-border",
        default: "size-(--fader-thumb-size) rounded-full",
      },
    },
    compoundVariants: [
      {
        className:
          "h-[calc(var(--fader-thumb-size)*1.6)] w-[calc(var(--fader-thumb-size)*0.7)] bg-[linear-gradient(to_right,transparent_calc(50%-0.5px),var(--foreground)_calc(50%-0.5px),var(--foreground)_calc(50%+0.5px),transparent_calc(50%+0.5px))]",
        orientation: "horizontal",
        variant: "console",
      },
      {
        className:
          "h-[calc(var(--fader-thumb-size)*0.7)] w-[calc(var(--fader-thumb-size)*1.8)] bg-[linear-gradient(to_bottom,transparent_calc(50%-0.5px),var(--foreground)_calc(50%-0.5px),var(--foreground)_calc(50%+0.5px),transparent_calc(50%+0.5px))]",
        orientation: "vertical",
        variant: "console",
      },
    ],
  }
);

export type FaderThumbProps = Omit<
  SliderPrimitive.Thumb.Props,
  "getAriaValueText" | "onKeyDown"
>;

export const FaderThumb = ({
  className,
  onDoubleClick,
  ...props
}: FaderThumbProps) => {
  const {
    change,
    commit,
    format,
    handleKeyDown,
    orientation,
    resetValue,
    value,
    variant,
  } = useFader("FaderThumb");

  return (
    <SliderPrimitive.Thumb
      className={cn(thumbVariants({ orientation, variant }), className)}
      data-slot="fader-thumb"
      getAriaValueText={() => format(value)}
      onDoubleClick={(event) => {
        onDoubleClick?.(event);
        change(resetValue, { event: event.nativeEvent, reason: "reset" });
        commit(resetValue);
      }}
      onKeyDown={handleKeyDown}
      {...props}
    />
  );
};

export type FaderScaleProps = Omit<
  DbScaleProps,
  "minDb" | "maxDb" | "taper" | "orientation"
>;

export const FaderScale = ({ className, ...props }: FaderScaleProps) => {
  const { max, min, orientation, taper } = useFader("FaderScale");
  const horizontal = orientation === "horizontal";

  return (
    <div
      className={cn(
        horizontal
          ? "px-[calc(var(--fader-thumb-size)/2)]"
          : "py-[calc(var(--fader-thumb-size)/2)]",
        className
      )}
      data-slot="fader-scale"
    >
      <DbScale
        maxDb={max}
        minDb={min}
        orientation={orientation}
        taper={taper}
        {...props}
      />
    </div>
  );
};

const parseDb = (text: string): number | null => {
  const normalized = text.replaceAll("−", "-").replace(/db/i, "").trim();
  if (/^-?(inf|infinity|∞)$/i.test(normalized)) {
    return SILENCE_DB;
  }
  const parsed = Number.parseFloat(normalized);
  return Number.isNaN(parsed) ? null : parsed;
};

export interface FaderValueProps extends ComponentProps<"span"> {
  /** Click to type a value. Default false. */
  editable?: boolean;
}

export const FaderValue = ({
  editable = false,
  className,
  ...props
}: FaderValueProps) => {
  const { change, commit, disabled, format, max, min, value } =
    useFader("FaderValue");
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const focusInput = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
    node?.select();
  }, []);

  const finish = (apply: boolean) => {
    setEditing(false);
    if (!apply) {
      return;
    }
    const parsed = parseDb(draft);
    if (parsed === null) {
      return;
    }
    const next = parsed === SILENCE_DB ? min : clamp(parsed, min, max);
    change(next, { reason: "input" });
    commit(next);
  };

  if (editing) {
    return (
      <input
        aria-label="Value in dB"
        className={cn(
          "h-6 w-20 rounded-md border bg-background px-1.5 text-end font-mono text-xs tabular-nums outline-none focus-visible:ring-3 focus-visible:ring-ring/30",
          className
        )}
        data-slot="fader-value-input"
        onBlur={() => finish(true)}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            finish(true);
          } else if (event.key === "Escape") {
            finish(false);
          }
        }}
        ref={focusInput}
        value={draft}
      />
    );
  }

  const text = format(value);

  if (editable) {
    return (
      <button
        className={cn(
          "h-6 rounded-md px-1.5 text-end font-mono text-muted-foreground text-xs tabular-nums outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30",
          className
        )}
        data-slot="fader-value"
        disabled={disabled}
        onClick={() => {
          setDraft(value === SILENCE_DB ? "-inf" : String(value));
          setEditing(true);
        }}
        type="button"
      >
        {text}
      </button>
    );
  }

  return (
    <span
      className={cn(
        "font-mono text-muted-foreground text-xs tabular-nums",
        className
      )}
      data-slot="fader-value"
      {...props}
    >
      {text}
    </span>
  );
};

export type FaderResetProps = useRender.ComponentProps<"button">;

export const FaderReset = ({
  render,
  className,
  children,
  ...props
}: FaderResetProps) => {
  const { change, commit, disabled, resetValue, value } = useFader("FaderReset");
  const modified = value !== resetValue;

  return useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      {
        "aria-label": "Reset",
        children: children ?? "Reset",
        className: cn(
          "inline-flex h-6 items-center rounded-md px-1.5 text-muted-foreground text-xs outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30 disabled:pointer-events-none disabled:opacity-0",
          className
        ),
        disabled: disabled || !modified,
        onClick: () => {
          change(resetValue, { reason: "reset" });
          commit(resetValue);
        },
        type: "button",
      },
      props
    ),
    render,
    state: {
      modified,
      slot: "fader-reset",
    },
  });
};

export { faderVariants };
