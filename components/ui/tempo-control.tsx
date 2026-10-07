"use client";

import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { mergeProps } from "@base-ui/react/merge-props";
import { NumberField as NumberFieldPrimitive } from "@base-ui/react/number-field";
import { cva } from "class-variance-authority";
import { createContext, useContext, useMemo, useState } from "react";
import type { ComponentProps } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import type { AudioSize } from "@/hooks/use-audio-config";
import { useTapTempo } from "@/hooks/use-tap-tempo";
import type { TempoChangeDetails } from "@/hooks/use-tap-tempo";
import { clamp } from "@/lib/audio/decibels";
import { cn } from "@/lib/utils";

const TEMPO_FORMAT: Intl.NumberFormatOptions = {
  maximumFractionDigits: 0,
  useGrouping: false,
};

interface TempoControlContextValue {
  disabled: boolean;
  readOnly: boolean;
  resetTaps: () => void;
  tap: (event?: Event) => void;
}

const TempoContext = createContext<TempoControlContextValue | null>(null);

const useTempoControl = (): TempoControlContextValue => {
  const context = useContext(TempoContext);
  if (!context) {
    throw new Error("useTempoControl must be used inside TempoControl.");
  }

  return context;
};

export const tempoControlVariants = cva(
  "group/tempo-control inline-flex items-center gap-2 data-disabled:opacity-50",
  {
    defaultVariants: { size: "default" },
    variants: {
      size: {
        default: "[--tempo-height:--spacing(8)]",
        lg: "[--tempo-height:--spacing(9)]",
        sm: "[--tempo-height:--spacing(7)]",
      },
    },
  }
);

export interface TempoControlProps extends Omit<
  ComponentProps<"div">,
  "defaultValue" | "onChange"
> {
  value?: number;
  defaultValue?: number;
  onValueChange?: (value: number, details: TempoChangeDetails) => void;
  onValueCommitted?: (value: number) => void;
  /** BPM bounds. Defaults 30 and 600. Both must be positive. */
  min?: number;
  max?: number;
  /** Shift+arrow increment. Default 10. Must be a positive integer. */
  largeStep?: number;
  /** Idle milliseconds before the next tap starts a new sequence. Default 2000. */
  tapTimeout?: number;
  size?: AudioSize;
  disabled?: boolean;
  readOnly?: boolean;
  /** Name of the numeric form field. */
  name?: string;
}

export const TempoControlInput = ({
  className,
  onChange,
  ...props
}: NumberFieldPrimitive.Input.Props) => {
  const { disabled, readOnly, resetTaps } = useTempoControl();

  return (
    <NumberFieldPrimitive.Group
      className="bg-input/50 focus-within:ring-ring/30 flex h-(--tempo-height) items-center rounded-lg focus-within:ring-3"
      data-slot="tempo-control-input-group"
    >
      <NumberFieldPrimitive.Input
        aria-label="Tempo in beats per minute"
        className={cn(
          "h-full w-11 bg-transparent px-2 text-end font-mono text-sm tabular-nums outline-hidden",
          className
        )}
        data-slot="tempo-control-input"
        {...props}
        {...mergeProps<"input">(
          {
            onChange: (event) => {
              if (event.defaultPrevented || disabled || readOnly) {
                return;
              }

              resetTaps();
            },
          },
          { onChange }
        )}
      />
      <span
        aria-hidden="true"
        className="text-muted-foreground pr-2 text-xs"
        data-slot="tempo-control-unit"
      >
        BPM
      </span>
    </NumberFieldPrimitive.Group>
  );
};

export const TempoControlTap = ({
  className,
  children,
  disabled: disabledProp,
  onClick,
  onKeyDown,
  ...props
}: ButtonPrimitive.Props) => {
  const { disabled, readOnly, tap } = useTempoControl();

  return (
    <ButtonPrimitive
      aria-label="Tap tempo"
      className={cn(
        "border-border bg-background hover:bg-muted focus-visible:ring-ring/30 active:bg-muted inline-flex h-(--tempo-height) shrink-0 touch-manipulation items-center justify-center gap-1.5 rounded-lg border px-3 text-sm font-medium transition-[background-color,box-shadow,transform,scale] outline-none select-none focus-visible:ring-3 active:scale-[0.98] disabled:pointer-events-none disabled:opacity-50 motion-reduce:transition-none motion-reduce:active:scale-100",
        className
      )}
      data-slot="tempo-control-tap"
      disabled={disabled || readOnly || disabledProp}
      type="button"
      {...props}
      {...mergeProps<"button">(
        {
          onClick: (event) => {
            if (!event.defaultPrevented) {
              tap(event.nativeEvent);
            }
          },
          onKeyDown: (event) => {
            if (event.repeat && (event.key === "Enter" || event.key === " ")) {
              event.preventDefault();
            }
          },
        },
        { onClick, onKeyDown }
      )}
    >
      {children ?? "Tap"}
    </ButtonPrimitive>
  );
};

export const TempoControl = ({
  value: valueProp,
  defaultValue = 120,
  onValueChange,
  onValueCommitted,
  min: minProp = 30,
  max: maxProp = 600,
  largeStep = 10,
  tapTimeout = 2000,
  size: sizeProp,
  disabled: disabledProp,
  readOnly = false,
  name,
  className,
  children,
  ...props
}: TempoControlProps) => {
  const min = Math.ceil(minProp);
  const max = Math.floor(maxProp);
  const config = useAudioConfig();
  const disabled = disabledProp ?? config.disabled ?? false;
  const size = sizeProp ?? config.size ?? "default";

  const [empty, setEmpty] = useState(false);

  const { value, tap, setValue, resetTaps } = useTapTempo({
    defaultValue,
    disabled,
    max,
    min,
    onValueChange,
    onValueCommitted: (next) => {
      setEmpty(false);
      onValueCommitted?.(next);
    },
    readOnly,
    tapTimeout,
    value: valueProp,
  });

  const context = useMemo(
    () => ({ disabled, readOnly, resetTaps, tap }),
    [disabled, readOnly, resetTaps, tap]
  );

  return (
    <TempoContext.Provider value={context}>
      <NumberFieldPrimitive.Root
        aria-label="Tempo"
        className={cn(tempoControlVariants({ size }), className)}
        data-size={size}
        data-slot="tempo-control"
        disabled={disabled}
        format={TEMPO_FORMAT}
        largeStep={Math.max(1, Math.round(largeStep))}
        max={max}
        min={min}
        name={name}
        onValueChange={(next, details) => {
          resetTaps();
          setEmpty(next === null);

          if (next !== null) {
            setValue(next, {
              event: details.event,
              reason: details.reason === "keyboard" ? "keyboard" : "input",
            });
          }
        }}
        onValueCommitted={(next) => {
          if (next === null) {
            setEmpty(false);

            return;
          }

          onValueCommitted?.(clamp(Math.round(next), min, max));
        }}
        readOnly={readOnly}
        role="group"
        smallStep={1}
        step={1}
        value={empty ? null : value}
        {...props}
      >
        {children ?? (
          <>
            <TempoControlInput />
            <TempoControlTap />
          </>
        )}
      </NumberFieldPrimitive.Root>
    </TempoContext.Provider>
  );
};
