"use client";

import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { SILENCE_DB } from "@/lib/audio/decibels";
import type {
  FrameSource,
  MeterFrame,
  MeterZone,
  MeterZoneName,
} from "@/lib/audio/types";
import { DEFAULT_ZONES, zoneForDb } from "@/lib/audio/zones";

const DEFAULT_INTERVAL_MS = 250;

export interface UseLevelOptions {
  /** How often React state updates. Default 250 ms. */
  intervalMs?: number;
  /** A channel index, or `max` for the loudest channel. Default `max`. */
  channel?: number | "max";
  /** Zones used for `zone`. */
  zones?: MeterZone[];
  enabled?: boolean;
}

export interface LevelState {
  peakDb: number;
  rmsDb: number | undefined;
  zone: MeterZoneName;
}

const pickChannel = (frame: MeterFrame, channel: number | "max") => {
  if (channel !== "max") {
    return frame.channels[channel];
  }
  let [loudest] = frame.channels;
  for (const level of frame.channels) {
    if (!loudest || level.peakDb > loudest.peakDb) {
      loudest = level;
    }
  }
  return loudest;
};

/**
 * Reads a meter source into React state at a low rate. Use it for labels and
 * conditional UI, not for drawing meters.
 */
export const useLevel = (
  source: FrameSource<MeterFrame> | null | undefined,
  {
    intervalMs = DEFAULT_INTERVAL_MS,
    channel = "max",
    zones = DEFAULT_ZONES,
    enabled = true,
  }: UseLevelOptions = {}
): LevelState => {
  const latestRef = useRef<MeterFrame | null>(null);
  const [level, setLevel] = useState<Omit<LevelState, "zone">>({
    peakDb: SILENCE_DB,
    rmsDb: undefined,
  });

  useFrameSource(
    source,
    (frame) => {
      latestRef.current = frame;
    },
    { enabled }
  );

  // Reads `channel` when it samples, so an inline option doesn't restart the
  // timer on every render.
  const sample = useEffectEvent(() => {
    const frame = latestRef.current;
    if (!frame) {
      return;
    }
    const picked = pickChannel(frame, channel);
    const peakDb = picked?.peakDb ?? SILENCE_DB;
    const rmsDb = picked?.rmsDb;
    setLevel((previous) =>
      previous.peakDb === peakDb && previous.rmsDb === rmsDb
        ? previous
        : { peakDb, rmsDb }
    );
  });

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const timer = setInterval(() => {
      sample();
    }, intervalMs);
    return () => {
      clearInterval(timer);
    };
  }, [enabled, intervalMs]);

  // Computed in render, so new zones apply even while the level holds still.
  const zone = zoneForDb(level.peakDb, zones);
  return useMemo(() => ({ ...level, zone }), [level, zone]);
};
