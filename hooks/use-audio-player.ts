"use client";

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { FrameSource } from "@/lib/audio/types";

export type AudioPlayerStatus =
  | "idle"
  | "loading"
  | "ready"
  | "playing"
  | "paused"
  | "ended"
  | "error";

export interface UseAudioPlayerOptions {
  src?: string;
  autoPlay?: boolean;
  loop?: boolean;
  /** 0..1. Default 1. */
  volume?: number;
  muted?: boolean;
  playbackRate?: number;
  preload?: "none" | "metadata" | "auto";
  /** Needed to analyse audio from another origin. */
  crossOrigin?: "anonymous" | "use-credentials";
  onPlay?: () => void;
  onPause?: () => void;
  onEnded?: () => void;
  onError?: (error: MediaError | null) => void;
}

export interface AudioPlayerController {
  /** The media element. Pass it to `useAudioAnalyser` or `useWebAudioMixer`. */
  element: HTMLAudioElement | null;
  status: AudioPlayerStatus;
  playing: boolean;
  /** Updates about four times a second. Use `time` for a smooth playhead. */
  currentTime: number;
  duration: number;
  /** End of the buffered range, in seconds. */
  buffered: number;
  volume: number;
  muted: boolean;
  playbackRate: number;
  loop: boolean;
  error: MediaError | null;
  play: () => Promise<void>;
  pause: () => void;
  toggle: () => Promise<void>;
  seek: (seconds: number) => void;
  setVolume: (volume: number) => void;
  setMuted: (muted: boolean) => void;
  setPlaybackRate: (rate: number) => void;
  setLoop: (loop: boolean) => void;
  /** The current time on every animation frame while playing. */
  time: FrameSource<number>;
}

interface PlaybackState {
  status: AudioPlayerStatus;
  currentTime: number;
  duration: number;
  buffered: number;
  volume: number;
  muted: boolean;
  playbackRate: number;
  failure: MediaError | null;
}

const bufferedEnd = (element: HTMLAudioElement) => {
  const { buffered } = element;
  return buffered.length > 0 ? buffered.end(buffered.length - 1) : 0;
};

const safeDuration = (element: HTMLAudioElement) =>
  Number.isFinite(element.duration) ? element.duration : 0;

/** One audio element per player, created on the client on first read. */
const createElementStore = () => {
  let element: HTMLAudioElement | null = null;
  return {
    get: () => {
      if (!element && typeof Audio !== "undefined") {
        element = new Audio();
      }
      return element;
    },
    subscribe: () => () => {
      // The element never changes.
    },
  };
};

const getServerElement = () => null;

type Callbacks = Pick<
  UseAudioPlayerOptions,
  "onEnded" | "onError" | "onPause" | "onPlay"
>;

/** Mirrors the element's events into React state. */
const usePlaybackState = (
  element: HTMLAudioElement | null,
  callbacks: Callbacks,
  initial: Pick<PlaybackState, "muted" | "playbackRate" | "volume">
) => {
  const [state, setState] = useState<PlaybackState>({
    buffered: 0,
    currentTime: 0,
    duration: 0,
    failure: null,
    status: "idle",
    ...initial,
  });
  const callbacksRef = useRef(callbacks);
  useLayoutEffect(() => {
    callbacksRef.current = callbacks;
  });

  useEffect(() => {
    if (!element) {
      return;
    }
    const patch = (next: Partial<PlaybackState>) => {
      setState((previous) => ({ ...previous, ...next }));
    };
    const sync = () => {
      patch({
        buffered: bufferedEnd(element),
        currentTime: element.currentTime,
        duration: safeDuration(element),
      });
    };
    const handlers: Record<string, () => void> = {
      canplay: () => {
        setState((previous) =>
          previous.status === "loading"
            ? { ...previous, status: "ready" }
            : previous
        );
      },
      durationchange: sync,
      emptied: () => patch({ buffered: 0, currentTime: 0, duration: 0 }),
      ended: () => {
        patch({ status: "ended" });
        callbacksRef.current.onEnded?.();
      },
      error: () => {
        patch({ failure: element.error, status: "error" });
        callbacksRef.current.onError?.(element.error);
      },
      loadedmetadata: sync,
      loadstart: () => patch({ failure: null, status: "loading" }),
      pause: () => {
        setState((previous) =>
          previous.status === "ended"
            ? previous
            : { ...previous, status: "paused" }
        );
        callbacksRef.current.onPause?.();
      },
      playing: () => {
        patch({ status: "playing" });
        callbacksRef.current.onPlay?.();
      },
      progress: () => patch({ buffered: bufferedEnd(element) }),
      ratechange: () => patch({ playbackRate: element.playbackRate }),
      seeked: sync,
      timeupdate: () => patch({ currentTime: element.currentTime }),
      volumechange: () =>
        patch({ muted: element.muted, volume: element.volume }),
    };
    for (const [event, handler] of Object.entries(handlers)) {
      element.addEventListener(event, handler);
    }
    return () => {
      for (const [event, handler] of Object.entries(handlers)) {
        element.removeEventListener(event, handler);
      }
    };
  }, [element]);

  return state;
};

const tryPlay = async (audio: HTMLAudioElement) => {
  try {
    await audio.play();
  } catch {
    // Playback was refused, for example before any user gesture.
  }
};

/** Playback state for an audio element the hook owns. */
export const useAudioPlayer = ({
  src,
  autoPlay = false,
  loop = false,
  volume = 1,
  muted = false,
  playbackRate = 1,
  preload = "metadata",
  crossOrigin,
  onPlay,
  onPause,
  onEnded,
  onError,
}: UseAudioPlayerOptions = {}): AudioPlayerController => {
  const store = useMemo(() => createElementStore(), []);
  const element = useSyncExternalStore(
    store.subscribe,
    store.get,
    getServerElement
  );
  const state = usePlaybackState(
    element,
    { onEnded, onError, onPause, onPlay },
    { muted, playbackRate, volume }
  );
  const [loopOverride, setLoopOverride] = useState<boolean | null>(null);
  const time = useMemo(() => createFrameEmitter<number>(), []);

  useEffect(() => {
    const audio = store.get();
    return () => {
      audio?.pause();
    };
  }, [store]);

  useEffect(() => {
    const audio = store.get();
    if (!audio) {
      return;
    }
    if (crossOrigin) {
      audio.crossOrigin = crossOrigin;
    }
    audio.preload = preload;
    if (!src) {
      audio.removeAttribute("src");
      return;
    }
    audio.src = src;
    audio.load();
    if (autoPlay) {
      tryPlay(audio);
    }
  }, [autoPlay, crossOrigin, preload, src, store]);

  useEffect(() => {
    const audio = store.get();
    if (audio) {
      audio.volume = clamp(volume, 0, 1);
      audio.muted = muted;
      audio.playbackRate = playbackRate;
    }
  }, [muted, playbackRate, store, volume]);

  const effectiveLoop = loopOverride ?? loop;

  useEffect(() => {
    const audio = store.get();
    if (audio) {
      audio.loop = effectiveLoop;
    }
  }, [effectiveLoop, store]);

  const status: AudioPlayerStatus = src ? state.status : "idle";
  const playing = status === "playing";

  useEffect(() => {
    const audio = store.get();
    if (!(audio && playing)) {
      return;
    }
    return subscribeFrame(() => {
      time.emit(audio.currentTime);
    });
  }, [playing, store, time]);

  const play = useCallback(async () => {
    const audio = store.get();
    if (!audio) {
      return;
    }
    if (audio.ended) {
      audio.currentTime = 0;
    }
    await tryPlay(audio);
  }, [store]);

  const pause = useCallback(() => {
    store.get()?.pause();
  }, [store]);

  const toggle = useCallback(async () => {
    if (store.get()?.paused) {
      await play();
    } else {
      pause();
    }
  }, [pause, play, store]);

  const seek = useCallback(
    (seconds: number) => {
      const audio = store.get();
      if (!audio) {
        return;
      }
      const target = clamp(seconds, 0, safeDuration(audio) || seconds);
      audio.currentTime = target;
      time.emit(target);
    },
    [store, time]
  );

  const setVolume = useCallback(
    (next: number) => {
      const audio = store.get();
      if (audio) {
        audio.volume = clamp(next, 0, 1);
      }
    },
    [store]
  );

  const setMuted = useCallback(
    (next: boolean) => {
      const audio = store.get();
      if (audio) {
        audio.muted = next;
      }
    },
    [store]
  );

  const setPlaybackRate = useCallback(
    (next: number) => {
      const audio = store.get();
      if (audio) {
        audio.playbackRate = next;
      }
    },
    [store]
  );

  return {
    buffered: state.buffered,
    currentTime: src ? state.currentTime : 0,
    duration: state.duration,
    element,
    error: state.failure,
    loop: effectiveLoop,
    muted: state.muted,
    pause,
    play,
    playbackRate: state.playbackRate,
    playing,
    seek,
    setLoop: setLoopOverride,
    setMuted,
    setPlaybackRate,
    setVolume,
    status,
    time,
    toggle,
    volume: state.volume,
  };
};
