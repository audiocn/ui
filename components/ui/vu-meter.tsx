"use client";

import { cva } from "class-variance-authority";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
  useId,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";
import type { ComponentProps, Ref, RefObject } from "react";

import { readChannel } from "@/components/ui/db-readout";
import { useAudioConfig } from "@/hooks/use-audio-config";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { useVisibility } from "@/hooks/use-visibility";
import { formatDb, SILENCE_DB } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { createNeedle, resolveNeedle } from "@/lib/audio/needle";
import type { NeedleInput } from "@/lib/audio/needle";
import { vuTaper } from "@/lib/audio/taper";
import type {
  ChannelLevel,
  FrameSource,
  MeterFrame,
  MeterZone,
  MeterZoneName,
  Taper,
} from "@/lib/audio/types";
import { zoneForDb } from "@/lib/audio/zones";
import { cn } from "@/lib/utils";

const ARIA_INTERVAL_MS = 250;
const REDUCED_MOTION_INTERVAL_MS = 250;
const POSITION_EPSILON = 0.0005;

/** The level that reads 0 VU, in dBFS (EBU R68). */
export const DEFAULT_REFERENCE_DB = -18;
const DEFAULT_MIN_VU = -10;
const DEFAULT_MAX_VU = 3;

/** The printed scale's normal range is ink; from 0 VU it is red. */
export const VU_ZONES: MeterZone[] = [
  { fromDb: Number.NEGATIVE_INFINITY, zone: "ok" },
  { fromDb: 0, zone: "clip" },
];

const DEFAULT_TICKS = [-10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
const DEFAULT_MINOR_TICKS = [-6, -4];

/** Where the needle turns: below the window, or above it. */
export type VuMeterPivot = "bottom" | "top";

interface PivotGeometry {
  /** The needle's pivot in the view box. It sits outside the window. */
  pivot: { x: number; y: number };
  /** 1 when the scale hangs below the pivot, −1 when it arcs above it. */
  direction: 1 | -1;
  /** The scale's arc. */
  radius: number;
  /** Degrees from one end of the scale to the other. */
  sweep: number;
  /** From the pivot to the centre of the numbers. */
  label: number;
  /** From the pivot to the needle's tip. */
  needle: number;
  /** The bezel lip that hides the bottom of the needle, if any. */
  cover: number;
}

/*
 * The face is a 200 × 100 view box. Every tick points away from the pivot,
 * so the scale arcs over a bottom pivot and hangs under a top one.
 */
const GEOMETRY: Record<VuMeterPivot, PivotGeometry> = {
  // The classic movement: the needle rises from under a lip in the bezel.
  bottom: {
    cover: 66,
    direction: -1,
    label: 136.5,
    needle: 131,
    pivot: { x: 100, y: 156 },
    radius: 125,
    sweep: 80,
  },
  // The needle hangs from above the window, behind the badge.
  top: {
    cover: 0,
    direction: 1,
    label: 183,
    needle: 199,
    pivot: { x: 100, y: -104 },
    radius: 170,
    sweep: 58,
  },
};

/** Turns the needle group about its pivot. Keep in step with GEOMETRY. */
const NEEDLE_TURN: Record<VuMeterPivot, string> = {
  bottom: "origin-[100px_156px] rotate-[calc((var(--vu-level)_-_0.5)_*_80deg)]",
  top: "origin-[100px_-104px] rotate-[calc((0.5_-_var(--vu-level))_*_58deg)]",
};

const VIEW_WIDTH = 200;
const VIEW_HEIGHT = 100;
const VIEW_BOX = `0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`;
/** How far past each end of the scale the pins sit, as a share of the sweep. */
const PIN = 0.04;
const REST = -PIN;
const NEEDLE_SHADOW_OFFSET = 1.8;
const NEEDLE_SHADOW_REGION = 6;
const COVER_SHADOW = 1.5;
const SCALE = {
  arc: 0.8,
  labelSize: 7,
  majorTick: 5.5,
  minorTick: 3.5,
  minorTickWidth: 0.55,
  tickWidth: 0.8,
  zoneArc: 2.4,
} as const;
const DEGREES_TO_RADIANS = Math.PI / 180;
const PRECISION = 1000;
const MINUS_SIGN = "−";

const ZONE_STROKE: Record<MeterZoneName, string> = {
  clip: "stroke-(--vu-zone-clip)",
  ok: "stroke-(--vu-zone-ok)",
  warn: "stroke-(--vu-zone-warn)",
};

const ZONE_FILL: Record<MeterZoneName, string> = {
  clip: "fill-(--vu-zone-clip)",
  ok: "fill-(--vu-zone-ok)",
  warn: "fill-(--vu-zone-warn)",
};

const round = (value: number) => Math.round(value * PRECISION) / PRECISION;

/** Degrees from the scale's centre line, positive to the right. */
const angleFor = (geometry: PivotGeometry, position: number) =>
  (position - 0.5) * geometry.sweep;

const pointAt = (geometry: PivotGeometry, angle: number, radius: number) => {
  const radians = angle * DEGREES_TO_RADIANS;
  return {
    x: round(geometry.pivot.x + radius * Math.sin(radians)),
    y: round(
      geometry.pivot.y + geometry.direction * radius * Math.cos(radians)
    ),
  };
};

/** An arc of the scale, left to right. */
const arcPath = (
  geometry: PivotGeometry,
  fromPosition: number,
  toPosition: number,
  radius: number
) => {
  const start = pointAt(geometry, angleFor(geometry, fromPosition), radius);
  const end = pointAt(geometry, angleFor(geometry, toPosition), radius);
  // Left to right is clockwise over a bottom pivot, anticlockwise under a top one.
  const clockwise = geometry.direction === -1 ? 1 : 0;
  return `M ${start.x} ${start.y} A ${radius} ${radius} 0 0 ${clockwise} ${end.x} ${end.y}`;
};

/** "+3", "0", "−7": the numbers printed on a VU scale. */
export const formatVu = (vu: number): string => {
  if (vu > 0) {
    return `+${vu}`;
  }
  if (vu < 0) {
    return `${MINUS_SIGN}${Math.abs(vu)}`;
  }
  return "0";
};

export type VuMeterVariant = "classic" | "flat";

export interface VuMeterActions {
  /** Paint a frame directly, for callers that own their own frame loop. */
  paint: (frame: MeterFrame) => void;
  /** Send silence: the needle falls back to rest. */
  reset: () => void;
}

interface VuMeterContextValue {
  variant: VuMeterVariant;
  pivot: VuMeterPivot;
  minDb: number;
  maxDb: number;
  /** Scale position for a VU reading, linear in amplitude. */
  taper: Taper;
  zones: MeterZone[];
}

const VuMeterContext = createContext<VuMeterContextValue | null>(null);

const useVuMeter = (part: string) => {
  const context = useContext(VuMeterContext);
  if (!context) {
    throw new Error(`${part} must be used inside VuMeter.`);
  }
  return context;
};

/** An id that is safe inside `url(#…)`. */
const useSvgId = () => `vu${useId().replaceAll(/[^\w-]/gu, "")}`;

const serializeLevels = (
  channels: ChannelLevel[] | undefined,
  peakDb: number | undefined,
  rmsDb: number | undefined
): string => {
  if (channels) {
    return channels.map((level) => `${level.peakDb}:${level.rmsDb}`).join("|");
  }
  if (peakDb === undefined && rmsDb === undefined) {
    return "";
  }
  return `${peakDb ?? rmsDb ?? SILENCE_DB}:${rmsDb}`;
};

const parseNumber = (text: string | undefined): number | undefined => {
  const value = Number(text);
  return text === undefined || Number.isNaN(value) ? undefined : value;
};

const parseLevels = (key: string): MeterFrame | null => {
  if (key === "") {
    return null;
  }
  return {
    channels: key.split("|").map((entry) => {
      const [peak, rms] = entry.split(":");
      return {
        peakDb: parseNumber(peak) ?? SILENCE_DB,
        rmsDb: parseNumber(rms),
      };
    }),
  };
};

/** Read on every paint, so changing it doesn't rebuild the needle. */
interface NeedleReading {
  channel: number | "max";
  maxDb: number;
  measure: "peak" | "rms";
  minDb: number;
  referenceDb: number;
  taper: Taper;
  zones: MeterZone[];
}

interface PainterOptions {
  ballistics: NeedleInput;
  latest: RefObject<MeterFrame | null>;
  reducedMotion: boolean;
  root: RefObject<HTMLElement | null>;
  visible: RefObject<boolean>;
}

/** The VU reading where the needle points, kept to the printed scale. */
const needleVu = (position: number, reading: NeedleReading) => {
  if (position <= 0) {
    return reading.minDb;
  }
  if (position >= 1) {
    return reading.maxDb;
  }
  return reading.taper.toValue(position);
};

/** Reports the VU reading where the needle points. */
const writeAria = (
  root: HTMLElement,
  position: number,
  reading: NeedleReading
) => {
  const vu = needleVu(position, reading);
  root.setAttribute("aria-valuenow", vu.toFixed(1));
  root.setAttribute("aria-valuetext", `${formatDb(vu, { unit: false })} VU`);
};

/**
 * Paints the needle outside React: steps its ballistics, writes `--vu-level`
 * and the data attributes, and throttles ARIA updates.
 */
const createNeedlePainter = (options: PainterOptions) => {
  const needle = createNeedle(
    options.reducedMotion ? "instant" : options.ballistics,
    { max: 1 + PIN, min: REST }
  );
  let level = Number.NaN;
  // Null until first painted, so a new painter always writes them.
  let zone: MeterZoneName | null = null;
  let active: boolean | null = null;
  let pinned: boolean | null = null;
  let lastAriaMs = Number.NEGATIVE_INFINITY;
  let lastPaintMs = Number.NEGATIVE_INFINITY;

  const writeState = (
    root: HTMLElement,
    position: number,
    reading: NeedleReading
  ) => {
    const nextZone = zoneForDb(needleVu(position, reading), reading.zones);
    if (nextZone !== zone) {
      zone = nextZone;
      root.dataset.zone = nextZone;
    }
    const nextActive = position > REST + POSITION_EPSILON;
    if (nextActive !== active) {
      active = nextActive;
      root.toggleAttribute("data-active", nextActive);
    }
    const nextPinned = position >= 1 + PIN - POSITION_EPSILON;
    if (nextPinned !== pinned) {
      pinned = nextPinned;
      root.toggleAttribute("data-pinned", nextPinned);
    }
  };

  return (nowMs: number, reading: NeedleReading) => {
    const root = options.root.current;
    if (!(root && options.visible.current)) {
      return;
    }
    if (
      options.reducedMotion &&
      nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS
    ) {
      return;
    }
    lastPaintMs = nowMs;

    const frame = options.latest.current;
    const db = frame
      ? readChannel(frame, reading.measure, reading.channel)
      : SILENCE_DB;
    const target = reading.taper.toPosition(db - reading.referenceDb);
    const position = needle.step(target, nowMs);

    if (!(Math.abs(position - level) <= POSITION_EPSILON)) {
      level = position;
      root.style.setProperty("--vu-level", position.toFixed(4));
    }
    writeState(root, position, reading);
    if (nowMs - lastAriaMs >= ARIA_INTERVAL_MS) {
      lastAriaMs = nowMs;
      writeAria(root, position, reading);
    }
  };
};

export const VuMeterFace = ({ className, ...props }: ComponentProps<"div">) => {
  const { variant } = useVuMeter("VuMeterFace");
  return (
    <div
      className={cn(
        "relative isolate m-[3.5cqw] aspect-[2/1] overflow-hidden rounded-sm text-(--vu-ink)",
        "after:pointer-events-none after:absolute after:inset-0 after:z-30 after:rounded-[inherit]",
        variant === "classic"
          ? "bg-[radial-gradient(110%_125%_at_50%_24%,var(--vu-face)_22%,var(--vu-face-shade)_100%)] after:bg-[linear-gradient(170deg,rgb(255_255_255/0.28),transparent_34%)] after:shadow-[inset_0_0.9cqw_2.2cqw_rgb(0_0_0/0.5),inset_0_0_0_1px_rgb(0_0_0/0.55)]"
          : "bg-(--vu-face) after:shadow-[inset_0_0_0_1px_var(--border)]",
        className
      )}
      data-slot="vu-meter-face"
      {...props}
    />
  );
};

export interface VuMeterScaleProps extends Omit<
  ComponentProps<"svg">,
  "format"
> {
  /** Numbered ticks, in VU, kept to the meter's range. Default −10, −7, −5, −3, −2, −1, 0, +1, +2, +3. */
  ticks?: number[];
  /** Unnumbered ticks, in VU. Default −6 and −4. */
  minorTicks?: number[];
  /** Text for each number. Default "+3", "0", "−7". */
  format?: (vu: number) => string;
}

/** The printed scale: an arc coloured by zone, with ticks and numbers. */
export const VuMeterScale = ({
  ticks = DEFAULT_TICKS,
  minorTicks = DEFAULT_MINOR_TICKS,
  format = formatVu,
  className,
  ...props
}: VuMeterScaleProps) => {
  const { maxDb, minDb, pivot, taper, zones } = useVuMeter("VuMeterScale");
  const geometry = GEOMETRY[pivot];
  const inRange = (vu: number) => vu >= minDb && vu <= maxDb;
  const sorted = zones.toSorted((a, b) => a.fromDb - b.fromDb);

  const arcs = sorted.map((zone, index) => {
    const from = Math.max(zone.fromDb, minDb);
    const to = Math.min(sorted[index + 1]?.fromDb ?? maxDb, maxDb);
    if (!(to > from)) {
      return null;
    }
    const thick = zone.zone !== "ok";
    // A thick band keeps its outer edge on the thin line, where the ticks start.
    const radius = thick
      ? geometry.radius + (SCALE.arc - SCALE.zoneArc) / 2
      : geometry.radius;
    return (
      <path
        className={ZONE_STROKE[zone.zone]}
        d={arcPath(
          geometry,
          taper.toPosition(from),
          taper.toPosition(to),
          radius
        )}
        data-slot="vu-meter-arc"
        data-zone={zone.zone}
        fill="none"
        key={`${zone.zone}-${zone.fromDb}`}
        strokeWidth={thick ? SCALE.zoneArc : SCALE.arc}
      />
    );
  });

  const tickLine = (vu: number, major: boolean) => {
    const angle = angleFor(geometry, taper.toPosition(vu));
    const zone = zoneForDb(vu, zones);
    const inner = pointAt(geometry, angle, geometry.radius - SCALE.arc / 2);
    const outer = pointAt(
      geometry,
      angle,
      geometry.radius + (major ? SCALE.majorTick : SCALE.minorTick)
    );
    return (
      <line
        className={cn(ZONE_STROKE[zone], !major && "opacity-70")}
        data-major={major ? "" : undefined}
        data-slot="vu-meter-tick"
        data-zone={zone}
        key={`tick-${vu}`}
        strokeWidth={major ? SCALE.tickWidth : SCALE.minorTickWidth}
        x1={inner.x}
        x2={outer.x}
        y1={inner.y}
        y2={outer.y}
      />
    );
  };

  const labels = ticks.filter(inRange).map((vu) => {
    const { x, y } = pointAt(
      geometry,
      angleFor(geometry, taper.toPosition(vu)),
      geometry.label
    );
    const zone = zoneForDb(vu, zones);
    return (
      <text
        className={ZONE_FILL[zone]}
        data-slot="vu-meter-scale-label"
        data-zone={zone}
        dominantBaseline="central"
        fontSize={SCALE.labelSize}
        key={`label-${vu}`}
        textAnchor="middle"
        x={x}
        y={y}
      >
        {format(vu)}
      </text>
    );
  });

  return (
    <svg
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 size-full",
        className
      )}
      data-pivot={pivot}
      data-slot="vu-meter-scale"
      viewBox={VIEW_BOX}
      {...props}
    >
      {arcs}
      {minorTicks.filter(inRange).map((vu) => tickLine(vu, false))}
      {ticks.filter(inRange).map((vu) => tickLine(vu, true))}
      {labels}
    </svg>
  );
};

/**
 * The needle and the shadow the backlight casts beside it. Over a bottom
 * pivot it also draws the bezel lip the needle rises from.
 */
export const VuMeterNeedle = ({
  className,
  ...props
}: ComponentProps<"svg">) => {
  const { pivot } = useVuMeter("VuMeterNeedle");
  const geometry = GEOMETRY[pivot];
  const id = useSvgId();
  const { x, y } = geometry.pivot;
  const tipY = y + geometry.direction * geometry.needle;
  return (
    <svg
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 z-10 size-full",
        className
      )}
      data-pivot={pivot}
      data-slot="vu-meter-needle"
      viewBox={VIEW_BOX}
      {...props}
    >
      <defs>
        {/* User units: a vertical line has no width for a relative region. */}
        <filter
          filterUnits="userSpaceOnUse"
          height={geometry.needle}
          id={`${id}-shadow`}
          width={NEEDLE_SHADOW_REGION * 2}
          x={x - NEEDLE_SHADOW_REGION}
          y={Math.min(y, tipY)}
        >
          <feGaussianBlur stdDeviation={0.55} />
        </filter>
        <filter
          filterUnits="userSpaceOnUse"
          height={VIEW_HEIGHT}
          id={`${id}-cover`}
          width={VIEW_WIDTH}
          x={0}
          y={0}
        >
          <feGaussianBlur stdDeviation={1.2} />
        </filter>
      </defs>
      <g className={cn("[transform-box:view-box]", NEEDLE_TURN[pivot])}>
        <line
          className="stroke-(--vu-needle) group-data-[variant=flat]/vu-meter:hidden"
          data-slot="vu-meter-needle-shadow"
          filter={`url(#${id}-shadow)`}
          opacity={0.38}
          strokeWidth={0.9}
          x1={x - NEEDLE_SHADOW_OFFSET}
          x2={x - NEEDLE_SHADOW_OFFSET}
          y1={y}
          y2={tipY}
        />
        <line
          className="stroke-(--vu-needle)"
          data-slot="vu-meter-needle-line"
          strokeLinecap="round"
          strokeWidth={0.8}
          x1={x}
          x2={x}
          y1={y}
          y2={tipY}
        />
      </g>
      {geometry.cover > 0 ? (
        <g data-slot="vu-meter-cover">
          <circle
            className="fill-(--vu-bezel) group-data-[variant=flat]/vu-meter:hidden"
            cx={x}
            cy={y}
            filter={`url(#${id}-cover)`}
            opacity={0.55}
            r={geometry.cover + COVER_SHADOW}
          />
          <circle
            className="fill-(--vu-bezel) stroke-(--vu-face)/25"
            cx={x}
            cy={y}
            r={geometry.cover}
            strokeWidth={0.6}
          />
        </g>
      ) : null}
    </svg>
  );
};

/**
 * A mark on the face. Over a bottom pivot it is printed under the needle;
 * under a top pivot it sits at the top and covers the needle's end.
 */
export const VuMeterBadge = ({
  className,
  ...props
}: ComponentProps<"div">) => {
  const { pivot } = useVuMeter("VuMeterBadge");
  return (
    <div
      aria-hidden
      className={cn(
        "absolute left-1/2 flex -translate-x-1/2 items-center justify-center",
        pivot === "top" ? "top-[7%] z-20" : "top-[60%] -translate-y-1/2",
        className
      )}
      data-slot="vu-meter-badge"
      {...props}
    />
  );
};

/** Text at the bottom left of the face, such as "VU". */
export const VuMeterLegend = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn(
      "absolute bottom-[7%] left-[4.5%] text-[3.2cqw] leading-none tracking-tight",
      className
    )}
    data-slot="vu-meter-legend"
    {...props}
  />
);

/** Text at the bottom right of the face, such as a channel letter. */
export const VuMeterLabel = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn(
      "absolute right-[5%] bottom-[7%] text-[3.4cqw] leading-none",
      className
    )}
    data-slot="vu-meter-label"
    {...props}
  />
);

const vuMeterVariants = cva(
  // A swinging needle must not become the page's scroll anchor.
  "group/vu-meter @container/vu-meter relative w-full rounded-lg [--vu-level:-0.04] [--vu-zone-ok:var(--vu-ink)] [--vu-zone-warn:var(--meter-warn)] [overflow-anchor:none] data-dimmed:opacity-50",
  {
    defaultVariants: {
      variant: "classic",
    },
    variants: {
      variant: {
        classic:
          "bg-(--vu-bezel) bg-[linear-gradient(to_bottom,rgb(255_255_255/0.09),transparent_45%)] shadow-[0_1px_2px_rgb(0_0_0/0.3),0_6px_16px_-6px_rgb(0_0_0/0.45),inset_0_1px_0_rgb(255_255_255/0.08)] [--vu-bezel:oklch(0.2_0.004_264)] [--vu-face-shade:oklch(0.72_0.006_264)] [--vu-face:oklch(0.975_0.003_264)] [--vu-ink:oklch(0.33_0.006_264)] [--vu-needle:oklch(0.2_0.004_264)] [--vu-zone-clip:oklch(0.64_0.19_24)]",
        flat: "bg-card border shadow-xs [--vu-bezel:var(--border)] [--vu-face-shade:var(--muted)] [--vu-face:var(--muted)] [--vu-ink:var(--foreground)] [--vu-needle:var(--foreground)] [--vu-zone-clip:var(--meter-clip)]",
      },
    },
  }
);

export interface VuMeterProps extends ComponentProps<"div"> {
  /** A meter source; the meter subscribes and paints itself without re-rendering React. */
  source?: FrameSource<MeterFrame> | null;
  /** Mono peak level in dBFS, for declarative use. */
  peakDb?: number;
  /** Mono RMS level in dBFS, for declarative use. */
  rmsDb?: number;
  /** Levels per channel, for declarative multi-channel use. */
  channels?: ChannelLevel[];
  /** Which channel the needle shows, or the loudest. Default `max`. */
  channel?: number | "max";
  /** Which reading moves the needle. RMS falls back to peak. Default `rms`. */
  measure?: "rms" | "peak";
  /** The level in dBFS that reads 0 VU. Default −18 (EBU R68); SMPTE uses −20. */
  referenceDb?: number;
  /** Left end of the printed scale, in VU. Default −10. */
  minDb?: number;
  /** Right end of the printed scale, in VU. Default +3. */
  maxDb?: number;
  /** Colour zones, in VU. Default ink, then red from 0 VU. */
  zones?: MeterZone[];
  /** How the needle moves. Default `vu`: 99% in 300 ms with 1.5% overshoot. */
  ballistics?: NeedleInput;
  /** `classic` is a backlit face in a dark bezel; `flat` follows the theme. Default `classic`. */
  variant?: VuMeterVariant;
  /** Where the needle turns: `bottom`, as on most meters, or `top`, hanging from above. Default `bottom`. */
  pivot?: VuMeterPivot;
  actionsRef?: Ref<VuMeterActions>;
}

export const VuMeter = ({
  source,
  peakDb,
  rmsDb,
  channels,
  channel = "max",
  measure = "rms",
  referenceDb = DEFAULT_REFERENCE_DB,
  minDb = DEFAULT_MIN_VU,
  maxDb = DEFAULT_MAX_VU,
  zones = VU_ZONES,
  ballistics = "vu",
  variant = "classic",
  pivot = "bottom",
  actionsRef,
  className,
  children,
  ref,
  ...props
}: VuMeterProps) => {
  const config = useAudioConfig();
  const reducedMotion = useReducedMotion();
  const latestRef = useRef<MeterFrame | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const visibleRef = useVisibility(rootRef);

  const accept = useCallback((frame: MeterFrame) => {
    latestRef.current = frame;
  }, []);

  useFrameSource(source, accept);

  const declarativeKey = serializeLevels(channels, peakDb, rmsDb);
  useEffect(() => {
    const frame = parseLevels(declarativeKey);
    if (frame) {
      accept(frame);
    }
  }, [accept, declarativeKey]);

  const taper = useMemo(() => vuTaper(minDb, maxDb), [minDb, maxDb]);

  // Scale and calibration changes reach the painter on its next frame
  // instead of rebuilding it and throwing the needle back to rest.
  const reading = useMemo<NeedleReading>(
    () => ({ channel, maxDb, measure, minDb, referenceDb, taper, zones }),
    [channel, maxDb, measure, minDb, referenceDb, taper, zones]
  );
  const readReading = useEffectEvent(() => reading);

  const ballisticsKey = JSON.stringify(
    typeof ballistics === "string" ? ballistics : resolveNeedle(ballistics)
  );

  useEffect(() => {
    const paint = createNeedlePainter({
      ballistics: JSON.parse(ballisticsKey) as NeedleInput,
      latest: latestRef,
      reducedMotion,
      root: rootRef,
      visible: visibleRef,
    });
    return subscribeFrame((nowMs) => {
      paint(nowMs, readReading());
    });
  }, [ballisticsKey, reducedMotion, visibleRef]);

  useImperativeHandle(
    actionsRef,
    () => ({
      paint: accept,
      reset: () => {
        accept({ channels: [] });
      },
    }),
    [accept]
  );

  const contextValue = useMemo<VuMeterContextValue>(
    () => ({ maxDb, minDb, pivot, taper, variant, zones }),
    [maxDb, minDb, pivot, taper, variant, zones]
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

  return (
    <VuMeterContext.Provider value={contextValue}>
      <div
        aria-valuemax={maxDb}
        aria-valuemin={minDb}
        aria-valuenow={minDb}
        className={cn(vuMeterVariants({ variant }), className)}
        data-dimmed={config.dimmed ? "" : undefined}
        data-pivot={pivot}
        data-slot="vu-meter"
        data-variant={variant}
        ref={setRootRef}
        role="meter"
        {...props}
      >
        {children ?? (
          <VuMeterFace>
            <VuMeterScale />
            <VuMeterNeedle />
            <VuMeterLegend>VU</VuMeterLegend>
          </VuMeterFace>
        )}
      </div>
    </VuMeterContext.Provider>
  );
};

export { vuMeterVariants };
