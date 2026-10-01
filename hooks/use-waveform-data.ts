"use client";

import { useEffect, useMemo, useState } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";
import { loadAudioBuffer } from "@/hooks/use-sound";

export type WaveformDataStatus = "idle" | "loading" | "ready" | "error";

export interface UseWaveformDataOptions {
  /** Number of peaks to compute. Default 512. */
  samples?: number;
}

export interface WaveformData {
  /** Loudest absolute sample per bucket, normalised so the loudest is 1. */
  peaks: Float32Array | null;
  duration: number;
  status: WaveformDataStatus;
  error: Error | null;
}

const peakCache = new WeakMap<AudioBuffer, Map<number, Float32Array>>();

const loudestInRange = (data: Float32Array, start: number, end: number) => {
  let peak = 0;
  for (let sample = start; sample < end; sample += 1) {
    const magnitude = Math.abs(data[sample] ?? 0);
    if (magnitude > peak) {
      peak = magnitude;
    }
  }
  return peak;
};

/** Reduces an audio buffer to `samples` peaks, 0..1, cached per buffer. */
export const computePeaks = (
  buffer: AudioBuffer,
  samples: number
): Float32Array => {
  let cache = peakCache.get(buffer);
  if (!cache) {
    cache = new Map();
    peakCache.set(buffer, cache);
  }
  const cached = cache.get(samples);
  if (cached) {
    return cached;
  }
  const peaks = new Float32Array(samples);
  const bucket = Math.max(1, Math.floor(buffer.length / samples));
  let loudest = 0;
  for (let channel = 0; channel < buffer.numberOfChannels; channel += 1) {
    const data = buffer.getChannelData(channel);
    for (let index = 0; index < samples; index += 1) {
      const start = index * bucket;
      const peak = Math.max(
        peaks[index] ?? 0,
        loudestInRange(data, start, Math.min(data.length, start + bucket))
      );
      peaks[index] = peak;
      loudest = Math.max(loudest, peak);
    }
  }
  if (loudest > 0) {
    for (let index = 0; index < samples; index += 1) {
      peaks[index] = (peaks[index] ?? 0) / loudest;
    }
  }
  cache.set(samples, peaks);
  return peaks;
};

interface LoadResult {
  key: string;
  data: WaveformData;
}

const IDLE: WaveformData = {
  duration: 0,
  error: null,
  peaks: null,
  status: "idle",
};

const LOADING: WaveformData = { ...IDLE, status: "loading" };

const UNSUPPORTED: WaveformData = {
  ...IDLE,
  error: new Error("This browser can't decode audio."),
  status: "error",
};

/** Decodes a file (or takes an `AudioBuffer`) and reduces it to waveform peaks. */
export const useWaveformData = (
  src: string | AudioBuffer | null,
  { samples = 512 }: UseWaveformDataOptions = {}
): WaveformData => {
  const { context, status } = useAudioContext();
  const [result, setResult] = useState<LoadResult | null>(null);
  const key = typeof src === "string" ? `${samples}:${src}` : "";

  useEffect(() => {
    if (typeof src !== "string" || !context) {
      return;
    }
    let cancelled = false;
    const load = async () => {
      try {
        const buffer = await loadAudioBuffer(context, src);
        if (!cancelled) {
          setResult({
            data: {
              duration: buffer.duration,
              error: null,
              peaks: computePeaks(buffer, samples),
              status: "ready",
            },
            key: `${samples}:${src}`,
          });
        }
      } catch (error) {
        if (!cancelled) {
          setResult({
            data: {
              ...IDLE,
              error: error instanceof Error ? error : new Error(String(error)),
              status: "error",
            },
            key: `${samples}:${src}`,
          });
        }
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [context, samples, src]);

  const direct = useMemo<WaveformData | null>(() => {
    if (!src || typeof src === "string") {
      return null;
    }
    return {
      duration: src.duration,
      error: null,
      peaks: computePeaks(src, samples),
      status: "ready",
    };
  }, [samples, src]);

  if (!src) {
    return IDLE;
  }
  if (direct) {
    return direct;
  }
  // Without Web Audio there is nothing to decode with; say so instead of
  // loading forever.
  if (status === "unsupported") {
    return UNSUPPORTED;
  }
  return result?.key === key ? result.data : LOADING;
};
