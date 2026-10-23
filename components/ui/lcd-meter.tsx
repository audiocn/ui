"use client";

import { createContext, useContext, useMemo } from "react";
import type { ComponentProps, CSSProperties } from "react";

import { useLcdDisplayRowLabelId } from "@/components/ui/lcd-display";
import {
  LevelMeter,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterClip,
  LevelMeterScale,
  LevelMeterValue,
  useLevelMeterLayout,
} from "@/components/ui/level-meter";
import type {
  LevelMeterBarProps,
  LevelMeterChannelProps,
  LevelMeterClipProps,
  LevelMeterProps,
  LevelMeterScaleProps,
  LevelMeterValueProps,
} from "@/components/ui/level-meter";
import { useAudioConfig } from "@/hooks/use-audio-config";
import type { Orientation } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

/** The Level Meter's segment count, so the two meters match by default. */
const DEFAULT_SEGMENTS = 24;
const MIN_SEGMENTS = 2;

/** `bar` lights every segment up to the level; `dot` lights only the top one. */
export type LcdMeterMode = "bar" | "dot";

interface LcdMeterContextValue {
  mode: LcdMeterMode;
}

const LcdMeterContext = createContext<LcdMeterContextValue>({ mode: "bar" });

/**
 * One mask of evenly spaced segments along the track, with a gap of
 * `--lcd-meter-gap` between them. The lit, unlit and hold layers all share it,
 * so their segments line up.
 */
const buildSegmentMask = (orientation: Orientation, segments: number) => {
  const direction = orientation === "horizontal" ? "to right" : "to top";
  const size = `calc(100% / ${segments})`;
  const end = `calc(${size} - var(--lcd-meter-gap))`;
  return `repeating-linear-gradient(${direction}, #000 0 ${end}, transparent ${end} ${size})`;
};

/**
 * The inks, mixed from the LCD Display's lit pixels, with `currentColor` as the
 * fallback outside a display.
 */
const LCD_METER_INKS =
  "[--lcd-meter-dim:color-mix(in_oklab,var(--lcd-meter-ink)_40%,transparent)] [--lcd-meter-gap:var(--lcd-display-pitch,2px)] [--lcd-meter-ink:var(--lcd-display-lit,currentColor)] [--lcd-meter-unlit:color-mix(in_oklab,var(--lcd-meter-ink)_14%,transparent)]";

/**
 * The peak level, the RMS level and the hold, snapped down to whole segments,
 * so a segment is either lit or not, like the hardware. Written on the
 * channel, where the meter paints `--meter-level`, `--meter-rms` and
 * `--meter-hold`.
 */
const SNAPPED_LEVELS =
  "[--lcd-meter-hold:calc(round(down,var(--meter-hold)*var(--lcd-meter-segments),1)/var(--lcd-meter-segments))] [--lcd-meter-lit:calc(round(down,var(--meter-level)*var(--lcd-meter-segments),1)/var(--lcd-meter-segments))] [--lcd-meter-rms:calc(round(down,var(--meter-rms)*var(--lcd-meter-segments),1)/var(--lcd-meter-segments))] [--lcd-meter-step:calc(1/var(--lcd-meter-segments))]";

/** Clips a segment layer to `--lcd-meter-from`..`--lcd-meter-to` (0..1). */
const SPAN_CLIP: Record<Orientation, string> = {
  horizontal:
    "[clip-path:inset(0_calc((1_-_var(--lcd-meter-to))*100%)_0_calc(var(--lcd-meter-from)*100%))]",
  vertical:
    "[clip-path:inset(calc((1_-_var(--lcd-meter-to))*100%)_0_calc(var(--lcd-meter-from)*100%)_0)]",
};

const SEGMENT_LAYER =
  "pointer-events-none absolute inset-0 mask-(--lcd-meter-mask)";

/** The track, like the Level Meter's: the unlit segments, with bars on top. */
export const LcdMeterTrack = ({
  className,
  children,
  ...props
}: ComponentProps<"div">) => {
  const { orientation } = useLevelMeterLayout("LcdMeterTrack");
  const horizontal = orientation === "horizontal";

  return (
    <div
      className={cn(
        "relative shrink-0",
        horizontal
          ? "h-(--meter-thickness) w-full"
          : "h-full w-(--meter-thickness)",
        className
      )}
      data-orientation={orientation}
      data-slot="lcd-meter-track"
      {...props}
    >
      <div
        aria-hidden
        className={cn(SEGMENT_LAYER, "bg-(--lcd-meter-unlit)")}
        data-slot="lcd-meter-unlit"
      />
      {children}
    </div>
  );
};

/**
 * The lit segments, like the Level Meter's bar: up to the level, or only the
 * top one in `dot` mode. Layer an `rms` bar over a dimmed `peak` bar for a
 * dual meter.
 */
export const LcdMeterBar = ({
  measure = "peak",
  className,
  ...props
}: LevelMeterBarProps) => {
  const { orientation } = useLevelMeterLayout("LcdMeterBar");
  const { mode } = useContext(LcdMeterContext);

  return (
    <div
      aria-hidden
      className={cn(
        SEGMENT_LAYER,
        SPAN_CLIP[orientation],
        "bg-(--lcd-meter-ink) [--lcd-meter-to:var(--lcd-meter-bar)]",
        measure === "rms"
          ? "[--lcd-meter-bar:var(--lcd-meter-rms)]"
          : "[--lcd-meter-bar:var(--lcd-meter-lit)]",
        mode === "dot"
          ? "[--lcd-meter-from:max(0,var(--lcd-meter-bar)_-_var(--lcd-meter-step))]"
          : "[--lcd-meter-from:0]",
        className
      )}
      data-measure={measure}
      data-slot="lcd-meter-bar"
      {...props}
    />
  );
};

/** The peak-hold segment, like the Level Meter's hold. */
export const LcdMeterHold = ({
  className,
  ...props
}: ComponentProps<"div">) => {
  const { orientation } = useLevelMeterLayout("LcdMeterHold");

  return (
    <div
      aria-hidden
      className={cn(
        SEGMENT_LAYER,
        SPAN_CLIP[orientation],
        "bg-(--lcd-meter-ink) opacity-[min(1,var(--lcd-meter-hold)*var(--lcd-meter-segments))] [--lcd-meter-from:max(0,var(--lcd-meter-hold)_-_var(--lcd-meter-step))] [--lcd-meter-to:var(--lcd-meter-hold)]",
        className
      )}
      data-slot="lcd-meter-hold"
      {...props}
    />
  );
};

/**
 * One channel. With no children it renders the Level Meter's default track:
 * a peak bar and the hold, here as segments.
 */
export const LcdMeterChannel = ({
  className,
  children,
  ...props
}: LevelMeterChannelProps) => (
  <LevelMeterChannel className={cn(SNAPPED_LEVELS, className)} {...props}>
    {children ?? (
      <LcdMeterTrack>
        <LcdMeterBar />
        <LcdMeterHold />
      </LcdMeterTrack>
    )}
  </LevelMeterChannel>
);

/**
 * The tracks. With no children it renders one `LcdMeterChannel` per channel,
 * following the meter's data, like the Level Meter's default anatomy.
 */
export const LcdMeterChannels = ({
  children,
  ...props
}: ComponentProps<typeof LevelMeterChannels>) => {
  const { channelCount } = useLevelMeterLayout("LcdMeterChannels");
  return (
    <LevelMeterChannels {...props}>
      {children ??
        Array.from({ length: channelCount }, (_, index) => (
          <LcdMeterChannel index={index} key={`channel-${index}`} />
        ))}
    </LevelMeterChannels>
  );
};

export interface LcdMeterProps extends Omit<
  LevelMeterProps,
  "variant" | "segments"
> {
  /** Segments along the track. Default 24, like the Level Meter. */
  segments?: number;
  /** `bar` lights every segment up to the level, `dot` only the top one. Default `bar`. */
  mode?: LcdMeterMode;
}

/**
 * A dot-matrix bar-graph meter for the LCD Display. It is a Level Meter, so it
 * takes the same sources, values, ballistics, zones and ARIA; it draws the
 * level as whole segments in the display's ink. Inside an `LcdDisplayRow` it is
 * named by the row's label.
 */
export const LcdMeter = ({
  segments = DEFAULT_SEGMENTS,
  mode = "bar",
  orientation: orientationProp,
  className,
  style,
  children,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  ...props
}: LcdMeterProps) => {
  const config = useAudioConfig();
  const rowLabelId = useLcdDisplayRowLabelId();
  // Resolved the way the Level Meter does, so the mask matches its axis.
  const orientation = orientationProp ?? config.orientation ?? "horizontal";
  const count = Math.max(MIN_SEGMENTS, Math.round(segments));
  const context = useMemo(() => ({ mode }), [mode]);

  return (
    <LcdMeterContext.Provider value={context}>
      <LevelMeter
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy ?? (ariaLabel ? undefined : rowLabelId)}
        className={cn(
          LCD_METER_INKS,
          "min-w-0 [--meter-scale-size:1.25em] data-[orientation=horizontal]:flex-1",
          className
        )}
        data-lcd-mode={mode}
        data-segments={count}
        orientation={orientation}
        style={
          {
            "--lcd-meter-mask": buildSegmentMask(orientation, count),
            "--lcd-meter-segments": count,
            ...style,
          } as CSSProperties
        }
        variant="solid"
        {...props}
      >
        {children ?? <LcdMeterChannels />}
      </LevelMeter>
    </LcdMeterContext.Provider>
  );
};

/** A dB scale in the display's dim ink. Inherits the meter's range and taper. */
export const LcdMeterScale = ({
  className,
  ...props
}: LevelMeterScaleProps) => (
  <LevelMeterScale
    className={cn(
      "text-[0.625em] text-(color:--lcd-meter-dim) [&_[data-slot=db-scale-mark]]:bg-(--lcd-meter-dim)",
      className
    )}
    {...props}
  />
);

/**
 * The level as lit digits, updated at a readable rate. It takes the display's
 * letter spacing as `--db-readout-tracking`, so its reserved width includes the
 * spacing and the meter beside it keeps its width as the digits change.
 */
export const LcdMeterValue = ({
  className,
  ...props
}: LevelMeterValueProps) => (
  <LevelMeterValue
    className={cn(
      "font-(family-name:--lcd-display-font,var(--font-mono)) text-[0.75em] text-(color:--lcd-meter-ink) [--db-readout-tracking:0.12em]",
      className
    )}
    {...props}
  />
);

/**
 * A clip light: a dot like the Level Meter's, unlit until the signal clips,
 * then lit for the hold time. It is the Level Meter's clip indicator, so a
 * click resets it and screen readers hear "Clipping". Children replace the dot.
 */
export const LcdMeterClip = ({
  className,
  children,
  ...props
}: LevelMeterClipProps) => (
  <LevelMeterClip
    className={cn(
      "h-auto px-1 py-0.5 text-[0.75em] font-normal text-(color:--lcd-meter-unlit) uppercase hover:bg-transparent focus-visible:ring-1 focus-visible:ring-(--lcd-meter-ink) data-clipping:text-(color:--lcd-meter-ink)",
      className
    )}
    {...props}
  >
    {children ?? (
      <span
        className="size-[0.6em] shrink-0 rounded-full bg-(--lcd-meter-unlit) group-data-clipping/clip-indicator:bg-(--lcd-meter-ink)"
        data-slot="lcd-meter-clip-light"
      />
    )}
  </LevelMeterClip>
);
