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
  ReactNode,
  RefObject,
} from "react";

import type { DialDragDirection } from "@/components/ui/knob";
import {
  DialProvider,
  pointAt,
  pointerAngle,
  turnBetween,
  useDialClick,
  useDialWheel,
} from "@/components/ui/knob";
import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { createClickSound } from "@/lib/audio/click";
import type { ClickSound } from "@/lib/audio/click";
import { cn } from "@/lib/utils";

const VIEWBOX = 100;
const CENTER = 50;
/** The dial's edge, in view box units. */
const DIAL_RADIUS = 50;
const FULL_TURN = 360;
/** Sweeps within half a degree of a turn wrap, so a rounded step like 51.4 (360 / 7) works. */
const WRAP_TOLERANCE = 0.5;
/** Pointer travel per position for vertical and horizontal drags. */
const DRAG_STEP_PX = 24;
/** Shortest gap between clicks, so a fast turn ticks instead of buzzing. */
const CLICK_INTERVAL_MS = 33;
/** Shortest radial segment of a leader that still reads as one. */
const MIN_RAY = 2;
const EPSILON = 1e-6;

const rotarySelectorClick = createClickSound({
  noise: { decayMs: 0.75, gain: 0.21 },
  pitchSpread: 0.03,
  tone: { decayMs: 1.3, gain: 1, hz: 460 },
  volume: 0.18,
});

export type RotarySelectorItem = string | number;

export type RotarySelectorChangeReason =
  | "drag"
  | "keyboard"
  | "wheel"
  | "reset"
  | "label";

export interface RotarySelectorChangeDetails {
  reason: RotarySelectorChangeReason;
  event?: Event;
}

export interface RotarySelectorPosition<
  Value extends RotarySelectorItem = RotarySelectorItem,
> {
  value: Value;
  index: number;
  /** Degrees clockwise from 12 o'clock. */
  angle: number;
  selected: boolean;
  /** A point at this radius along the position, in the dial's view box. */
  pointAt: (radius: number) => { x: number; y: number };
}

export interface RotarySelectorRenderProps<Value extends RotarySelectorItem> {
  value: Value;
  positions: readonly RotarySelectorPosition<Value>[];
  /** The position of one value, for one-off print. */
  position: (value: Value) => RotarySelectorPosition<Value>;
}

/** A tuple of `Length` items, or of 361 for anything longer than a turn. */
type Tuple<
  Length extends number,
  Items extends unknown[] = [],
> = Items["length"] extends Length | 361
  ? Items
  : Tuple<Length, [...Items, unknown]>;

/** Adds `Step` once per value, and stops as soon as the sum passes 360. */
type OverflowsTurn<
  Values extends readonly unknown[],
  Step extends unknown[],
  Sweep extends unknown[] = [],
> = Sweep extends [...Tuple<360>, unknown, ...unknown[]]
  ? true
  : Values extends readonly [unknown, ...infer Rest]
    ? OverflowsTurn<Rest, Step, [...Sweep, ...Step]>
    : false;

type Magnitude<Angle extends number> =
  `${Angle}` extends `-${infer Positive extends number}` ? Positive : Angle;

/**
 * `StepAngle`, or an error message when a literal tuple of values times a
 * whole literal step is over a turn. Other overflows are caught at runtime.
 */
type CheckedStepAngle<
  Values extends readonly unknown[],
  StepAngle extends number,
> = number extends Values["length"] | StepAngle
  ? StepAngle
  : `${StepAngle}` extends `${string}.${string}`
    ? StepAngle
    : OverflowsTurn<Values, Tuple<Magnitude<StepAngle>>> extends true
      ? `${Values["length"]} values × ${Magnitude<StepAngle>}° is over 360°`
      : StepAngle;

interface RotarySelectorContextValue {
  angles: readonly number[];
  index: number;
  stepAngle: number;
  wraps: boolean;
  disabled: boolean;
  dragDirection: DialDragDirection;
  allowWheel: boolean;
  labelId: string;
  valueText: string;
  /** Characters in the widest value, for a steady readout. */
  valueWidth: number;
  latestRef: RefObject<number>;
  change: (index: number, details: RotarySelectorChangeDetails) => void;
  commit: () => void;
  reset: () => void;
}

const RotarySelectorContext = createContext<RotarySelectorContextValue | null>(
  null
);

const useRotarySelector = (part: string) => {
  const context = useContext(RotarySelectorContext);
  if (!context) {
    throw new Error(`${part} must be used inside RotarySelector.`);
  }
  return context;
};

/** A ref that always holds the latest value, for event handlers. */
const useLatest = <T,>(value: T): RefObject<T> => {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  });
  return ref;
};

type Geometry = Pick<
  RotarySelectorContextValue,
  "angles" | "stepAngle" | "wraps"
>;

/**
 * The index one position clockwise (`turn` 1) or anticlockwise (-1), or null
 * past an end. With a negative `stepAngle`, clockwise is a lower index.
 */
const stepFrom = (index: number, turn: 1 | -1, geometry: Geometry) => {
  const count = geometry.angles.length;
  const next = index + (geometry.stepAngle < 0 ? -turn : turn);
  if (geometry.wraps) {
    return (next + count) % count;
  }
  return next < 0 || next >= count ? null : next;
};

const ARROW_TURNS: Record<string, 1 | -1> = {
  ArrowDown: -1,
  ArrowLeft: -1,
  ArrowRight: 1,
  ArrowUp: 1,
};

/** The index a key moves to, or undefined for keys the dial leaves alone. */
const keyTarget = (key: string, index: number, geometry: Geometry) => {
  if (key === "Home") {
    return 0;
  }
  if (key === "End") {
    return Math.max(0, geometry.angles.length - 1);
  }
  const turn = ARROW_TURNS[key];
  return turn ? (stepFrom(index, turn, geometry) ?? index) : undefined;
};

/**
 * Circular drags follow the pointer's angle unwrapped across turns, so a
 * selector that does not wrap stops at its ends like a real switch: past an
 * end, the pointer has to come back the way it went. The first angle is
 * taken within half a turn of the middle of the positions.
 */
const unwrappedAngle = (
  pointer: number,
  previous: number | null,
  angles: readonly number[]
) => {
  const reference = previous ?? ((angles[0] ?? 0) + (angles.at(-1) ?? 0)) / 2;
  return reference + turnBetween(reference, pointer);
};

/** The position nearest an angle around the circle; the lower index on a tie. */
const nearestIndex = (angle: number, angles: readonly number[]) => {
  let nearest = 0;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const [index, candidate] of angles.entries()) {
    const distance = Math.abs(turnBetween(candidate, angle));
    if (distance < nearestDistance) {
      nearest = index;
      nearestDistance = distance;
    }
  }
  return nearest;
};

/**
 * The index at an unwrapped angle. A selector that wraps takes the nearest
 * position, so a rounded step neither drifts turn after turn nor jumps at
 * the seam. One that does not wrap stops at its ends.
 */
const indexAt = (angle: number, { angles, stepAngle, wraps }: Geometry) => {
  if (wraps) {
    return nearestIndex(angle, angles);
  }
  // A step of 0, already reported as an error, gives NaN here.
  const steps = Math.round((angle - (angles[0] ?? 0)) / stepAngle) || 0;
  return Math.max(0, Math.min(steps, angles.length - 1));
};

interface DragState {
  x: number;
  y: number;
  /** Vertical and horizontal drags: pixels toward the next step. */
  travel: number;
  /** Circular drags: the pointer's last readable angle, unwrapped across turns. */
  angle: number | null;
}

/** The index after a vertical or horizontal move, and the travel left over. */
const linearDrag = (
  event: PointerEvent<HTMLDivElement>,
  drag: DragState,
  index: number,
  geometry: Geometry & { vertical: boolean }
) => {
  let travel =
    drag.travel +
    (geometry.vertical ? drag.y - event.clientY : event.clientX - drag.x);
  let next = index;
  while (Math.abs(travel) >= DRAG_STEP_PX) {
    const turn = travel > 0 ? 1 : -1;
    travel -= turn * DRAG_STEP_PX;
    next = stepFrom(next, turn, geometry) ?? next;
  }
  return { next, travel };
};

export interface RotarySelectorDialProps extends ComponentProps<"div"> {
  /** Radius of the drag circle, in view box units. Default: the cap's, or 50 without a cap. */
  hitRadius?: number;
}

/**
 * The turning part: a slider whose print (marks, leaders, labels) and cap
 * are SVG children in a 100 × 100 view box. Only a circle around the cap
 * starts a drag, and only the position labels take clicks.
 */
export const RotarySelectorDial = ({
  hitRadius,
  className,
  children,
  ref,
  style,
  ...props
}: RotarySelectorDialProps) => {
  const selector = useRotarySelector("RotarySelectorDial");
  const { angles, change, commit, disabled, dragDirection, latestRef } =
    selector;
  const dragRef = useRef<DragState | null>(null);
  const [dragging, setDragging] = useState(false);
  const [capRadius, setCapRadius] = useState<number | null>(null);
  const angle = angles[selector.index] ?? 0;

  const dialRef = useDialWheel(
    ref,
    selector.allowWheel && !disabled,
    (turn, event) => {
      const { current } = latestRef;
      change(stepFrom(current, turn, selector) ?? current, {
        event,
        reason: "wheel",
      });
      commit();
    }
  );

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) {
      return;
    }
    const next = keyTarget(event.key, latestRef.current, selector);
    if (next === undefined) {
      return;
    }
    event.preventDefault();
    change(next, { event: event.nativeEvent, reason: "keyboard" });
    commit();
  };

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (disabled || event.button !== 0) {
      return;
    }
    if (event.altKey) {
      event.preventDefault();
      selector.reset();
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    event.currentTarget.focus();
    const pointer =
      dragDirection === "circular"
        ? pointerAngle(event, event.currentTarget)
        : null;
    dragRef.current = {
      angle: pointer === null ? null : unwrappedAngle(pointer, null, angles),
      travel: 0,
      x: event.clientX,
      y: event.clientY,
    };
    setDragging(true);
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag) {
      return;
    }
    const details = { event: event.nativeEvent, reason: "drag" } as const;
    if (dragDirection === "circular") {
      const pointer = pointerAngle(event, event.currentTarget);
      if (pointer === null) {
        return;
      }
      const unwrapped = unwrappedAngle(pointer, drag.angle, angles);
      dragRef.current = { ...drag, angle: unwrapped };
      change(indexAt(unwrapped, selector), details);
      return;
    }
    const { next, travel } = linearDrag(event, drag, latestRef.current, {
      ...selector,
      vertical: dragDirection === "vertical",
    });
    dragRef.current = { ...drag, travel, x: event.clientX, y: event.clientY };
    change(next, details);
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
    commit();
  };

  const named = props["aria-label"] !== undefined;

  return (
    <div
      aria-disabled={disabled || undefined}
      aria-labelledby={named ? undefined : selector.labelId}
      aria-valuemax={angles.length - 1}
      aria-valuemin={0}
      aria-valuenow={selector.index}
      aria-valuetext={selector.valueText}
      className={cn(
        "group/rotary-selector-dial focus-visible:ring-ring/50 pointer-events-none relative size-(--knob-size) touch-none rounded-full outline-none focus-visible:ring-3 data-dragging:pointer-events-auto",
        {
          "data-dragging:cursor-ew-resize": dragDirection === "horizontal",
          "data-dragging:cursor-grabbing": dragDirection === "circular",
          "data-dragging:cursor-ns-resize": dragDirection === "vertical",
        },
        className
      )}
      data-disabled={disabled ? "" : undefined}
      data-dragging={dragging ? "" : undefined}
      data-slot="rotary-selector-dial"
      data-wraps={selector.wraps ? "" : undefined}
      onDoubleClick={() => {
        if (!disabled) {
          selector.reset();
        }
      }}
      onKeyDown={handleKeyDown}
      onLostPointerCapture={endDrag}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endDrag}
      ref={dialRef}
      role="slider"
      style={{ "--knob-angle": `${angle}deg`, ...style } as CSSProperties}
      tabIndex={disabled ? -1 : 0}
      {...props}
    >
      <svg
        aria-hidden
        className="size-full overflow-visible"
        viewBox={`0 0 ${VIEWBOX} ${VIEWBOX}`}
      >
        <DialProvider angle={angle} onCapRadiusChange={setCapRadius}>
          {children}
        </DialProvider>
        <circle
          className="[pointer-events:all] cursor-grab touch-none group-aria-disabled/rotary-selector-dial:cursor-default group-data-dragging/rotary-selector-dial:cursor-[inherit]"
          cx={CENTER}
          cy={CENTER}
          data-slot="rotary-selector-hit-area"
          fill="none"
          r={hitRadius ?? capRadius ?? DIAL_RADIUS}
        />
      </svg>
    </div>
  );
};

const focusDial = (from: Element) => {
  from
    .closest<HTMLElement>("[data-slot='rotary-selector-dial']")
    ?.focus({ preventScroll: true });
};

interface PositionPartProps {
  position: RotarySelectorPosition;
}

export interface RotarySelectorPositionLabelProps
  extends PositionPartProps, ComponentProps<"g"> {}

/** Print for one position that selects it when clicked. Not a tab stop. */
export const RotarySelectorPositionLabel = ({
  position,
  className,
  onClick,
  onDoubleClick,
  onPointerDown,
  ...props
}: RotarySelectorPositionLabelProps) => {
  const { change, commit, disabled } = useRotarySelector(
    "RotarySelectorPositionLabel"
  );
  return (
    <g
      aria-hidden
      className={cn(
        "text-muted-foreground data-selected:text-foreground pointer-events-auto cursor-pointer fill-current group-aria-disabled/rotary-selector-dial:cursor-default",
        className
      )}
      data-selected={position.selected ? "true" : undefined}
      data-slot="rotary-selector-position-label"
      onClick={(event) => {
        onClick?.(event);
        if (disabled) {
          return;
        }
        change(position.index, {
          event: event.nativeEvent,
          reason: "label",
        });
        commit();
        focusDial(event.currentTarget);
      }}
      // A press on a label neither drags nor resets the dial around it.
      onDoubleClick={(event) => {
        onDoubleClick?.(event);
        event.stopPropagation();
      }}
      onPointerDown={(event) => {
        onPointerDown?.(event);
        event.stopPropagation();
      }}
      {...props}
    />
  );
};

export interface RotarySelectorPositionMarkProps
  extends PositionPartProps, ComponentProps<"line"> {
  /** Default 52, just outside the dial. */
  inner?: number;
  /** Default 56. */
  outer?: number;
}

/** A radial tick at one position. */
export const RotarySelectorPositionMark = ({
  position,
  inner = 52,
  outer = 56,
  className,
  ...props
}: RotarySelectorPositionMarkProps) => {
  const from = position.pointAt(inner);
  const to = position.pointAt(outer);
  return (
    <line
      className={cn(
        "stroke-muted-foreground data-selected:stroke-foreground",
        className
      )}
      data-selected={position.selected ? "true" : undefined}
      data-slot="rotary-selector-position-mark"
      strokeLinecap="round"
      strokeWidth={1}
      x1={from.x}
      x2={to.x}
      y1={from.y}
      y2={to.y}
      {...props}
    />
  );
};

export interface RotarySelectorLeader {
  /** Where the leader ends, on its column or row. */
  end: { x: number; y: number };
  /** Where the ray turns into the run. */
  bend: { x: number; y: number };
  /** The SVG path data of both segments. */
  path: string;
}

/** Where a leader's run ends: a column (`x`) or a row (`y`) in the view box. */
export type RotarySelectorLeaderTarget =
  | { x: number; y?: never }
  | { y: number; x?: never };

const roundForMessage = (value: number) => Number(value.toFixed(1));

/** The leader's two segments, and what makes them impossible to draw well. */
const leaderGeometry = (
  angle: number,
  from: number,
  ray: number,
  to: RotarySelectorLeaderTarget
) => {
  const start = pointAt(angle, from);
  const bend = pointAt(angle, from + ray);
  const end =
    to.x === undefined ? { x: bend.x, y: to.y } : { x: to.x, y: bend.y };
  const column = to.x !== undefined;
  const axis = column ? "x" : "y";
  const target = column ? to.x : to.y;
  const towardTarget = Math.sign(target - CENTER);
  const along = pointAt(angle, 1);
  const rayHeading = column ? along.x - CENTER : along.y - CENTER;
  const run = column ? end.x - bend.x : end.y - bend.y;
  const line = column ? "column" : "row";

  const problems: string[] = [];
  const bendRadius = from + ray;
  if (bendRadius < DIAL_RADIUS) {
    problems.push(
      `the bend is inside the dial (radius ${roundForMessage(bendRadius)} < ${DIAL_RADIUS})`
    );
  } else if (ray < MIN_RAY) {
    problems.push(
      `the ray is too short (${roundForMessage(ray)} < ${MIN_RAY})`
    );
  }
  if (rayHeading * towardTarget < -EPSILON) {
    problems.push(`the ray points away from its ${line} (${axis} = ${target})`);
  }
  if (run * towardTarget < -EPSILON) {
    problems.push(
      `the bend is past its ${line} (${axis} = ${target}), so the run doubles back`
    );
  }
  const path = `M ${start.x} ${start.y} L ${bend.x} ${bend.y} L ${end.x} ${end.y}`;
  return { leader: { bend, end, path }, problems };
};

/** One console warning per problem in development, repeated only when the problems change. */
const useLeaderWarnings = (value: RotarySelectorItem, problems: string[]) => {
  const messages = problems
    .map(
      (problem) =>
        `RotarySelectorPositionLeader for ${JSON.stringify(value)}: ${problem}.`
    )
    .join("\n");
  useEffect(() => {
    if (!messages || process.env.NODE_ENV === "production") {
      return;
    }
    for (const message of messages.split("\n")) {
      console.warn(message);
    }
  }, [messages]);
};

export interface RotarySelectorPositionLeaderProps
  extends
    PositionPartProps,
    Omit<ComponentProps<"path">, "children" | "from" | "to"> {
  /** Radius the leader starts at. */
  from: number;
  /** Length of the radial segment. */
  ray: number;
  /** End of the run: a column `x` or a row `y`. */
  to: RotarySelectorLeaderTarget;
  /** Placed at the leader's end, or a function that places them. */
  children?: ReactNode | ((leader: RotarySelectorLeader) => ReactNode);
}

/**
 * A line from a position to its label: a radial ray, then a horizontal run to
 * a column or a vertical run to a row. Impossible geometry still draws, with
 * `data-invalid` and a warning in development.
 */
export const RotarySelectorPositionLeader = ({
  position,
  from,
  ray,
  to,
  children,
  className,
  ...props
}: RotarySelectorPositionLeaderProps) => {
  const { leader, problems } = leaderGeometry(position.angle, from, ray, to);
  useLeaderWarnings(position.value, problems);
  return (
    <>
      <path
        className={cn(
          "stroke-muted-foreground data-selected:stroke-foreground data-invalid:stroke-destructive data-invalid:[stroke-dasharray:2_1.5]",
          className
        )}
        d={leader.path}
        data-invalid={problems.length > 0 ? "" : undefined}
        data-selected={position.selected ? "true" : undefined}
        data-slot="rotary-selector-position-leader"
        fill="none"
        strokeLinejoin="round"
        strokeWidth={0.8}
        {...props}
      />
      {typeof children === "function" ? (
        children(leader)
      ) : (
        <g transform={`translate(${leader.end.x} ${leader.end.y})`}>
          {children}
        </g>
      )}
    </>
  );
};

/** The selected value's text, from `format`. */
export const RotarySelectorValue = ({
  className,
  style,
  ...props
}: ComponentProps<"span">) => {
  const { valueText, valueWidth } = useRotarySelector("RotarySelectorValue");
  return (
    <span
      className={cn(
        "text-muted-foreground inline-block min-w-(--rotary-selector-value-width) text-center font-mono text-xs whitespace-nowrap tabular-nums",
        className
      )}
      data-slot="rotary-selector-value"
      style={
        {
          "--rotary-selector-value-width": `${valueWidth}ch`,
          ...style,
        } as CSSProperties
      }
      {...props}
    >
      {valueText}
    </span>
  );
};

/** The control's name, which labels the dial. */
export const RotarySelectorLabel = ({
  className,
  ...props
}: ComponentProps<"span">) => {
  const { labelId } = useRotarySelector("RotarySelectorLabel");
  return (
    <span
      className={cn("text-xs font-medium", className)}
      data-slot="rotary-selector-label"
      id={labelId}
      {...props}
    />
  );
};

const rotarySelectorVariants = cva(
  "group/rotary-selector inline-flex flex-col items-center gap-1.5 select-none data-disabled:opacity-50",
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

export interface RotarySelectorProps<
  Values extends readonly RotarySelectorItem[],
  StepAngle extends number = number,
> extends Omit<
  ComponentProps<"div">,
  "children" | "defaultValue" | "onChange"
> {
  /** The positions' values, in index order. */
  values: Values;
  value?: Values[number];
  /** Default: the first value. */
  defaultValue?: Values[number];
  onValueChange?: (
    value: Values[number],
    details: RotarySelectorChangeDetails
  ) => void;
  /** At the end of every interaction, even without a change. */
  onValueCommitted?: (value: Values[number]) => void;
  /** Double-click or Alt+click restores this. Default `defaultValue`. */
  resetValue?: Values[number];
  /** Angle of the first value, clockwise from 12 o'clock. Default 0. */
  startAngle?: number;
  /**
   * Degrees between values; negative runs anticlockwise. Default 30. The
   * selector wraps when the values fill a whole turn.
   */
  stepAngle?: CheckedStepAngle<Values, StepAngle>;
  /** Text for `aria-valuetext` and RotarySelectorValue. Default `String`. */
  format?: (value: Values[number]) => string;
  /** How dragging the cap turns the selector. Default `vertical`. */
  dragDirection?: DialDragDirection;
  /** The wheel turns the selector while it has focus. Default true. */
  allowWheel?: boolean;
  /**
   * Plays a click on each position change. Pass a `ClickSound` to replace
   * the selector's own. Default false.
   */
  clickSound?: boolean | ClickSound;
  size?: AudioSize;
  disabled?: boolean;
  children: (selector: RotarySelectorRenderProps<Values[number]>) => ReactNode;
}

/** What makes the positions impossible to lay out, or null. */
const geometryError = (count: number, stepAngle: number) => {
  if (count === 0) {
    return "RotarySelector: values is empty.";
  }
  if (!Number.isFinite(stepAngle) || stepAngle === 0) {
    return `RotarySelector: stepAngle must be a finite number other than 0, not ${stepAngle}.`;
  }
  const sweep = count * Math.abs(stepAngle);
  if (sweep <= FULL_TURN + WRAP_TOLERANCE) {
    return null;
  }
  const step = roundForMessage(Math.abs(stepAngle));
  return `RotarySelector: ${count} values × ${step}° = ${roundForMessage(sweep)}°, max ${FULL_TURN}°.`;
};

/** Impossible geometry throws in development, and logs in production. */
const useGeometryCheck = (error: string | null) => {
  if (error && process.env.NODE_ENV !== "production") {
    throw new Error(error);
  }
  useEffect(() => {
    if (error) {
      console.error(error);
    }
  }, [error]);
};

/** A development warning for a value the selector cannot show. */
const useMissingValueWarning = (
  name: "value" | "defaultValue" | "resetValue",
  value: RotarySelectorItem | undefined,
  values: readonly RotarySelectorItem[]
) => {
  const missing = value !== undefined && !values.includes(value);
  useEffect(() => {
    if (missing && process.env.NODE_ENV !== "production") {
      console.warn(
        `RotarySelector: ${name} ${JSON.stringify(value)} is not in values.`
      );
    }
  }, [missing, name, value]);
};

/** Controlled or uncontrolled index, with change notifications and a click. */
const useSelectedIndex = <Value extends RotarySelectorItem>(
  values: readonly Value[],
  value: Value | undefined,
  defaultValue: Value | undefined,
  onValueChange: RotarySelectorProps<readonly Value[], number>["onValueChange"],
  playClick: (() => void) | null
) => {
  const [uncontrolled, setUncontrolled] = useState(
    () => defaultValue ?? values[0]
  );
  const selected = value ?? uncontrolled;
  const index = Math.max(0, values.indexOf(selected as Value));
  const latestRef = useLatest(index);
  const controlled = value !== undefined;

  const change = useCallback(
    (next: number, details: RotarySelectorChangeDetails) => {
      if (next === latestRef.current) {
        return;
      }
      latestRef.current = next;
      const nextValue = values[next] as Value;
      if (!controlled) {
        setUncontrolled(nextValue);
      }
      onValueChange?.(nextValue, details);
      playClick?.();
    },
    [controlled, latestRef, onValueChange, playClick, values]
  );

  return { change, index, latestRef };
};

/**
 * A rotary switch: one of a fixed list of values, never empty. The children
 * function gets typed positions to compose the panel print from.
 */
export const RotarySelector = <
  const Values extends readonly RotarySelectorItem[],
  StepAngle extends number,
>({
  values,
  value: valueProp,
  defaultValue,
  onValueChange,
  onValueCommitted,
  resetValue,
  startAngle = 0,
  stepAngle: stepAngleProp,
  format = String,
  dragDirection = "vertical",
  allowWheel = true,
  clickSound = false,
  size: sizeProp,
  disabled: disabledProp,
  className,
  children,
  ...props
}: RotarySelectorProps<Values, StepAngle>) => {
  type Value = Values[number];
  const config = useAudioConfig();
  const disabled = disabledProp ?? config.disabled ?? false;
  const size = sizeProp ?? config.size ?? "default";
  const stepAngle = typeof stepAngleProp === "number" ? stepAngleProp : 30;
  const count = values.length;
  const sweep = count * Math.abs(stepAngle);
  useGeometryCheck(geometryError(count, stepAngle));
  useMissingValueWarning("value", valueProp, values);
  useMissingValueWarning("resetValue", resetValue, values);
  useMissingValueWarning("defaultValue", defaultValue, values);

  const labelId = useId();
  const playClick = useDialClick(
    clickSound,
    rotarySelectorClick,
    CLICK_INTERVAL_MS
  );
  const { change, index, latestRef } = useSelectedIndex(
    values,
    valueProp,
    defaultValue,
    onValueChange,
    playClick
  );
  const resetIndex = Math.max(
    0,
    values.indexOf((resetValue ?? defaultValue ?? values[0]) as Value)
  );

  const angles = useMemo(
    () => values.map((_, at) => startAngle + at * stepAngle),
    [startAngle, stepAngle, values]
  );
  const positions = useMemo(
    () =>
      values.map((item, at): RotarySelectorPosition<Value> => ({
        angle: angles[at] ?? 0,
        index: at,
        pointAt: (radius) => pointAt(angles[at] ?? 0, radius),
        selected: at === index,
        value: item,
      })),
    [angles, index, values]
  );
  const position = useCallback(
    (item: Value) => {
      const found = positions[values.indexOf(item)];
      if (!found) {
        throw new Error(`RotarySelector: no position for ${String(item)}.`);
      }
      return found;
    },
    [positions, values]
  );
  const selected = values[index] as Value;

  const commit = useCallback(() => {
    const committed = values[latestRef.current];
    if (committed !== undefined) {
      onValueCommitted?.(committed);
    }
  }, [latestRef, onValueCommitted, values]);

  const contextValue = useMemo<RotarySelectorContextValue>(
    () => ({
      allowWheel,
      angles,
      change,
      commit,
      disabled,
      dragDirection,
      index,
      labelId,
      latestRef,
      reset: () => {
        change(resetIndex, { reason: "reset" });
        commit();
      },
      stepAngle,
      valueText: count > 0 ? format(selected) : "",
      valueWidth: Math.max(0, ...values.map((item) => format(item).length)),
      wraps: Math.abs(sweep - FULL_TURN) <= WRAP_TOLERANCE,
    }),
    [
      allowWheel,
      angles,
      change,
      commit,
      count,
      disabled,
      dragDirection,
      format,
      index,
      labelId,
      latestRef,
      resetIndex,
      selected,
      stepAngle,
      sweep,
      values,
    ]
  );

  return (
    <RotarySelectorContext.Provider value={contextValue}>
      <div
        className={cn(rotarySelectorVariants({ size }), className)}
        data-disabled={disabled ? "" : undefined}
        data-size={size}
        data-slot="rotary-selector"
        {...props}
      >
        {children({ position, positions, value: selected })}
      </div>
    </RotarySelectorContext.Provider>
  );
};

export { rotarySelectorVariants };
