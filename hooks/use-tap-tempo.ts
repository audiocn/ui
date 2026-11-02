"use client";

import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";

import { clamp } from "@/lib/audio/decibels";

const MAX_TAPS = 5;
const MIN_TAP_INTERVAL = 80;
const MINUTE_MS = 60_000;

export interface TempoChangeDetails {
  reason: "input" | "keyboard" | "tap";
  event?: Event;
}

export interface UseTapTempoOptions {
  value?: number;
  defaultValue?: number;
  onValueChange?: (value: number, details: TempoChangeDetails) => void;
  onValueCommitted?: (value: number) => void;
  min?: number;
  max?: number;
  tapTimeout?: number;
  disabled?: boolean;
  readOnly?: boolean;
}

export interface TapTempo {
  value: number;
  tapCount: number;
  tap: (event?: Event) => void;
  setValue: (value: number, details?: TempoChangeDetails) => void;
  resetTaps: () => void;
}

/** Tap tempo with a four-interval average and a two-second idle reset. */
export const useTapTempo = ({
  value: valueProp,
  defaultValue = 120,
  onValueChange,
  onValueCommitted,
  min: minProp = 30,
  max: maxProp = 600,
  tapTimeout = 2000,
  disabled = false,
  readOnly = false,
}: UseTapTempoOptions = {}): TapTempo => {
  const min = Math.ceil(minProp);
  const max = Math.floor(maxProp);

  const [uncontrolled, setUncontrolled] = useState(defaultValue);
  const [tapCount, setTapCount] = useState(0);

  const value = clamp(Math.round(valueProp ?? uncontrolled), min, max);
  const latestValueRef = useRef(value);

  useLayoutEffect(() => {
    // Keep uncontrolled comparisons in sync with rendered bounds and values.
    latestValueRef.current = value;
  });

  const measurement = useMemo(() => {
    const taps: number[] = [];

    return {
      reset: () => {
        taps.length = 0;
      },
      tap: (now: number): { value: number | null } | null => {
        if (disabled || readOnly) {
          return null;
        }

        const previous = taps.at(-1);

        if (previous !== undefined) {
          const interval = now - previous;

          if (interval >= 0 && interval < MIN_TAP_INTERVAL) {
            return null;
          }

          if (interval > tapTimeout || interval < 0) {
            taps.length = 0;
          }
        }

        taps.push(now);

        if (taps.length > MAX_TAPS) {
          taps.shift();
        }

        const [first] = taps;

        if (first === undefined || taps.length < 2) {
          return { value: null };
        }

        const bpm = (MINUTE_MS * (taps.length - 1)) / (now - first);

        return {
          value: clamp(Math.round(bpm), min, max),
        };
      },
    };
  }, [disabled, readOnly, min, max, tapTimeout]);

  const change = useCallback(
    (next: number, details: TempoChangeDetails) => {
      if (disabled || readOnly || !Number.isFinite(next)) {
        return;
      }

      const bounded = clamp(Math.round(next), min, max);
      // Controlled edits are proposals; only the parent can accept a new value.
      const current = valueProp === undefined ? latestValueRef.current : value;

      if (bounded === current) {
        return;
      }

      if (valueProp === undefined) {
        latestValueRef.current = bounded;
        setUncontrolled(bounded);
      }

      onValueChange?.(bounded, details);
    },
    [disabled, readOnly, min, max, valueProp, value, onValueChange]
  );

  const tap = useCallback(
    (event?: Event) => {
      const result = measurement.tap(performance.now());

      if (result === null) {
        return;
      }

      setTapCount((count) => count + 1);
      const { value: next } = result;

      if (next === null) {
        return;
      }

      change(next, { event, reason: "tap" });
      onValueCommitted?.(next);
    },
    [change, measurement, onValueCommitted]
  );

  const setValue = useCallback(
    (next: number, details?: TempoChangeDetails) => {
      measurement.reset();
      change(next, details ?? { reason: "input" });
    },
    [change, measurement]
  );

  return { resetTaps: measurement.reset, setValue, tap, tapCount, value };
};
