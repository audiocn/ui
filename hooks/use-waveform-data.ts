"use client";

import { useEffect, useState } from "react";

import { getSharedAudioContext } from "@/hooks/use-audio-context";
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
      const end = Math.min(data.length, start + bucket);
      let peak = peaks[index] ?? 0;
      for (let sample = start; sample < end; sample += 1) {
        const magnitude = Math.abs(data[sample] ?? 0);
        if (magnitude > peak) {
          peak = magnitude;
        }
      }
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

/** Decodes a file (or takes an `AudioBuffer`) and reduces it to waveform peaks. */
export const useWaveformData = (
  src: string | AudioBuffer | null,
  { samples = 512 }: UseWaveformDataOptions = {}
): WaveformData => {
  const [data, setData] = useState<WaveformData>({
    duration: 0,
    error: null,
    peaks: null,
    status: "idle",
  });

  useEffect(() => {
    if (!src) {
      setData({ duration: 0, error: null, peaks: null, status: "idle" });
      return;
    }
    if (typeof src !== "string") {
      setData({
        duration: src.duration,
        error: null,
        peaks: computePeaks(src, samples),
        status: "ready",
      });
      return;
    }
    const context = getSharedAudioContext();
    if (!context) {
      return;
    }
    let cancelled = false;
    setData((previous) => ({ ...previous, error: null, status: "loading" }));
    loadAudioBuffer(context, src)
      .then((buffer) => {
        if (!cancelled) {
          setData({
            duration: buffer.duration,
            error: null,
            peaks: computePeaks(buffer, samples),
            status: "ready",
          });
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setData({
            duration: 0,
            error: error instanceof Error ? error : new Error(String(error)),
            peaks: null,
            status: "error",
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [samples, src]);

  return data;
};
