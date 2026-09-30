"use client";

import { cva } from "class-variance-authority";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type {
  ComponentProps,
  CSSProperties,
  KeyboardEvent,
  PointerEvent,
  RefObject,
} from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { clamp } from "@/lib/audio/decibels";
import { linearTaper, logTaper } from "@/lib/audio/taper";
import type { Taper } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const VIEWBOX = 100;
const CENTER = 50;
const RADIUS = 40;
const FINE_FACTOR = 0.1;
const PRECISION = 1e6;
const DEGREES_TO_RADIANS = Math.PI / 180;
const HALF_TURN = 180;

export type KnobChangeReason =
  | "drag"
  | "keyboard"
  | "wheel"
  | "reset"
  | "input";

export interface KnobChangeDetails {
  reason: KnobChangeReason;
  event?: Event;
}

interface KnobContextValue {
  value: number;
  position: number;
  originPosition: number;
  arc: number;
  labelId: string;
  disabled: boolean;
  format: (value: number) => string;
  parse: (text: string) => number | null;
  /** Characters in the widest value, for a steady value label. */
  valueWidth: number;
  /** A typed value is being entered in KnobValue. */
  editing: boolean;
  setEditing: (editing: boolean) => void;
}

const KnobContext = createContext<KnobContextValue | null>(null);

const useKnob = (part: string) => {
  const context = useContext(KnobContext);
  if (!context) {
    throw new Error(`${part} must be used inside Knob.`);
  }
  return context;
};

interface KnobDialContextValue {
  change: (value: number, details: KnobChangeDetails) => void;
  commit: (value: number) => void;
  dragDirection: "vertical" | "horizontal" | "circular";
  fineStep: number;
  largeStep: number;
  latestRef: RefObject<number>;
  max: number;
  min: number;
  quantize: (value: number, increment: number) => number;
  resetValue: number;
  sensitivity: number;
  step: number;
  taper: Taper;
  allowWheel: boolean;
}

const KnobDialContext = createContext<KnobDialContextValue | null>(null);

const useKnobDial = () => {
  const context = useContext(KnobDialContext);
  if (!context) {
    throw new Error("KnobDial must be used inside Knob.");
  }
  return context;
};

const roundValue = (value: number) => Math.round(value * PRECISION) / PRECISION;

/** Points along the range sampled to find the widest value text. */
const WIDTH_SAMPLES = 24;

/** Characters in the widest value along the range, so the value keeps one width. */
const widestValue = (
  format: (value: number) => string,
  taper: Taper,
  snap: (value: number) => number
) => {
  let widest = 0;
  for (let index = 0; index <= WIDTH_SAMPLES; index += 1) {
    const sample = snap(taper.toValue(index / WIDTH_SAMPLES));
    widest = Math.max(widest, format(sample).length);
  }
  return widest;
};

const NUMBER = /[-+]?(?:\d+\.?\d*|\.\d+)/u;
const THOUSANDS = /\d\s*k/iu;
const THOUSAND = 1000;

/** The first number in the text. "−" counts as a minus and "k" as thousands. */
export const parseKnobValue = (text: string): number | null => {
  const normalized = text.replaceAll("\u2212", "-");
  const match = NUMBER.exec(normalized);
  if (!match) {
    return null;
  }
  const number = Number(match[0]);
  return THOUSANDS.test(normalized) ? number * THOUSAND : number;
};

const angleFor = (position: number, arc: number) => -arc / 2 + position * arc;

const pointAt = (angle: number, radius: number) => {
  const radians = angle * DEGREES_TO_RADIANS;
  return {
    x: CENTER + radius * Math.sin(radians),
    y: CENTER - radius * Math.cos(radians),
  };
};

const arcPath = (fromAngle: number, toAngle: number, radius = RADIUS) => {
  const start = Math.min(fromAngle, toAngle);
  const end = Math.max(fromAngle, toAngle);
  if (end - start < 0.01) {
    return "";
  }
  const from = pointAt(start, radius);
  const to = pointAt(end, radius);
  const largeArc = end - start > HALF_TURN ? 1 : 0;
  return `M ${from.x} ${from.y} A ${radius} ${radius} 0 ${largeArc} 1 ${to.x} ${to.y}`;
};

/** A ref that always holds the latest value, for event handlers. */
const useLatest = <T,>(value: T): RefObject<T> => {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  });
  return ref;
};

const keyTarget = (
  key: string,
  current: number,
  increment: number,
  dial: KnobDialContextValue
): number | null => {
  const targets: Record<string, number> = {
    ArrowDown: current - increment,
    ArrowLeft: current - increment,
    ArrowRight: current + increment,
    ArrowUp: current + increment,
    End: dial.max,
    Home: dial.min,
    PageDown: current - dial.largeStep,
    PageUp: current + dial.largeStep,
  };
  return targets[key] ?? null;
};

const incrementFor = (
  event: { altKey: boolean; shiftKey: boolean },
  dial: KnobDialContextValue
) => {
  if (event.altKey) {
    return dial.fineStep;
  }
  return event.shiftKey ? dial.largeStep : dial.step;
};

/**
 * The next position, from the movement since the last pointer event, so
 * pressing or releasing Shift mid-drag changes the speed without a jump.
 */
const dragPosition = (
  event: PointerEvent<HTMLDivElement>,
  last: { x: number; y: number; position: number },
  dial: KnobDialContextValue,
  arc: number
) => {
  if (dial.dragDirection === "circular") {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX - (rect.left + rect.width / 2);
    const y = event.clientY - (rect.top + rect.height / 2);
    const angle = Math.atan2(x, -y) / DEGREES_TO_RADIANS;
    return clamp((angle + arc / 2) / arc, 0, 1);
  }
  const delta =
    dial.dragDirection === "vertical"
      ? last.y - event.clientY
      : event.clientX - last.x;
  const fine = event.shiftKey ? FINE_FACTOR : 1;
  return clamp(last.position + (delta / dial.sensitivity) * fine, 0, 1);
};

const useDialWheel = (
  elementRef: RefObject<HTMLDivElement | null>,
  dial: KnobDialContextValue
) => {
  const latest = useLatest(dial);

  useEffect(() => {
    const element = elementRef.current;
    if (!element) {
      return;
    }
    const onWheel = (event: WheelEvent) => {
      const { current } = latest;
      // Shift turns the wheel sideways on some systems.
      const delta = event.deltaY || event.deltaX;
      if (
        !current.allowWheel ||
        document.activeElement !== element ||
        delta === 0
      ) {
        return;
      }
      event.preventDefault();
      const fine = event.shiftKey || event.altKey;
      const increment = fine ? current.fineStep : current.step;
      const direction = delta < 0 ? 1 : -1;
      const next = current.quantize(
        current.latestRef.current + direction * increment,
        increment
      );
      current.change(next, { event, reason: "wheel" });
      current.commit(next);
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      element.removeEventListener("wheel", onWheel);
    };
  }, [elementRef, latest]);
};

export const KnobDial = ({
  className,
  children,
  style,
  ...props
}: ComponentProps<"div">) => {
  const { arc, disabled, format, labelId, position, setEditing, value } =
    useKnob("KnobDial");
  const dial = useKnobDial();
  const dialRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ x: number; y: number; position: number } | null>(
    null
  );
  const [dragging, setDragging] = useState(false);
  useDialWheel(dialRef, dial);

  const reset = () => {
    dial.change(dial.resetValue, { reason: "reset" });
    dial.commit(dial.resetValue);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) {
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      setEditing(true);
      return;
    }
    const increment = incrementFor(event, dial);
    const next = keyTarget(event.key, dial.latestRef.current, increment, dial);
    if (next === null) {
      return;
    }
    event.preventDefault();
    const quantized = dial.quantize(next, Math.min(increment, dial.step));
    dial.change(quantized, { event: event.nativeEvent, reason: "keyboard" });
    dial.commit(quantized);
  };

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (disabled || event.button !== 0) {
      return;
    }
    if (event.altKey) {
      event.preventDefault();
      reset();
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    event.currentTarget.focus();
    dragRef.current = {
      position: dial.taper.toPosition(dial.latestRef.current),
      x: event.clientX,
      y: event.clientY,
    };
    setDragging(true);
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const last = dragRef.current;
    if (!last) {
      return;
    }
    const next = dragPosition(event, last, dial, arc);
    dragRef.current = { position: next, x: event.clientX, y: event.clientY };
    const increment = event.shiftKey ? dial.fineStep : dial.step;
    dial.change(dial.quantize(dial.taper.toValue(next), increment), {
      event: event.nativeEvent,
      reason: "drag",
    });
  };

  const endDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragRef.current) {
      return;
    }
    dragRef.current = null;
    setDragging(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    dial.commit(dial.latestRef.current);
  };

  return (
    <div
      aria-disabled={disabled || undefined}
      aria-labelledby={labelId}
      aria-valuemax={dial.max}
      aria-valuemin={dial.min}
      aria-valuenow={value}
      aria-valuetext={format(value)}
      className={cn(
        "focus-visible:ring-ring/40 relative size-(--knob-size) touch-none rounded-full outline-none focus-visible:ring-3",
        dial.dragDirection === "horizontal"
          ? "cursor-ew-resize"
          : "cursor-ns-resize",
        className
      )}
      data-dragging={dragging ? "" : undefined}
      data-slot="knob-dial"
      onDoubleClick={() => {
        if (!disabled) {
          reset();
        }
      }}
      onKeyDown={handleKeyDown}
      onLostPointerCapture={endDrag}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endDrag}
      ref={dialRef}
      role="slider"
      style={
        {
          "--knob-angle": `${angleFor(position, arc)}deg`,
          ...style,
        } as CSSProperties
      }
      tabIndex={disabled ? -1 : 0}
      {...props}
    >
      <svg
        aria-hidden
        className="size-full overflow-visible"
        viewBox={`0 0 ${VIEWBOX} ${VIEWBOX}`}
      >
        {children}
      </svg>
    </div>
  );
};

export const KnobTrack = ({ className, ...props }: ComponentProps<"path">) => {
  const { arc } = useKnob("KnobTrack");
  return (
    <path
      className={cn("stroke-input", className)}
      d={arcPath(-arc / 2, arc / 2)}
      data-slot="knob-track"
      fill="none"
      strokeLinecap="round"
      strokeWidth={8}
      {...props}
    />
  );
};

export const KnobRange = ({ className, ...props }: ComponentProps<"path">) => {
  const { arc, originPosition, position } = useKnob("KnobRange");
  return (
    <path
      className={cn("stroke-primary", className)}
      d={arcPath(angleFor(originPosition, arc), angleFor(position, arc))}
      data-slot="knob-range"
      fill="none"
      strokeLinecap="round"
      strokeWidth={8}
      {...props}
    />
  );
};

export const KnobPointer = ({
  className,
  ...props
}: ComponentProps<"line">) => {
  const { arc, position } = useKnob("KnobPointer");
  const angle = angleFor(position, arc);
  const inner = pointAt(angle, RADIUS * 0.3);
  const outer = pointAt(angle, RADIUS * 0.72);
  return (
    <>
      <circle
        className="fill-muted stroke-border"
        cx={CENTER}
        cy={CENTER}
        r={RADIUS * 0.8}
        strokeWidth={1}
      />
      <line
        className={cn("stroke-foreground", className)}
        data-slot="knob-pointer"
        strokeLinecap="round"
        strokeWidth={6}
        x1={inner.x}
        x2={outer.x}
        y1={inner.y}
        y2={outer.y}
        {...props}
      />
    </>
  );
};

const focusDial = (from: HTMLElement) => {
  from
    .closest("[data-slot='knob']")
    ?.querySelector<HTMLElement>("[data-slot='knob-dial']")
    ?.focus();
};

/** The inline editor KnobValue shows while a value is typed. */
const KnobValueInput = ({
  className,
  style,
}: Pick<ComponentProps<"input">, "className" | "style">) => {
  const { format, parse, setEditing, value } = useKnob("KnobValue");
  const dial = useKnobDial();
  const [draft, setDraft] = useState(() => format(value));
  const doneRef = useRef(false);
  const focusInput = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
    node?.select();
  }, []);

  const finish = (apply: boolean) => {
    if (doneRef.current) {
      return;
    }
    doneRef.current = true;
    setEditing(false);
    const parsed = apply ? parse(draft) : null;
    if (parsed === null || Number.isNaN(parsed)) {
      return;
    }
    const next = dial.quantize(
      clamp(parsed, dial.min, dial.max),
      Math.min(dial.fineStep, dial.step)
    );
    dial.change(next, { reason: "input" });
    dial.commit(next);
  };

  return (
    <input
      aria-label="Value"
      className={cn(
        "bg-background ring-ring/50 h-4 w-(--knob-value-width) min-w-0 rounded-sm p-0 text-center font-mono text-xs tabular-nums ring-1 outline-none",
        className
      )}
      data-slot="knob-value-input"
      onBlur={() => finish(true)}
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== "Escape") {
          return;
        }
        event.preventDefault();
        const input = event.currentTarget;
        finish(event.key === "Enter");
        focusDial(input);
      }}
      ref={focusInput}
      style={style}
      value={draft}
    />
  );
};

export interface KnobValueProps extends ComponentProps<"span"> {
  /** Double-click the value, or press Enter on the dial, to type one. Default true. */
  editable?: boolean;
}

export const KnobValue = ({
  editable = true,
  className,
  style,
  onDoubleClick,
  ...props
}: KnobValueProps) => {
  const { disabled, editing, format, setEditing, value, valueWidth } =
    useKnob("KnobValue");
  const widthStyle = {
    "--knob-value-width": `${valueWidth}ch`,
    ...style,
  } as CSSProperties;

  if (editable && editing) {
    return <KnobValueInput className={className} style={widthStyle} />;
  }

  return (
    <span
      className={cn(
        "text-muted-foreground inline-block min-w-(--knob-value-width) text-center font-mono text-xs whitespace-nowrap tabular-nums",
        editable && !disabled && "cursor-text",
        className
      )}
      data-slot="knob-value"
      onDoubleClick={(event) => {
        onDoubleClick?.(event);
        if (editable && !disabled) {
          setEditing(true);
        }
      }}
      style={widthStyle}
      {...props}
    >
      {format(value)}
    </span>
  );
};

export const KnobLabel = ({
  className,
  onDoubleClick,
  ...props
}: ComponentProps<"span">) => {
  const { disabled, labelId, setEditing } = useKnob("KnobLabel");
  return (
    <span
      className={cn("text-xs font-medium", className)}
      data-slot="knob-label"
      id={labelId}
      onDoubleClick={(event) => {
        onDoubleClick?.(event);
        if (!disabled) {
          setEditing(true);
        }
      }}
      {...props}
    />
  );
};

const knobVariants = cva(
  "group/knob inline-flex flex-col items-center gap-1.5 select-none data-disabled:opacity-50",
  {
    defaultVariants: { size: "default" },
    variants: {
      size: {
        default: "[--knob-size:3rem]",
        lg: "[--knob-size:4rem]",
        sm: "[--knob-size:2.25rem]",
      },
    },
  }
);

export interface KnobProps extends Omit<
  ComponentProps<"div">,
  "defaultValue" | "onChange"
> {
  value?: number;
  defaultValue?: number;
  onValueChange?: (value: number, details: KnobChangeDetails) => void;
  onValueCommitted?: (value: number) => void;
  /** Default 0. */
  min?: number;
  /** Default 100. */
  max?: number;
  /** Default 1. */
  step?: number;
  /** Default 10. */
  largeStep?: number;
  /** Shift+drag, Shift+wheel and Alt+arrow. Default: `step / 10`. */
  fineStep?: number;
  /** Double-click or Alt+click restores this. Default `defaultValue` or `min`. */
  resetValue?: number;
  /** Where the arc starts; the centre for bipolar knobs. Default `min`. */
  origin?: number;
  /** Sweep in degrees. Default 270. */
  arc?: number;
  /** How dragging turns the knob. Default `vertical`. */
  dragDirection?: "vertical" | "horizontal" | "circular";
  /** Pixels of drag for the full range. Default 200. */
  sensitivity?: number;
  /** Default `linear`. */
  scale?: "linear" | "log";
  /** The wheel adjusts the value while focused. Default false. */
  allowWheel?: boolean;
  format?: (value: number) => string;
  /** Reads a typed value. Default: the first number, with "k" as thousands. */
  parse?: (text: string) => number | null;
  size?: AudioSize;
  disabled?: boolean;
}

interface KnobValueOptions {
  value: number | undefined;
  defaultValue: number | undefined;
  resetValue: number;
  min: number;
  max: number;
  onValueChange: KnobProps["onValueChange"];
}

/** Controlled or uncontrolled value with change notifications. */
const useKnobValue = ({
  value: valueProp,
  defaultValue,
  resetValue,
  min,
  max,
  onValueChange,
}: KnobValueOptions) => {
  const [uncontrolled, setUncontrolled] = useState(
    clamp(defaultValue ?? resetValue, min, max)
  );
  const value = valueProp ?? uncontrolled;
  const latestRef = useLatest(value);
  const controlled = valueProp !== undefined;

  const change = useCallback(
    (next: number, details: KnobChangeDetails) => {
      if (next === latestRef.current) {
        return;
      }
      latestRef.current = next;
      if (!controlled) {
        setUncontrolled(next);
      }
      onValueChange?.(next, details);
    },
    [controlled, latestRef, onValueChange]
  );

  return { change, latestRef, value };
};

/** Size and disabled, from props or the surrounding strip. */
const useKnobSettings = (
  size: AudioSize | undefined,
  disabled: boolean | undefined
) => {
  const config = useAudioConfig();
  return {
    disabled: disabled ?? config.disabled ?? false,
    size: size ?? config.size ?? "default",
  };
};

export const Knob = ({
  value: valueProp,
  defaultValue,
  onValueChange,
  onValueCommitted,
  min = 0,
  max = 100,
  step = 1,
  largeStep = 10,
  fineStep,
  resetValue,
  origin,
  arc = 270,
  dragDirection = "vertical",
  sensitivity = 200,
  scale = "linear",
  allowWheel = false,
  format = String,
  parse = parseKnobValue,
  size: sizeProp,
  disabled: disabledProp,
  className,
  children,
  ...props
}: KnobProps) => {
  const { disabled, size } = useKnobSettings(sizeProp, disabledProp);
  const reset = resetValue ?? defaultValue ?? min;
  const labelId = useId();
  const { change, latestRef, value } = useKnobValue({
    defaultValue,
    max,
    min,
    onValueChange,
    resetValue: reset,
    value: valueProp,
  });

  const taper: Taper = useMemo(
    () => (scale === "log" ? logTaper(min, max) : linearTaper(min, max)),
    [max, min, scale]
  );

  const quantize = useCallback(
    (next: number, increment: number) =>
      roundValue(
        clamp(min + Math.round((next - min) / increment) * increment, min, max)
      ),
    [max, min]
  );

  const position = taper.toPosition(value);
  const originValue = clamp(origin ?? min, min, max);
  const originPosition = taper.toPosition(originValue);

  const [editing, setEditing] = useState(false);

  const valueWidth = useMemo(
    () => widestValue(format, taper, (next) => quantize(next, step)),
    [format, quantize, step, taper]
  );

  const contextValue = useMemo<KnobContextValue>(
    () => ({
      arc,
      disabled,
      editing,
      format,
      labelId,
      originPosition,
      parse,
      position,
      setEditing,
      value,
      valueWidth,
    }),
    [
      arc,
      disabled,
      editing,
      format,
      labelId,
      originPosition,
      parse,
      position,
      value,
      valueWidth,
    ]
  );

  const dialContext = useMemo<KnobDialContextValue>(
    () => ({
      allowWheel,
      change,
      commit: (next: number) => onValueCommitted?.(next),
      dragDirection,
      fineStep: fineStep ?? step * FINE_FACTOR,
      largeStep,
      latestRef,
      max,
      min,
      quantize,
      resetValue: reset,
      sensitivity,
      step,
      taper,
    }),
    [
      allowWheel,
      change,
      dragDirection,
      fineStep,
      largeStep,
      latestRef,
      max,
      min,
      onValueCommitted,
      quantize,
      reset,
      sensitivity,
      step,
      taper,
    ]
  );

  return (
    <KnobContext.Provider value={contextValue}>
      <KnobDialContext.Provider value={dialContext}>
        <div
          className={cn(knobVariants({ size }), className)}
          data-at-origin={value === originValue ? "" : undefined}
          data-disabled={disabled ? "" : undefined}
          data-size={size}
          data-slot="knob"
          {...props}
        >
          {children ?? (
            <KnobDial>
              <KnobTrack />
              <KnobRange />
              <KnobPointer />
            </KnobDial>
          )}
        </div>
      </KnobDialContext.Provider>
    </KnobContext.Provider>
  );
};

export { knobVariants };
