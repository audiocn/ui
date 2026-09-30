"use client";

import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ComponentProps, CSSProperties, Ref } from "react";

import { ClipIndicator } from "@/components/ui/clip-indicator";
import type { ClipIndicatorProps } from "@/components/ui/clip-indicator";
import { DbReadout } from "@/components/ui/db-readout";
import type { DbReadoutProps } from "@/components/ui/db-readout";
import { DbScale } from "@/components/ui/db-scale";
import type { DbScaleProps } from "@/components/ui/db-scale";
import { useAudioConfig } from "@/hooks/use-audio-config";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { createBallistics, resolveBallistics } from "@/lib/audio/ballistics";
import type { Ballistics, BallisticsInput } from "@/lib/audio/ballistics";
import {
  DEFAULT_MAX_DB,
  DEFAULT_MIN_DB,
  formatDb,
  SILENCE_DB,
} from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import { resolveTaper } from "@/lib/audio/taper";
import type { TaperInput } from "@/lib/audio/taper";
import type {
  ChannelLevel,
  FrameSource,
  MeterFrame,
  MeterZone,
  Orientation,
  Taper,
} from "@/lib/audio/types";
import {
  CLIP_HOLD_MS,
  CLIP_THRESHOLD_DB,
  DEFAULT_ZONES,
  zoneForDb,
} from "@/lib/audio/zones";
import { cn } from "@/lib/utils";

const ARIA_INTERVAL_MS = 250;
const REDUCED_MOTION_INTERVAL_MS = 250;
const DEFAULT_SEGMENTS = 24;
const POSITION_EPSILON = 0.0005;

export type LevelMeterVariant = "solid" | "segmented" | "gradient";

export interface LevelMeterActions {
  /** Paint a frame directly, for callers that own their own frame loop. */
  paint: (frame: MeterFrame) => void;
  /** Drop the level and the peak hold to silence. */
  reset: () => void;
}

interface LevelMeterContextValue {
  orientation: Orientation;
  variant: LevelMeterVariant;
  segments: number;
  minDb: number;
  maxDb: number;
  taper: Taper;
  zoneFill: string;
  segmentMask: string | undefined;
  frames: FrameSource<MeterFrame>;
  registerChannel: (index: number, element: HTMLElement | null) => void;
}

const LevelMeterContext = createContext<LevelMeterContextValue | null>(null);

const useLevelMeter = (part: string) => {
  const context = useContext(LevelMeterContext);
  if (!context) {
    throw new Error(`${part} must be used inside LevelMeter.`);
  }
  return context;
};

const buildZoneFill = (
  zones: MeterZone[],
  taper: Taper,
  orientation: Orientation,
  variant: LevelMeterVariant
) => {
  const direction = orientation === "horizontal" ? "to right" : "to top";
  const sorted = zones.toSorted((a, b) => a.fromDb - b.fromDb);
  const starts = sorted.map((zone) => taper.toPosition(zone.fromDb) * 100);

  if (variant === "gradient") {
    const stops = sorted.map(
      (zone, index) => `var(--meter-${zone.zone}) ${starts[index] ?? 0}%`
    );
    const last = sorted.at(-1);
    return `linear-gradient(${direction}, ${stops.join(", ")}, var(--meter-${last?.zone ?? "ok"}) 100%)`;
  }

  const stops = sorted.map((zone, index) => {
    const start = starts[index] ?? 0;
    const end = starts[index + 1] ?? 100;
    return `var(--meter-${zone.zone}) ${start}% ${end}%`;
  });
  return `linear-gradient(${direction}, ${stops.join(", ")})`;
};

const buildSegmentMask = (orientation: Orientation, segments: number) => {
  const direction = orientation === "horizontal" ? "to right" : "to top";
  const size = `calc(100% / ${segments})`;
  return `repeating-linear-gradient(${direction}, black 0 calc(${size} - 2px), transparent calc(${size} - 2px) ${size})`;
};

const levelMeterVariants = cva(
  "group/level-meter flex gap-2 [--meter-gap:0.25rem] data-dimmed:opacity-50",
  {
    defaultVariants: {
      orientation: "horizontal",
      size: "default",
    },
    variants: {
      orientation: {
        horizontal: "w-full flex-row items-center",
        vertical: "min-h-32 flex-col items-center",
      },
      size: {
        default: "[--meter-thickness:0.5rem]",
        lg: "[--meter-thickness:0.75rem]",
        sm: "[--meter-thickness:0.25rem]",
      },
    },
  }
);

export interface LevelMeterProps
  extends
    ComponentProps<"div">,
    Omit<VariantProps<typeof levelMeterVariants>, "orientation"> {
  /** A meter source; the meter subscribes and paints itself without re-rendering React. */
  source?: FrameSource<MeterFrame> | null;
  /** Mono peak level in dBFS, for declarative use. */
  peakDb?: number;
  /** Mono RMS level in dBFS, for declarative use. */
  rmsDb?: number;
  /** Levels per channel, for declarative multi-channel use. */
  channels?: ChannelLevel[];
  /** Tracks to render before data arrives. Default 1. */
  channelCount?: number;
  /** Bottom of the displayed range. Default −60. */
  minDb?: number;
  /** Top of the displayed range. Default 0. */
  maxDb?: number;
  /** Colour zones. Default ok / warn from −20 / clip from −9. */
  zones?: MeterZone[];
  /** How the meter moves. Default `peak`. */
  ballistics?: BallisticsInput;
  /** Scale law. Default `linear`. */
  taper?: TaperInput;
  orientation?: Orientation;
  /** `segmented` is an LED ladder. Default `solid`. */
  variant?: LevelMeterVariant;
  /** Segments for the `segmented` variant. Default 24. */
  segments?: number;
  actionsRef?: Ref<LevelMeterActions>;
}

interface ChannelState {
  element: HTMLElement;
  peak: Ballistics;
  rms: Ballistics;
  level: number;
  rmsLevel: number;
  hold: number;
  zone: string;
  active: boolean;
}

const silentLevel: ChannelLevel = { peakDb: SILENCE_DB };

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
  return `${peakDb ?? SILENCE_DB}:${rmsDb}`;
};

const parseNumber = (text: string | undefined): number | undefined => {
  const value = Number(text);
  return Number.isNaN(value) ? undefined : value;
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

export const LevelMeter = ({
  source,
  peakDb,
  rmsDb,
  channels,
  channelCount: channelCountProp,
  minDb: minDbProp,
  maxDb: maxDbProp,
  zones: zonesProp,
  ballistics: ballisticsProp,
  taper = "linear",
  orientation: orientationProp,
  variant = "solid",
  segments = DEFAULT_SEGMENTS,
  size: sizeProp,
  actionsRef,
  className,
  children,
  ref,
  ...props
}: LevelMeterProps) => {
  const config = useAudioConfig();
  const orientation = orientationProp ?? config.orientation ?? "horizontal";
  const size = sizeProp ?? config.size ?? "default";
  const minDb = minDbProp ?? config.minDb ?? DEFAULT_MIN_DB;
  const maxDb = maxDbProp ?? config.maxDb ?? DEFAULT_MAX_DB;
  const zones = zonesProp ?? config.zones ?? DEFAULT_ZONES;
  const ballistics = ballisticsProp ?? config.ballistics ?? "peak";
  const reducedMotion = useReducedMotion();

  const [frames] = useState(() => createFrameEmitter<MeterFrame>());
  const latestRef = useRef<MeterFrame | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const visibleRef = useRef(true);
  const channelsRef = useRef(new Map<number, HTMLElement>());
  const [channelCount, setChannelCount] = useState(
    channels?.length ?? channelCountProp ?? 1
  );
  const countRef = useRef(channelCount);

  const accept = useCallback(
    (frame: MeterFrame) => {
      latestRef.current = frame;
      frames.emit(frame);
      if (
        frame.channels.length > 0 &&
        frame.channels.length !== countRef.current
      ) {
        countRef.current = frame.channels.length;
        setChannelCount(frame.channels.length);
      }
    },
    [frames]
  );

  useFrameSource(source, accept);

  const declarativeKey = serializeLevels(channels, peakDb, rmsDb);

  useEffect(() => {
    const frame = parseLevels(declarativeKey);
    if (frame) {
      accept(frame);
    }
  }, [accept, declarativeKey]);

  const taperFn = useMemo(
    () => resolveTaper(taper, minDb, maxDb),
    [taper, minDb, maxDb]
  );

  const registerChannel = useCallback(
    (index: number, element: HTMLElement | null) => {
      if (element) {
        channelsRef.current.set(index, element);
      } else {
        channelsRef.current.delete(index);
      }
    },
    []
  );

  const ballisticsKey = JSON.stringify(
    typeof ballistics === "string" ? ballistics : resolveBallistics(ballistics)
  );

  useEffect(() => {
    const options: BallisticsInput = reducedMotion
      ? "instant"
      : (JSON.parse(ballisticsKey) as BallisticsInput);
    const states = new Map<number, ChannelState>();
    let clipUntil = 0;
    let clippingShown = false;
    let lastAriaMs = 0;
    let lastPaintMs = 0;

    const stateFor = (index: number, element: HTMLElement) => {
      const existing = states.get(index);
      if (existing && existing.element === element) {
        return existing;
      }
      const created: ChannelState = {
        active: false,
        element,
        hold: -1,
        level: -1,
        peak: createBallistics(options),
        rms: createBallistics({ ...resolveBallistics(options), peakHoldMs: 0 }),
        rmsLevel: -1,
        zone: "",
      };
      states.set(index, created);
      return created;
    };

    const write = (
      element: HTMLElement,
      property: string,
      previous: number,
      next: number
    ) => {
      if (Math.abs(previous - next) > POSITION_EPSILON) {
        element.style.setProperty(property, next.toFixed(4));
        return next;
      }
      return previous;
    };

    const tick = (nowMs: number) => {
      if (!visibleRef.current) {
        return;
      }
      if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
        return;
      }
      lastPaintMs = nowMs;
      const frame = latestRef.current;
      let loudest = SILENCE_DB;

      for (const [index, element] of channelsRef.current) {
        const input = frame?.channels[index] ?? silentLevel;
        const state = stateFor(index, element);
        const peak = state.peak.step(input.peakDb, nowMs);
        const rms = state.rms.step(input.rmsDb ?? input.peakDb, nowMs);
        loudest = Math.max(loudest, peak.db);
        if (input.peakDb >= CLIP_THRESHOLD_DB) {
          clipUntil = nowMs + CLIP_HOLD_MS;
        }

        state.level = write(
          element,
          "--meter-level",
          state.level,
          taperFn.toPosition(peak.db)
        );
        state.rmsLevel = write(
          element,
          "--meter-rms",
          state.rmsLevel,
          taperFn.toPosition(rms.db)
        );
        state.hold = write(
          element,
          "--meter-hold",
          state.hold,
          taperFn.toPosition(peak.holdDb)
        );

        const zone = zoneForDb(peak.db, zones);
        if (zone !== state.zone) {
          state.zone = zone;
          element.dataset.zone = zone;
        }
        const active = state.level > 0;
        if (active !== state.active) {
          state.active = active;
          element.toggleAttribute("data-active", active);
        }
      }

      const root = rootRef.current;
      if (!root) {
        return;
      }
      const clipping = nowMs < clipUntil;
      if (clipping !== clippingShown) {
        clippingShown = clipping;
        root.toggleAttribute("data-clipping", clipping);
      }
      if (nowMs - lastAriaMs >= ARIA_INTERVAL_MS) {
        lastAriaMs = nowMs;
        const clamped = Math.min(maxDb, Math.max(minDb, loudest));
        root.setAttribute("aria-valuenow", clamped.toFixed(1));
        root.setAttribute(
          "aria-valuetext",
          formatDb(loudest, { floorDb: minDb })
        );
        root.dataset.zone = zoneForDb(loudest, zones);
      }
    };

    return subscribeFrame(tick);
  }, [ballisticsKey, maxDb, minDb, reducedMotion, taperFn, zones]);

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

  const zoneFill = useMemo(
    () => buildZoneFill(zones, taperFn, orientation, variant),
    [orientation, taperFn, variant, zones]
  );
  const segmentMask =
    variant === "segmented"
      ? buildSegmentMask(orientation, segments)
      : undefined;

  const contextValue = useMemo<LevelMeterContextValue>(
    () => ({
      frames,
      maxDb,
      minDb,
      orientation,
      registerChannel,
      segmentMask,
      segments,
      taper: taperFn,
      variant,
      zoneFill,
    }),
    [
      frames,
      maxDb,
      minDb,
      orientation,
      registerChannel,
      segmentMask,
      segments,
      taperFn,
      variant,
      zoneFill,
    ]
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
    <LevelMeterContext.Provider value={contextValue}>
      <div
        aria-valuemax={maxDb}
        aria-valuemin={minDb}
        aria-valuenow={minDb}
        className={cn(levelMeterVariants({ orientation, size }), className)}
        data-dimmed={config.dimmed ? "" : undefined}
        data-orientation={orientation}
        data-size={size}
        data-slot="level-meter"
        data-variant={variant}
        ref={setRootRef}
        role="meter"
        {...props}
      >
        {children ?? (
          <LevelMeterChannels>
            {Array.from({ length: channelCount }, (_, index) => (
              <LevelMeterChannel index={index} key={`channel-${index}`}>
                <LevelMeterTrack>
                  <LevelMeterBar />
                  <LevelMeterHold />
                </LevelMeterTrack>
              </LevelMeterChannel>
            ))}
          </LevelMeterChannels>
        )}
      </div>
    </LevelMeterContext.Provider>
  );
};

export const LevelMeterChannels = ({
  className,
  ...props
}: ComponentProps<"div">) => {
  const { orientation } = useLevelMeter("LevelMeterChannels");
  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 flex-1 gap-(--meter-gap)",
        orientation === "horizontal" ? "flex-col" : "h-full flex-row",
        className
      )}
      data-orientation={orientation}
      data-slot="level-meter-channels"
      {...props}
    />
  );
};

export interface LevelMeterChannelProps extends ComponentProps<"div"> {
  /** Which channel of the frames this track shows. Default 0. */
  index?: number;
}

export const LevelMeterChannel = ({
  index = 0,
  className,
  ref,
  ...props
}: LevelMeterChannelProps) => {
  const { orientation, registerChannel } = useLevelMeter("LevelMeterChannel");

  const setRef = useCallback(
    (node: HTMLDivElement | null) => {
      registerChannel(index, node);
      if (typeof ref === "function") {
        ref(node);
      } else if (ref) {
        ref.current = node;
      }
    },
    [index, ref, registerChannel]
  );

  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 [--meter-hold:0] [--meter-level:0] [--meter-rms:0]",
        orientation === "horizontal" ? "w-full" : "h-full",
        className
      )}
      data-index={index}
      data-orientation={orientation}
      data-slot="level-meter-channel"
      ref={setRef}
      {...props}
    />
  );
};

export const LevelMeterTrack = ({
  className,
  children,
  ...props
}: ComponentProps<"div">) => {
  const { orientation, segmentMask, zoneFill } =
    useLevelMeter("LevelMeterTrack");
  const horizontal = orientation === "horizontal";

  return (
    <div
      className={cn(
        "bg-muted relative overflow-hidden rounded-full",
        horizontal
          ? "h-(--meter-thickness) w-full"
          : "h-full w-(--meter-thickness)",
        className
      )}
      data-orientation={orientation}
      data-slot="level-meter-track"
      {...props}
    >
      {segmentMask ? (
        <div
          aria-hidden
          className="absolute inset-0 opacity-20"
          data-slot="level-meter-segments"
          style={{ backgroundImage: zoneFill, maskImage: segmentMask }}
        />
      ) : null}
      {children}
    </div>
  );
};

export interface LevelMeterBarProps extends ComponentProps<"div"> {
  /** Which measurement this bar shows. Layer a `rms` and a `peak` bar for a dual meter. Default `peak`. */
  measure?: "peak" | "rms";
}

export const LevelMeterBar = ({
  measure = "peak",
  className,
  style,
  ...props
}: LevelMeterBarProps) => {
  const { orientation, segmentMask, zoneFill } = useLevelMeter("LevelMeterBar");
  const horizontal = orientation === "horizontal";
  const variable =
    measure === "rms" ? "var(--meter-rms)" : "var(--meter-level)";
  const outer: CSSProperties = {
    transform: horizontal
      ? `translateX(calc((${variable} - 1) * 100%))`
      : `translateY(calc((1 - ${variable}) * 100%))`,
  };
  const inner: CSSProperties = {
    backgroundImage: zoneFill,
    maskImage: segmentMask,
    transform: horizontal
      ? `translateX(calc((1 - ${variable}) * 100%))`
      : `translateY(calc((${variable} - 1) * 100%))`,
  };

  return (
    <div
      aria-hidden
      className={cn("absolute inset-0 overflow-hidden", className)}
      data-measure={measure}
      data-slot="level-meter-bar"
      style={{ ...outer, ...style }}
      {...props}
    >
      <div
        className="absolute inset-0"
        data-slot="level-meter-fill"
        style={inner}
      />
    </div>
  );
};

export const LevelMeterHold = ({
  className,
  style,
  ...props
}: ComponentProps<"div">) => {
  const { orientation } = useLevelMeter("LevelMeterHold");
  const horizontal = orientation === "horizontal";
  const position: CSSProperties = {
    opacity: "calc(var(--meter-hold) * 50)",
    transform: horizontal
      ? "translateX(calc((var(--meter-hold) - 1) * 100%))"
      : "translateY(calc((1 - var(--meter-hold)) * 100%))",
  };

  return (
    <div
      aria-hidden
      className="pointer-events-none absolute inset-0"
      data-slot="level-meter-hold"
      style={{ ...position, ...style }}
      {...props}
    >
      <div
        className={cn(
          "bg-foreground/80 absolute",
          horizontal ? "inset-y-0 right-0 w-0.5" : "inset-x-0 top-0 h-0.5",
          className
        )}
      />
    </div>
  );
};

export type LevelMeterScaleProps = Omit<
  DbScaleProps,
  "minDb" | "maxDb" | "taper" | "orientation"
>;

export const LevelMeterScale = (props: LevelMeterScaleProps) => {
  const { maxDb, minDb, orientation, taper } = useLevelMeter("LevelMeterScale");
  return (
    <DbScale
      maxDb={maxDb}
      minDb={minDb}
      orientation={orientation}
      taper={taper}
      {...props}
    />
  );
};

export type LevelMeterValueProps = Omit<DbReadoutProps, "source" | "value">;

export const LevelMeterValue = (props: LevelMeterValueProps) => {
  const { frames, minDb } = useLevelMeter("LevelMeterValue");
  return (
    <DbReadout
      className="text-muted-foreground text-xs"
      floorDb={minDb}
      source={frames}
      {...props}
    />
  );
};

export type LevelMeterClipProps = Omit<ClipIndicatorProps, "source">;

export const LevelMeterClip = (props: LevelMeterClipProps) => {
  const { frames } = useLevelMeter("LevelMeterClip");
  return <ClipIndicator source={frames} {...props} />;
};

export { levelMeterVariants };
