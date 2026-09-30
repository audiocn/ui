"use client";

import { useEffect, useRef } from "react";
import type { ComponentProps, CSSProperties } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { formatDb, SILENCE_DB } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame, MeterZone } from "@/lib/audio/types";
import { DEFAULT_ZONES, zoneForDb } from "@/lib/audio/zones";
import { cn } from "@/lib/utils";

const DEFAULT_INTERVAL_MS = 250;
const DEFAULT_FLOOR_DB = -60;
/** Two-digit levels, the widest a meter usually shows, on either side of 0. */
const WIDEST_MAGNITUDE_DB = 88.8;

export interface DbReadoutProps extends Omit<
  ComponentProps<"span">,
  "children"
> {
  /** A level in dB, for declarative use. */
  value?: number;
  /** A meter source; the readout updates itself without re-rendering React. */
  source?: FrameSource<MeterFrame> | null;
  /** Which measurement to show. Default `peak`. */
  measure?: "peak" | "rms";
  /** A channel index, or `max` for the loudest channel. Default `max`. */
  channel?: number | "max";
  /** How often the text changes. Default 250 ms. */
  intervalMs?: number;
  /** Show the highest value seen within this window. Default 0. */
  holdMs?: number;
  /** Digits after the decimal point. Default 1. */
  decimals?: number;
  /** Append " dB". Default true. */
  unit?: boolean;
  /** At or below this level, show "−∞". Default −60. */
  floorDb?: number;
  /** Zones used for `data-zone`. */
  zones?: MeterZone[];
  /** Replaces all formatting. */
  format?: (db: number) => string;
}

const readChannel = (
  frame: MeterFrame,
  measure: "peak" | "rms",
  channel: number | "max"
) => {
  const read = (index: number) => {
    const level = frame.channels[index];
    if (!level) {
      return SILENCE_DB;
    }
    return measure === "rms" ? (level.rmsDb ?? level.peakDb) : level.peakDb;
  };
  if (channel !== "max") {
    return read(channel);
  }
  let loudest = SILENCE_DB;
  for (let index = 0; index < frame.channels.length; index += 1) {
    loudest = Math.max(loudest, read(index));
  }
  return loudest;
};

export const DbReadout = ({
  value,
  source,
  measure = "peak",
  channel = "max",
  intervalMs = DEFAULT_INTERVAL_MS,
  holdMs = 0,
  decimals = 1,
  unit = true,
  floorDb = DEFAULT_FLOOR_DB,
  zones = DEFAULT_ZONES,
  format,
  className,
  style,
  ...props
}: DbReadoutProps) => {
  const elementRef = useRef<HTMLSpanElement>(null);
  const peakRef = useRef(SILENCE_DB);
  const peakAtRef = useRef(0);
  const shownRef = useRef<string | null>(null);

  const render = (db: number) =>
    format ? format(db) : formatDb(db, { decimals, floorDb, unit });

  const initialDb = value ?? SILENCE_DB;
  // Size for the widest text the readout can show, so it never changes width.
  // Values at or below the floor read as −∞, so measure just above it too.
  const justAboveFloor = Number.isFinite(floorDb)
    ? floorDb + 10 ** -decimals
    : -WIDEST_MAGNITUDE_DB;
  const widest = Math.max(
    ...[
      SILENCE_DB,
      justAboveFloor,
      -WIDEST_MAGNITUDE_DB,
      WIDEST_MAGNITUDE_DB,
    ].map((db) => render(db).length)
  );

  useFrameSource(source, (frame) => {
    const db = readChannel(frame, measure, channel);
    const now = performance.now();
    if (db >= peakRef.current || now - peakAtRef.current > holdMs) {
      peakRef.current = db;
      peakAtRef.current = now;
    }
  });

  useEffect(() => {
    if (!source) {
      return;
    }
    const update = () => {
      const element = elementRef.current;
      if (!element) {
        return;
      }
      const db = peakRef.current;
      const text = format
        ? format(db)
        : formatDb(db, { decimals, floorDb, unit });
      if (text !== shownRef.current) {
        shownRef.current = text;
        if (element.firstChild) {
          element.firstChild.nodeValue = text;
        } else {
          element.textContent = text;
        }
        element.dataset.zone = zoneForDb(db, zones);
        element.toggleAttribute("data-silent", db <= floorDb);
      }
      if (holdMs === 0) {
        peakRef.current = SILENCE_DB;
      }
    };
    const timer = setInterval(update, intervalMs);
    return () => {
      clearInterval(timer);
    };
  }, [decimals, floorDb, format, holdMs, intervalMs, source, unit, zones]);

  return (
    <span
      className={cn(
        "inline-block min-w-(--db-readout-width) text-end font-mono tabular-nums",
        className
      )}
      data-silent={initialDb <= floorDb ? "" : undefined}
      data-slot="db-readout"
      data-zone={zoneForDb(initialDb, zones)}
      ref={elementRef}
      style={{ "--db-readout-width": `${widest}ch`, ...style } as CSSProperties}
      {...props}
    >
      {render(initialDb)}
    </span>
  );
};
