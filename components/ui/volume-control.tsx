"use client";

import { mergeProps } from "@base-ui/react/merge-props";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { useRender } from "@base-ui/react/use-render";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ComponentProps } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { clamp } from "@/lib/audio/decibels";
import type { Orientation } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

const PERCENT = 100;
const LOW_LEVEL = 0.34;
const MEDIUM_LEVEL = 0.67;
const PERCEPTUAL_EXPONENT = 2;

export type VolumeLevel = "muted" | "low" | "medium" | "high";

/** Maps a volume (0..1 gain) to a slider position and back. */
const curves = {
  linear: {
    toPosition: (volume: number) => volume,
    toVolume: (position: number) => position,
  },
  perceptual: {
    toPosition: (volume: number) => volume ** (1 / PERCEPTUAL_EXPONENT),
    toVolume: (position: number) => position ** PERCEPTUAL_EXPONENT,
  },
} as const;

interface VolumeControlContextValue {
  volume: number;
  muted: boolean;
  level: VolumeLevel;
  position: number;
  step: number;
  orientation: Orientation;
  disabled: boolean;
  setPosition: (position: number) => void;
  commit: () => void;
  toggleMuted: () => void;
}

const VolumeControlContext = createContext<VolumeControlContextValue | null>(
  null
);

const useVolumeControl = (part: string) => {
  const context = useContext(VolumeControlContext);
  if (!context) {
    throw new Error(`${part} must be used inside VolumeControl.`);
  }
  return context;
};

const levelFor = (volume: number, muted: boolean): VolumeLevel => {
  if (muted || volume <= 0) {
    return "muted";
  }
  if (volume < LOW_LEVEL) {
    return "low";
  }
  if (volume < MEDIUM_LEVEL) {
    return "medium";
  }
  return "high";
};

export type VolumeControlMuteProps = useRender.ComponentProps<"button">;

export const VolumeControlMute = ({
  render,
  className,
  children,
  ...props
}: VolumeControlMuteProps) => {
  const { disabled, level, muted, toggleMuted } =
    useVolumeControl("VolumeControlMute");
  const label = muted ? "Unmute" : "Mute";

  return useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      {
        "aria-label": label,
        "aria-pressed": muted,
        children: children ?? <span className="text-xs">{label}</span>,
        className: cn(
          "text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-ring/30 inline-flex size-7 shrink-0 items-center justify-center rounded-lg transition-colors outline-none focus-visible:ring-3 disabled:pointer-events-none [&_svg:not([class*='size-'])]:size-4",
          !children && "w-auto px-2",
          className
        ),
        disabled,
        onClick: toggleMuted,
        type: "button",
      },
      props
    ),
    render,
    state: { level, muted, slot: "volume-control-mute" },
  });
};

export type VolumeControlSliderProps = Omit<
  SliderPrimitive.Root.Props<number>,
  | "value"
  | "defaultValue"
  | "onValueChange"
  | "onValueCommitted"
  | "min"
  | "max"
  | "step"
  | "orientation"
>;

export const VolumeControlSlider = ({
  className,
  ...props
}: VolumeControlSliderProps) => {
  const {
    commit,
    disabled,
    muted,
    orientation,
    position,
    setPosition,
    step,
    volume,
  } = useVolumeControl("VolumeControlSlider");
  const horizontal = orientation === "horizontal";
  const shown = muted ? 0 : position;

  return (
    <SliderPrimitive.Root
      className={cn(
        "relative flex touch-none items-center select-none",
        horizontal ? "w-full min-w-20" : "h-24 flex-col",
        className
      )}
      data-slot="volume-control-slider"
      disabled={disabled}
      max={1}
      min={0}
      onValueChange={(next) => setPosition(next)}
      onValueCommitted={commit}
      orientation={orientation}
      step={step}
      value={shown}
      {...props}
    >
      <SliderPrimitive.Control
        className={cn(
          "relative flex items-center",
          horizontal
            ? "h-(--volume-thumb-size) w-full px-[calc(var(--volume-thumb-size)/2)] before:absolute before:inset-x-0 before:-inset-y-1.5 pointer-coarse:before:-inset-y-3"
            : "h-full w-(--volume-thumb-size) flex-col py-[calc(var(--volume-thumb-size)/2)] before:absolute before:-inset-x-1.5 before:inset-y-0 pointer-coarse:before:-inset-x-3"
        )}
      >
        <SliderPrimitive.Track
          className={cn(
            "bg-input/90 relative grow rounded-full",
            horizontal
              ? "h-(--volume-track-size) w-full"
              : "h-full w-(--volume-track-size)"
          )}
          data-slot="volume-control-track"
        >
          <SliderPrimitive.Indicator
            className={cn(
              "bg-primary rounded-full",
              horizontal ? "h-full" : "w-full"
            )}
            data-slot="volume-control-range"
          />
          <SliderPrimitive.Thumb
            aria-label="Volume"
            className="bg-background ring-foreground/15 hover:ring-ring/30 focus-visible:ring-ring/40 block size-(--volume-thumb-size) shrink-0 rounded-full shadow-sm ring-1 outline-hidden transition-[box-shadow] hover:ring-4 focus-visible:ring-4"
            data-slot="volume-control-thumb"
            getAriaValueText={() =>
              muted ? "Muted" : `${Math.round(volume * PERCENT)}%`
            }
          />
        </SliderPrimitive.Track>
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
};

export const VolumeControlValue = ({
  className,
  ...props
}: ComponentProps<"span">) => {
  const { muted, position } = useVolumeControl("VolumeControlValue");
  return (
    <span
      className={cn(
        "text-muted-foreground w-9 shrink-0 text-end font-mono text-xs tabular-nums",
        className
      )}
      data-slot="volume-control-value"
      {...props}
    >
      {muted ? "0%" : `${Math.round(position * PERCENT)}%`}
    </span>
  );
};

export interface VolumeControlProps extends Omit<
  ComponentProps<"div">,
  "defaultValue" | "onChange"
> {
  /** Volume, 0..1, like `HTMLMediaElement.volume`. */
  value?: number;
  /** Default 1. */
  defaultValue?: number;
  onValueChange?: (value: number) => void;
  onValueCommitted?: (value: number) => void;
  muted?: boolean;
  defaultMuted?: boolean;
  onMutedChange?: (muted: boolean) => void;
  /** Slider step, in position. Default 0.05. */
  step?: number;
  /** How slider position maps to volume. Default `perceptual`. */
  curve?: "linear" | "perceptual";
  orientation?: Orientation;
  size?: AudioSize;
  disabled?: boolean;
}

export const VolumeControl = ({
  value: valueProp,
  defaultValue = 1,
  onValueChange,
  onValueCommitted,
  muted: mutedProp,
  defaultMuted = false,
  onMutedChange,
  step = 0.05,
  curve = "perceptual",
  orientation = "horizontal",
  size: sizeProp,
  disabled: disabledProp,
  className,
  children,
  ...props
}: VolumeControlProps) => {
  const config = useAudioConfig();
  const size = sizeProp ?? config.size ?? "default";
  const disabled = disabledProp ?? config.disabled ?? false;
  const [uncontrolledVolume, setUncontrolledVolume] = useState(() =>
    clamp(defaultValue, 0, 1)
  );
  const [uncontrolledMuted, setUncontrolledMuted] = useState(defaultMuted);
  const volume = valueProp ?? uncontrolledVolume;
  const muted = mutedProp ?? uncontrolledMuted;
  const lastAudibleRef = useRef(volume > 0 ? volume : 1);
  const mapping = curves[curve];

  const setVolume = useCallback(
    (next: number) => {
      if (next > 0) {
        lastAudibleRef.current = next;
      }
      if (valueProp === undefined) {
        setUncontrolledVolume(next);
      }
      onValueChange?.(next);
    },
    [onValueChange, valueProp]
  );

  const setMuted = useCallback(
    (next: boolean) => {
      if (mutedProp === undefined) {
        setUncontrolledMuted(next);
      }
      onMutedChange?.(next);
    },
    [mutedProp, onMutedChange]
  );

  const contextValue = useMemo<VolumeControlContextValue>(
    () => ({
      commit: () => onValueCommitted?.(volume),
      disabled,
      level: levelFor(volume, muted),
      muted,
      orientation,
      position: mapping.toPosition(volume),
      setPosition: (position: number) => {
        const next = clamp(mapping.toVolume(position), 0, 1);
        setVolume(next);
        if (muted && next > 0) {
          setMuted(false);
        }
      },
      step,
      toggleMuted: () => {
        if (muted && volume <= 0) {
          setVolume(lastAudibleRef.current);
        }
        setMuted(!muted);
      },
      volume,
    }),
    [
      disabled,
      mapping,
      muted,
      onValueCommitted,
      orientation,
      setMuted,
      setVolume,
      step,
      volume,
    ]
  );

  return (
    <VolumeControlContext.Provider value={contextValue}>
      <div
        className={cn(
          "group/volume-control flex items-center gap-2 data-disabled:opacity-50",
          orientation === "vertical" && "flex-col-reverse",
          size === "sm" &&
            "[--volume-thumb-size:0.75rem] [--volume-track-size:0.1875rem]",
          size === "default" &&
            "[--volume-thumb-size:0.875rem] [--volume-track-size:0.25rem]",
          size === "lg" &&
            "[--volume-thumb-size:1rem] [--volume-track-size:0.375rem]",
          className
        )}
        data-disabled={disabled ? "" : undefined}
        data-level={levelFor(volume, muted)}
        data-muted={muted ? "" : undefined}
        data-orientation={orientation}
        data-slot="volume-control"
        role="group"
        aria-label="Volume"
        {...props}
      >
        {children ?? (
          <>
            <VolumeControlMute />
            <VolumeControlSlider />
          </>
        )}
      </div>
    </VolumeControlContext.Provider>
  );
};
