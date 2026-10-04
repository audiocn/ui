"use client";

import { cva } from "class-variance-authority";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
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
import { getSharedAudioContext } from "@/hooks/use-audio-context";
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
  /** KnobScale reports its long ticks, so the click sound lands on them. */
  setDetents: (positions: readonly number[] | null) => void;
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

/** Coordinates are rounded so server and browser trigonometry agree on hydration. */
const COORDINATE_PRECISION = 1e4;
const roundCoordinate = (value: number) =>
  Math.round(value * COORDINATE_PRECISION) / COORDINATE_PRECISION;

const pointAt = (angle: number, radius: number) => {
  const radians = angle * DEGREES_TO_RADIANS;
  return {
    x: roundCoordinate(CENTER + radius * Math.sin(radians)),
    y: roundCoordinate(CENTER - radius * Math.cos(radians)),
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

interface DragState {
  x: number;
  y: number;
  position: number;
  /** The pointer's angle around the dial, or null too close to its centre. */
  angle: number | null;
}

/** Share of the dial's radius around its centre where the angle is too jumpy to read. */
const DEAD_ZONE = 0.25;

/** The pointer's angle around the dial centre, clockwise from 12 o'clock. */
const pointerAngle = (
  event: { clientX: number; clientY: number },
  element: HTMLElement
): number | null => {
  const rect = element.getBoundingClientRect();
  const x = event.clientX - (rect.left + rect.width / 2);
  const y = event.clientY - (rect.top + rect.height / 2);
  if (Math.hypot(x, y) < (rect.width / 2) * DEAD_ZONE) {
    return null;
  }
  return Math.atan2(x, -y) / DEGREES_TO_RADIANS;
};

/** The pointer's angle for circular drags; other drags only need its position. */
const dragAngle = (
  event: PointerEvent<HTMLDivElement>,
  dial: KnobDialContextValue
) =>
  dial.dragDirection === "circular"
    ? pointerAngle(event, event.currentTarget)
    : null;

/** The shortest turn from one angle to another, in -180..180 degrees. */
const turnBetween = (from: number, to: number) =>
  ((to - from + HALF_TURN * 3) % (HALF_TURN * 2)) - HALF_TURN;

/**
 * The next position, from the movement since the last pointer event, so
 * pressing or releasing Shift mid-drag changes the speed without a jump.
 * Circular drags add the turn since the last event, so the knob turns from
 * where it is grabbed and stops at its ends instead of wrapping across the gap.
 */
const dragPosition = (
  event: PointerEvent<HTMLDivElement>,
  last: DragState,
  angle: number | null,
  dial: KnobDialContextValue,
  arc: number
) => {
  const fine = event.shiftKey ? FINE_FACTOR : 1;
  if (dial.dragDirection === "circular") {
    if (last.angle === null || angle === null) {
      return last.position;
    }
    const turn = turnBetween(last.angle, angle);
    return clamp(last.position + (turn / arc) * fine, 0, 1);
  }
  const delta =
    dial.dragDirection === "vertical"
      ? last.y - event.clientY
      : event.clientX - last.x;
  return clamp(last.position + (delta / dial.sensitivity) * fine, 0, 1);
};

const useDialWheel = (
  elementRef: RefObject<HTMLDivElement | null>,
  dial: KnobDialContextValue,
  disabled: boolean
) => {
  const { allowWheel } = dial;
  const onWheel = useEffectEvent(
    (event: WheelEvent, element: HTMLDivElement) => {
      // Shift turns the wheel sideways on some systems.
      const delta = event.deltaY || event.deltaX;
      if (disabled || document.activeElement !== element || delta === 0) {
        return;
      }
      event.preventDefault();
      const fine = event.shiftKey || event.altKey;
      const increment = fine ? dial.fineStep : dial.step;
      const direction = delta < 0 ? 1 : -1;
      const next = dial.quantize(
        dial.latestRef.current + direction * increment,
        increment
      );
      dial.change(next, { event, reason: "wheel" });
      dial.commit(next);
    }
  );

  // A non-passive listener blocks scrolling, so only attach one when the
  // wheel is allowed.
  useEffect(() => {
    const element = elementRef.current;
    if (!(element && allowWheel)) {
      return;
    }
    const listener = (event: WheelEvent) => {
      onWheel(event, element);
    };
    element.addEventListener("wheel", listener, { passive: false });
    return () => {
      element.removeEventListener("wheel", listener);
    };
  }, [allowWheel, elementRef]);
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
  const dragRef = useRef<DragState | null>(null);
  const [dragging, setDragging] = useState(false);
  useDialWheel(dialRef, dial, disabled);

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
      angle: dragAngle(event, dial),
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
    const angle = dragAngle(event, dial);
    const next = dragPosition(event, last, angle, dial, arc);
    dragRef.current = {
      angle,
      position: next,
      x: event.clientX,
      y: event.clientY,
    };
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
        "relative size-(--knob-size) cursor-grab touch-none rounded-full outline-none aria-disabled:cursor-default",
        {
          "data-dragging:cursor-ew-resize": dial.dragDirection === "horizontal",
          "data-dragging:cursor-grabbing": dial.dragDirection === "circular",
          "data-dragging:cursor-ns-resize": dial.dragDirection === "vertical",
        },
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

/** Radii of the volume dial, in view box units. */
const SCALE = {
  label: 47.5,
  labelSize: 4.5,
  majorInner: 34.5,
  majorOuter: 43.5,
  minorInner: 35.5,
  minorOuter: 40.5,
} as const;
/**
 * Radii of the cap, in view box units: inside a KnobScale by default, or
 * filling the inside of a KnobTrack when mini. The shadow starts at `haloFrom`
 * of the halo's radius.
 */
const CAP = {
  default: {
    bezel: 25.5,
    face: 22.5,
    halo: 34,
    haloFrom: 0.6,
    rim: 23.3,
  },
  mini: {
    bezel: 32,
    face: 28.2,
    halo: 36,
    haloFrom: 0.8,
    rim: 29.2,
  },
} as const;
/** The default cap's indicator dot. */
const CAP_DOT = { distance: 17, radius: 1.6 } as const;
/** The mini cap's engraved indicator line, which reads at small sizes. */
const CAP_LINE = { inner: 8.5, lip: 0.6, outer: 23, width: 3.4 } as const;
/** Ticks this close to the lit range's ends still count as lit. */
const TICK_EPSILON = 1e-9;

export interface KnobScaleProps extends Omit<ComponentProps<"g">, "format"> {
  /** Divisions across the arc; draws `ticks + 1` marks. Default 50. */
  ticks?: number;
  /** Every nth tick is long. Default 5. */
  majorEvery?: number;
  /** Every nth tick is numbered; 0 hides the numbers. Default 10. */
  labelEvery?: number;
  /** Text for each number. Default: the knob's `format`. */
  format?: (value: number) => string;
}

/** Tick marks and numbers around the dial, lit from `origin` to the value. */
export const KnobScale = ({
  ticks = 50,
  majorEvery = 5,
  labelEvery = 10,
  format: formatProp,
  className,
  ...props
}: KnobScaleProps) => {
  const { arc, format, originPosition, position } = useKnob("KnobScale");
  const { setDetents, taper } = useKnobDial();
  const formatLabel = formatProp ?? format;
  const litFrom = Math.min(originPosition, position) - TICK_EPSILON;
  const litTo = Math.max(originPosition, position) + TICK_EPSILON;
  const count = Math.max(1, Math.round(ticks));
  // One step for the long ticks drawn and the detents they click on.
  const majorStep = Math.max(1, Math.round(majorEvery));

  useEffect(() => {
    const majors: number[] = [];
    for (let index = 0; index <= count; index += majorStep) {
      majors.push(index / count);
    }
    setDetents(majors);
    return () => {
      setDetents(null);
    };
  }, [count, majorStep, setDetents]);

  const marks = Array.from({ length: count + 1 }, (_, index) => {
    const tickPosition = index / count;
    const angle = angleFor(tickPosition, arc);
    const major = index % majorStep === 0;
    const inner = pointAt(angle, major ? SCALE.majorInner : SCALE.minorInner);
    const outer = pointAt(angle, major ? SCALE.majorOuter : SCALE.minorOuter);
    const lit = tickPosition >= litFrom && tickPosition <= litTo;
    return (
      <line
        className="stroke-muted-foreground/45 data-active:stroke-foreground data-major:stroke-muted-foreground data-major:data-active:stroke-foreground"
        data-active={lit ? "" : undefined}
        data-major={major ? "" : undefined}
        data-slot="knob-tick"
        key={index}
        strokeWidth={major ? 1.1 : 0.55}
        x1={inner.x}
        x2={outer.x}
        y1={inner.y}
        y2={outer.y}
      />
    );
  });

  const labels =
    labelEvery > 0
      ? Array.from(
          { length: Math.floor(count / labelEvery) + 1 },
          (_, index) => {
            const tickPosition = (index * labelEvery) / count;
            const angle = angleFor(tickPosition, arc);
            const { x, y } = pointAt(angle, SCALE.label);
            return (
              <text
                className="fill-muted-foreground"
                data-slot="knob-scale-label"
                dominantBaseline="central"
                fontSize={SCALE.labelSize}
                key={tickPosition}
                textAnchor="middle"
                transform={`rotate(${angle} ${x} ${y})`}
                x={x}
                y={y}
              >
                {formatLabel(roundValue(taper.toValue(tickPosition)))}
              </text>
            );
          }
        )
      : null;

  return (
    <g className={className} data-slot="knob-scale" {...props}>
      {marks}
      {labels}
    </g>
  );
};

/** An id that is safe inside `url(#…)`. */
const useSvgId = () => `knob${useId().replaceAll(/[^\w-]/gu, "")}`;

const knobCapVariants = cva(
  "[--knob-cap-metal:var(--color-white)] [--knob-cap-shade:var(--color-black)]",
  {
    defaultVariants: { variant: "default" },
    variants: {
      variant: {
        default: "[--knob-cap-pitch:0.3px]",
        mini: "[--knob-cap-pitch:0.9px]",
      },
    },
  }
);

type KnobCapVariant = "default" | "mini";

/** The value marker: a dot on the default cap, an engraved line on the mini one. */
const KnobCapIndicator = ({
  angle,
  variant,
}: {
  angle: number;
  variant: KnobCapVariant;
}) => {
  if (variant === "default") {
    const dot = pointAt(angle, CAP_DOT.distance);
    return (
      <circle
        className="fill-(--knob-cap-shade)/85 stroke-(--knob-cap-shade)/45"
        cx={dot.x}
        cy={dot.y}
        data-slot="knob-cap-dot"
        r={CAP_DOT.radius}
        strokeWidth={0.35}
      />
    );
  }
  const inner = pointAt(angle, CAP_LINE.inner);
  const outer = pointAt(angle, CAP_LINE.outer);
  return (
    <>
      {/* Light from above catches the groove's lower wall. */}
      <line
        className="stroke-(--knob-cap-metal)/70"
        strokeLinecap="round"
        strokeWidth={CAP_LINE.width}
        x1={inner.x}
        x2={outer.x}
        y1={inner.y + CAP_LINE.lip}
        y2={outer.y + CAP_LINE.lip}
      />
      <line
        className="stroke-(--knob-cap-shade)/80"
        data-slot="knob-cap-pointer"
        strokeLinecap="round"
        strokeWidth={CAP_LINE.width}
        x1={inner.x}
        x2={outer.x}
        y1={inner.y}
        y2={outer.y}
      />
    </>
  );
};

export interface KnobCapProps extends ComponentProps<"g"> {
  /**
   * `default` sits inside a KnobScale and marks the value with a dot. `mini`
   * fills the inside of a KnobTrack and marks it with an engraved line, for
   * small knobs. Default `default`.
   */
  variant?: KnobCapVariant;
}

/**
 * A brushed aluminium cap in a dark bezel, with a marker that turns with the
 * value. The metal is `--knob-cap-metal` shaded by `--knob-cap-shade`.
 */
export const KnobCap = ({
  variant = "default",
  className,
  ...props
}: KnobCapProps) => {
  const { arc, position } = useKnob("KnobCap");
  const id = useSvgId();
  const angle = angleFor(position, arc);
  const cap = CAP[variant];
  return (
    <g
      className={cn(knobCapVariants({ variant }), className)}
      data-slot="knob-cap"
      data-variant={variant}
      {...props}
    >
      <defs>
        <filter height="100%" id={`${id}-grain`} width="100%" x="0" y="0">
          <feTurbulence
            baseFrequency={0.9}
            numOctaves={3}
            seed={7}
            type="fractalNoise"
          />
          <feColorMatrix type="saturate" values="0" />
          <feComposite in2="SourceGraphic" operator="in" />
        </filter>
        <radialGradient id={`${id}-halo`}>
          <stop
            className="[stop-color:var(--knob-cap-shade)]"
            offset={cap.haloFrom}
            stopOpacity={0.55}
          />
          <stop
            className="[stop-color:var(--knob-cap-shade)]"
            offset="1"
            stopOpacity={0}
          />
        </radialGradient>
        {/* Light from above: the bezel brightens at the top, the rim at the bottom. */}
        <linearGradient id={`${id}-bezel`} x1="0" x2="0" y1="0" y2="1">
          <stop
            className="[stop-color:var(--knob-cap-metal)]"
            offset="0"
            stopOpacity={0.16}
          />
          <stop
            className="[stop-color:var(--knob-cap-metal)]"
            offset="1"
            stopOpacity={0}
          />
        </linearGradient>
        <linearGradient id={`${id}-rim`} x1="0" x2="0" y1="0" y2="1">
          <stop
            className="[stop-color:var(--knob-cap-shade)]"
            offset="0"
            stopOpacity={0.62}
          />
          <stop
            className="[stop-color:var(--knob-cap-shade)]"
            offset="1"
            stopOpacity={0.08}
          />
        </linearGradient>
      </defs>
      <circle cx={CENTER} cy={CENTER} fill={`url(#${id}-halo)`} r={cap.halo} />
      <circle
        className="fill-(--knob-cap-shade)/85 stroke-(--knob-cap-metal)/15"
        cx={CENTER}
        cy={CENTER}
        r={cap.bezel}
        strokeWidth={0.4}
      />
      <circle
        cx={CENTER}
        cy={CENTER}
        fill={`url(#${id}-bezel)`}
        r={cap.bezel}
      />
      <circle
        className="fill-(--knob-cap-metal)"
        cx={CENTER}
        cy={CENTER}
        r={cap.rim}
      />
      <circle cx={CENTER} cy={CENTER} fill={`url(#${id}-rim)`} r={cap.rim} />
      {/* SVG has no conic gradient, so the face is HTML with a CSS one. */}
      <foreignObject
        height={cap.face * 2}
        width={cap.face * 2}
        x={CENTER - cap.face}
        y={CENTER - cap.face}
      >
        <div
          className="size-full rounded-full bg-(--knob-cap-metal) bg-[repeating-radial-gradient(circle,var(--knob-cap-brush)_0_var(--knob-cap-pitch),transparent_var(--knob-cap-pitch)_calc(var(--knob-cap-pitch)*2)),conic-gradient(from_15deg,var(--knob-cap-sheen),transparent_9%,var(--knob-cap-sheen)_21%,transparent_32%,var(--knob-cap-sheen-deep)_46%,transparent_58%,var(--knob-cap-sheen)_70%,transparent_83%,var(--knob-cap-sheen))] [--knob-cap-brush:color-mix(in_oklab,var(--knob-cap-shade)_7%,transparent)] [--knob-cap-sheen-deep:color-mix(in_oklab,var(--knob-cap-shade)_50%,transparent)] [--knob-cap-sheen:color-mix(in_oklab,var(--knob-cap-shade)_34%,transparent)]"
          data-slot="knob-cap-face"
        />
      </foreignObject>
      <circle
        cx={CENTER}
        cy={CENTER}
        data-slot="knob-cap-grain"
        filter={`url(#${id}-grain)`}
        opacity={0.25}
        pointerEvents="none"
        r={cap.face}
        transform={`rotate(${angle} ${CENTER} ${CENTER})`}
      />
      <KnobCapIndicator angle={angle} variant={variant} />
    </g>
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
        "bg-background ring-ring/50 focus-visible:ring-foreground h-4 w-(--knob-value-width) min-w-0 rounded-sm p-0 text-center font-mono text-xs tabular-nums ring-1 outline-none focus-visible:ring-2",
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
  /** Pixels of vertical or horizontal drag for the full range. Default 200. */
  sensitivity?: number;
  /** Default `linear`. */
  scale?: "linear" | "log";
  /** The wheel adjusts the value while focused. Default false. */
  allowWheel?: boolean;
  /**
   * Plays a soft click on each graduation: KnobScale's long ticks, or every
   * `largeStep` without a scale. Default false.
   */
  clickSound?: boolean;
  format?: (value: number) => string;
  /** Reads a typed value. Default: the first number, with "k" as thousands. */
  parse?: (text: string) => number | null;
  size?: AudioSize;
  disabled?: boolean;
}

/** Shortest gap between clicks, so a fast turn ticks instead of buzzing. */
const CLICK_INTERVAL_MS = 30;
const CLICK_SECONDS = 0.006;
const CLICK_VOLUME = 0.12;
/** Each click is pitched a little at random, like a real detent. */
const CLICK_PITCH_SPREAD = 0.04;
/** A short noise snap over a high, fast-damped ring: a tick, not a thud. */
const CLICK_PARTS = [
  { decay: 0.0004, gain: 0.6, hz: 0 },
  { decay: 0.0012, gain: 0.4, hz: 4200 },
] as const;

/** Positions this close to a detent count as on it. */
const DETENT_EPSILON = 1e-9;

/** Whether moving between positions reaches or passes a detent. */
const reachesDetent = (detents: readonly number[], from: number, to: number) =>
  detents.some((detent) =>
    from < to
      ? detent > from + DETENT_EPSILON && detent <= to + DETENT_EPSILON
      : detent < from - DETENT_EPSILON && detent >= to - DETENT_EPSILON
  );

/** Whether moving between values reaches or passes a multiple of `every`. */
const reachesMultiple = (
  from: number,
  to: number,
  min: number,
  every: number
) => {
  const start = roundValue((from - min) / every);
  const end = roundValue((to - min) / every);
  return from < to
    ? Math.floor(end) > Math.floor(start)
    : Math.ceil(start) > Math.ceil(end);
};

const clickBuffers = new WeakMap<BaseAudioContext, AudioBuffer>();
let lastClickAt = Number.NEGATIVE_INFINITY;

/** One synthesised click, made once per audio context. */
const clickBuffer = (context: BaseAudioContext) => {
  const cached = clickBuffers.get(context);
  if (cached) {
    return cached;
  }
  const { sampleRate } = context;
  const buffer = context.createBuffer(
    1,
    Math.ceil(CLICK_SECONDS * sampleRate),
    sampleRate
  );
  const samples = buffer.getChannelData(0);
  for (let index = 0; index < samples.length; index += 1) {
    const time = index / sampleRate;
    let sample = 0;
    for (const part of CLICK_PARTS) {
      const wave =
        part.hz === 0
          ? Math.random() * 2 - 1
          : Math.sin(2 * Math.PI * part.hz * time);
      sample += part.gain * wave * Math.exp(-time / part.decay);
    }
    samples[index] = sample;
  }
  clickBuffers.set(context, buffer);
  return buffer;
};

const resumeContext = async (context: AudioContext) => {
  try {
    await context.resume();
  } catch {
    // Without a user gesture the browser refuses; the next one tries again.
  }
};

/** Plays a click through the shared audio context, at most every 30 ms. */
const playClick = () => {
  const now = performance.now();
  const context = getSharedAudioContext();
  if (!context || now - lastClickAt < CLICK_INTERVAL_MS) {
    return;
  }
  lastClickAt = now;
  if (context.state === "suspended") {
    resumeContext(context);
  }
  const source = context.createBufferSource();
  source.buffer = clickBuffer(context);
  source.playbackRate.value =
    1 + (Math.random() - 0.5) * CLICK_PITCH_SPREAD * 2;
  const gain = context.createGain();
  gain.gain.value = CLICK_VOLUME;
  source.connect(gain).connect(context.destination);
  source.addEventListener("ended", () => {
    source.disconnect();
    gain.disconnect();
  });
  source.start();
};

interface KnobValueOptions {
  value: number | undefined;
  defaultValue: number | undefined;
  resetValue: number;
  min: number;
  max: number;
  onValueChange: KnobProps["onValueChange"];
  /** Whether a change between two values should click; null for silence. */
  clicksBetween: ((from: number, to: number) => boolean) | null;
}

/** Controlled or uncontrolled value with change notifications. */
const useKnobValue = ({
  value: valueProp,
  defaultValue,
  resetValue,
  min,
  max,
  onValueChange,
  clicksBetween,
}: KnobValueOptions) => {
  const [uncontrolled, setUncontrolled] = useState(() =>
    clamp(defaultValue ?? resetValue, min, max)
  );
  const value = valueProp ?? uncontrolled;
  const latestRef = useLatest(value);
  const controlled = valueProp !== undefined;

  const change = useCallback(
    (next: number, details: KnobChangeDetails) => {
      const previous = latestRef.current;
      if (next === previous) {
        return;
      }
      latestRef.current = next;
      if (!controlled) {
        setUncontrolled(next);
      }
      onValueChange?.(next, details);
      if (clicksBetween?.(previous, next)) {
        playClick();
      }
    },
    [clicksBetween, controlled, latestRef, onValueChange]
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
  clickSound = false,
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
  const taper: Taper = useMemo(
    () => (scale === "log" ? logTaper(min, max) : linearTaper(min, max)),
    [max, min, scale]
  );

  // Clicks land on KnobScale's long ticks, or every largeStep without one.
  const [detents, setDetents] = useState<readonly number[] | null>(null);
  const clicksBetween = useMemo(() => {
    if (!clickSound) {
      return null;
    }
    if (detents) {
      return (from: number, to: number) =>
        reachesDetent(detents, taper.toPosition(from), taper.toPosition(to));
    }
    return (from: number, to: number) =>
      reachesMultiple(from, to, min, largeStep);
  }, [clickSound, detents, largeStep, min, taper]);

  const { change, latestRef, value } = useKnobValue({
    clicksBetween,
    defaultValue,
    max,
    min,
    onValueChange,
    resetValue: reset,
    value: valueProp,
  });

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
      setDetents,
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

export { knobCapVariants, knobVariants };
