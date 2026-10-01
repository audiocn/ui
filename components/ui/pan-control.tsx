"use client";

import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cva } from "class-variance-authority";
import { useState } from "react";
import type { CSSProperties } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { clamp } from "@/lib/audio/decibels";
import { cn } from "@/lib/utils";

const PERCENT = 100;
const DETENT_RANGE = 0.08;

/** "L30", "C", "R30". */
export const formatPan = (value: number): string => {
  const amount = Math.round(Math.abs(value) * PERCENT);
  if (amount === 0) {
    return "C";
  }
  return `${value < 0 ? "L" : "R"}${amount}`;
};

const CENTER_TEXT = /^c(?:enter|entre)?$/iu;
const SIDE_PREFIX = /^[LR]/iu;

/** Reads "L30", "R15", "C" or a number from −100 to 100; the inverse of formatPan. */
export const parsePan = (text: string): number | null => {
  const trimmed = text.trim().replaceAll("\u2212", "-");
  if (CENTER_TEXT.test(trimmed)) {
    return 0;
  }
  const side = SIDE_PREFIX.test(trimmed) ? trimmed[0]?.toUpperCase() : null;
  const digits = (side ? trimmed.slice(1) : trimmed).trim();
  const amount = Number(digits) / PERCENT;
  if (digits === "" || Number.isNaN(amount)) {
    return null;
  }
  return side === "L" ? -Math.abs(amount) : amount;
};

export const describePan = (value: number): string => {
  const amount = Math.round(Math.abs(value) * PERCENT);
  if (amount === 0) {
    return "Center";
  }
  return `${amount}% ${value < 0 ? "left" : "right"}`;
};

const panControlVariants = cva(
  "group/pan-control relative flex w-full touch-none items-center select-none data-disabled:opacity-50",
  {
    defaultVariants: { size: "default" },
    variants: {
      size: {
        default: "[--pan-thumb-size:0.875rem] [--pan-track-size:0.25rem]",
        lg: "[--pan-thumb-size:1rem] [--pan-track-size:0.375rem]",
        sm: "[--pan-thumb-size:0.75rem] [--pan-track-size:0.1875rem]",
      },
    },
  }
);

export interface PanControlProps extends Omit<
  SliderPrimitive.Root.Props<number>,
  | "value"
  | "defaultValue"
  | "onValueChange"
  | "onValueCommitted"
  | "min"
  | "max"
  | "orientation"
  | "format"
> {
  /** −1 (left) to 1 (right). */
  value?: number;
  /** Default 0. */
  defaultValue?: number;
  onValueChange?: (value: number) => void;
  onValueCommitted?: (value: number) => void;
  /** Default 0.05. */
  step?: number;
  /** Snap to the centre while dragging near it. Default true. */
  detent?: boolean;
  /** Default: "L30", "C", "R30". */
  format?: (value: number) => string;
  size?: AudioSize;
}

export const PanControl = ({
  value: valueProp,
  defaultValue = 0,
  onValueChange,
  onValueCommitted,
  step = 0.05,
  largeStep = 0.25,
  detent = true,
  format = formatPan,
  size: sizeProp,
  disabled: disabledProp,
  className,
  ...props
}: PanControlProps) => {
  const config = useAudioConfig();
  const size = sizeProp ?? config.size ?? "default";
  const disabled = disabledProp ?? config.disabled ?? false;
  const [uncontrolled, setUncontrolled] = useState(() =>
    clamp(defaultValue, -1, 1)
  );
  const value = valueProp ?? uncontrolled;

  const setValue = (next: number) => {
    if (valueProp === undefined) {
      setUncontrolled(next);
    }
    onValueChange?.(next);
  };

  const start = Math.min(0, value);
  const end = Math.max(0, value);

  return (
    <SliderPrimitive.Root
      className={cn(panControlVariants({ size }), className)}
      data-centered={value === 0 ? "" : undefined}
      data-size={size}
      data-slot="pan-control"
      disabled={disabled}
      largeStep={largeStep}
      max={1}
      min={-1}
      onValueChange={(next, details) => {
        const snapped =
          detent && details.reason === "drag" && Math.abs(next) < DETENT_RANGE
            ? 0
            : next;
        setValue(snapped);
      }}
      onValueCommitted={(next) => {
        onValueCommitted?.(detent && Math.abs(next) < DETENT_RANGE ? 0 : next);
      }}
      step={step}
      value={value}
      {...props}
    >
      <SliderPrimitive.Control className="relative flex h-(--pan-thumb-size) w-full items-center px-[calc(var(--pan-thumb-size)/2)] before:absolute before:inset-x-0 before:-inset-y-1.5 pointer-coarse:before:-inset-y-3">
        <SliderPrimitive.Track
          className="bg-input/90 relative h-(--pan-track-size) w-full grow rounded-full"
          data-slot="pan-control-track"
        >
          <span
            aria-hidden
            className="bg-border absolute top-1/2 left-1/2 h-[calc(var(--pan-track-size)*3)] w-px -translate-x-1/2 -translate-y-1/2"
            data-slot="pan-control-center"
          />
          <div
            className="bg-primary absolute inset-y-0 left-(--pan-range-start) w-(--pan-range-size) rounded-full"
            data-slot="pan-control-range"
            style={
              {
                "--pan-range-size": `${((end - start) / 2) * PERCENT}%`,
                "--pan-range-start": `${((start + 1) / 2) * PERCENT}%`,
              } as CSSProperties
            }
          />
          <SliderPrimitive.Thumb
            aria-label="Pan"
            className="bg-background ring-foreground/15 hover:ring-ring/30 focus-visible:ring-ring/40 data-dragging:ring-ring/30 block size-(--pan-thumb-size) shrink-0 rounded-full shadow-sm ring-1 outline-hidden transition-[box-shadow] hover:ring-4 focus-visible:ring-4 data-dragging:ring-4"
            data-slot="pan-control-thumb"
            getAriaValueText={() => describePan(value)}
            onDoubleClick={() => {
              setValue(0);
              onValueCommitted?.(0);
            }}
          />
        </SliderPrimitive.Track>
      </SliderPrimitive.Control>
      <span className="sr-only">{format(value)}</span>
    </SliderPrimitive.Root>
  );
};

export { panControlVariants };
