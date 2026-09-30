"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource } from "@/lib/audio/types";

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
  const loading = fetch(src)
    .then((response) => {
      if (!response.ok) {
        throw new Error(`Could not load ${src}: ${response.status}`);
      }
      return response.arrayBuffer();
    })
    .then((data) => context.decodeAudioData(data));
  cache.set(src, loading);
  loading.catch(() => {
    cache?.delete(src);
  });
  return loading;
};

interface Voice {
  node: AudioBufferSourceNode;
  startedAt: number;
}

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
  const [buffer, setBuffer] = useState<AudioBuffer | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [output, setOutput] = useState<GainNode | null>(null);
  const voicesRef = useRef<Voice[]>([]);
  const [progressSubscribers] = useState(
    () => new Set<(value: number) => void>()
  );

  useEffect(() => {
    if (!context) {
      return;
    }
    const gain = context.createGain();
    setOutput(gain);
    return () => {
      gain.disconnect();
      setOutput(null);
    };
  }, [context]);

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
      output.gain.setTargetAtTime(volume, context.currentTime, 0.005);
    }
  }, [context, output, volume]);

  useEffect(() => {
    if (!src) {
      setBuffer(null);
      return;
    }
    if (typeof src !== "string") {
      setBuffer(src);
      return;
    }
    if (!context) {
      return;
    }
    let cancelled = false;
    setError(null);
    loadAudioBuffer(context, src)
      .then((decoded) => {
        if (!cancelled) {
          setBuffer(decoded);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setError(error instanceof Error ? error : new Error(String(error)));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [context, src]);

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
      context.resume().catch(() => {
        // Resumed on the next gesture instead.
      });
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
    node.addEventListener("ended", () => {
      voicesRef.current = voicesRef.current.filter((item) => item !== voice);
      if (voicesRef.current.length === 0) {
        setIsPlaying(false);
        for (const subscriber of progressSubscribers) {
          subscriber(0);
        }
      }
    });
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
    progressSubscribers,
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
      for (const subscriber of progressSubscribers) {
        subscriber(value);
      }
    });
  }, [buffer, context, isPlaying, loop, playbackRate, progressSubscribers]);

  useEffect(() => stop, [stop]);

  const progress = useMemo<FrameSource<number>>(
    () => ({
      subscribe: (listener) => {
        progressSubscribers.add(listener);
        return () => {
          progressSubscribers.delete(listener);
        };
      },
    }),
    [progressSubscribers]
  );

  return {
    duration: buffer?.duration ?? 0,
    error,
    isLoaded: buffer !== null,
    isPlaying,
    output,
    play,
    progress,
    stop,
  };
};
