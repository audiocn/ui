"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { FrameSource } from "@/lib/audio/types";

const RAMP_SECONDS = 0.005;

export interface UseSoundOptions {
  /** 0..1 gain. Default 1. */
  volume?: number;
  /** Default 1. */
  playbackRate?: number;
  loop?: boolean;
  /** Restart instead of layering when played again. Default true. */
  interrupt?: boolean;
  /** Most simultaneous voices when not interrupting. Default 4. */
  maxVoices?: number;
  /**
   * Where the sound goes. Default: the speakers. Pass `null` to route
   * `output` yourself, for example into a mixer.
   */
  destination?: AudioNode | null;
}

export interface SoundController {
  play: () => void;
  stop: () => void;
  isPlaying: boolean;
  isLoaded: boolean;
  error: Error | null;
  duration: number;
  /** Playback progress, 0..1, on every animation frame while playing. */
  progress: FrameSource<number>;
  /** The sound's output node. */
  output: AudioNode | null;
}

const bufferCache = new WeakMap<
  BaseAudioContext,
  Map<string, Promise<AudioBuffer>>
>();

const fetchAndDecode = async (context: BaseAudioContext, src: string) => {
  const response = await fetch(src);
  if (!response.ok) {
    throw new Error(`Could not load ${src}: ${response.status}`);
  }
  const data = await response.arrayBuffer();
  return context.decodeAudioData(data);
};

/** Fetches and decodes a sound once per context. */
export const loadAudioBuffer = (
  context: BaseAudioContext,
  src: string
): Promise<AudioBuffer> => {
  let cache = bufferCache.get(context);
  if (!cache) {
    cache = new Map();
    bufferCache.set(context, cache);
  }
  const cached = cache.get(src);
  if (cached) {
    return cached;
  }
  const loading = fetchAndDecode(context, src);
  cache.set(src, loading);
  const forgetOnFailure = async () => {
    try {
      await loading;
    } catch {
      cache?.delete(src);
    }
  };
  forgetOnFailure();
  return loading;
};

interface Voice {
  node: AudioBufferSourceNode;
  startedAt: number;
}

interface LoadResult {
  src: string;
  buffer: AudioBuffer | null;
  failure: Error | null;
}

/** Loads a sound by URL, or passes an `AudioBuffer` straight through. */
const useSoundBuffer = (
  context: AudioContext | null,
  src: string | AudioBuffer | null
) => {
  const [result, setResult] = useState<LoadResult | null>(null);

  useEffect(() => {
    if (!(context && typeof src === "string")) {
      return;
    }
    let cancelled = false;
    const load = async () => {
      try {
        const buffer = await loadAudioBuffer(context, src);
        if (!cancelled) {
          setResult({ buffer, failure: null, src });
        }
      } catch (error) {
        if (!cancelled) {
          setResult({
            buffer: null,
            failure: error instanceof Error ? error : new Error(String(error)),
            src,
          });
        }
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [context, src]);

  if (!src) {
    return { buffer: null, failure: null };
  }
  if (typeof src !== "string") {
    return { buffer: src, failure: null };
  }
  if (result?.src !== src) {
    return { buffer: null, failure: null };
  }
  return { buffer: result.buffer, failure: result.failure };
};

/** Low-latency playback of a short sound, decoded into memory. */
export const useSound = (
  src: string | AudioBuffer | null,
  {
    volume = 1,
    playbackRate = 1,
    loop = false,
    interrupt = true,
    maxVoices = 4,
    destination,
  }: UseSoundOptions = {}
): SoundController => {
  const { context } = useAudioContext();
  const { buffer, failure } = useSoundBuffer(context, src);
  const [isPlaying, setIsPlaying] = useState(false);
  const voicesRef = useRef<Voice[]>([]);
  const progress = useMemo(() => createFrameEmitter<number>(), []);
  const output = useMemo(() => context?.createGain() ?? null, [context]);

  useEffect(() => {
    if (!(context && output)) {
      return;
    }
    const target =
      destination === undefined ? context.destination : destination;
    if (!target) {
      return;
    }
    output.connect(target);
    return () => {
      output.disconnect(target);
    };
  }, [context, destination, output]);

  useEffect(() => {
    if (output && context) {
      output.gain.setTargetAtTime(volume, context.currentTime, RAMP_SECONDS);
    }
  }, [context, output, volume]);

  const stop = useCallback(() => {
    for (const voice of voicesRef.current) {
      try {
        voice.node.stop();
      } catch {
        // Already stopped.
      }
    }
    voicesRef.current = [];
    setIsPlaying(false);
  }, []);

  const play = useCallback(() => {
    if (!(context && buffer && output)) {
      return;
    }
    if (context.state === "suspended") {
      context.resume();
    }
    if (interrupt) {
      stop();
    } else if (voicesRef.current.length >= maxVoices) {
      voicesRef.current.shift()?.node.stop();
    }
    const node = context.createBufferSource();
    node.buffer = buffer;
    node.loop = loop;
    node.playbackRate.value = playbackRate;
    node.connect(output);
    const voice: Voice = { node, startedAt: context.currentTime };
    node.addEventListener(
      "ended",
      () => {
        voicesRef.current = voicesRef.current.filter((item) => item !== voice);
        if (voicesRef.current.length === 0) {
          setIsPlaying(false);
          progress.emit(0);
        }
      },
      { once: true }
    );
    node.start();
    voicesRef.current.push(voice);
    setIsPlaying(true);
  }, [
    buffer,
    context,
    interrupt,
    loop,
    maxVoices,
    output,
    playbackRate,
    progress,
    stop,
  ]);

  useEffect(() => {
    if (!(isPlaying && context && buffer)) {
      return;
    }
    return subscribeFrame(() => {
      const latest = voicesRef.current.at(-1);
      if (!latest) {
        return;
      }
      const elapsed = (context.currentTime - latest.startedAt) * playbackRate;
      const value = loop
        ? (elapsed % buffer.duration) / buffer.duration
        : Math.min(1, elapsed / buffer.duration);
      progress.emit(value);
    });
  }, [buffer, context, isPlaying, loop, playbackRate, progress]);

  useEffect(() => stop, [stop]);

  useEffect(
    () => () => {
      output?.disconnect();
    },
    [output]
  );

  return {
    duration: buffer?.duration ?? 0,
    error: failure,
    isLoaded: buffer !== null,
    isPlaying,
    output,
    play,
    progress,
    stop,
  };
};
