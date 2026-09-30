"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
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

const bufferedEnd = (element: HTMLAudioElement) => {
  const { buffered } = element;
  return buffered.length > 0 ? buffered.end(buffered.length - 1) : 0;
};

const safeDuration = (element: HTMLAudioElement) =>
  Number.isFinite(element.duration) ? element.duration : 0;

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
  const [element, setElement] = useState<HTMLAudioElement | null>(null);
  const [status, setStatus] = useState<AudioPlayerStatus>("idle");
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [buffered, setBuffered] = useState(0);
  const [state, setState] = useState({ loop, muted, playbackRate, volume });
  const [error, setError] = useState<MediaError | null>(null);
  const callbacksRef = useRef({ onEnded, onError, onPause, onPlay });
  callbacksRef.current = { onEnded, onError, onPause, onPlay };
  const [timeSubscribers] = useState(() => new Set<(time: number) => void>());

  useEffect(() => {
    const audio = new Audio();
    setElement(audio);
    return () => {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    };
  }, []);

  useEffect(() => {
    if (!element) {
      return;
    }
    const sync = () => {
      setCurrentTime(element.currentTime);
      setDuration(safeDuration(element));
      setBuffered(bufferedEnd(element));
    };
    const handlers: Record<string, () => void> = {
      canplay: () => {
        setStatus((previous) => (previous === "loading" ? "ready" : previous));
      },
      durationchange: sync,
      ended: () => {
        setStatus("ended");
        callbacksRef.current.onEnded?.();
      },
      error: () => {
        setStatus("error");
        setError(element.error);
        callbacksRef.current.onError?.(element.error);
      },
      loadedmetadata: sync,
      loadstart: () => {
        setStatus("loading");
        setError(null);
      },
      pause: () => {
        setStatus((previous) => (previous === "ended" ? previous : "paused"));
        callbacksRef.current.onPause?.();
      },
      playing: () => {
        setStatus("playing");
        callbacksRef.current.onPlay?.();
      },
      progress: () => {
        setBuffered(bufferedEnd(element));
      },
      ratechange: () => {
        setState((previous) => ({
          ...previous,
          playbackRate: element.playbackRate,
        }));
      },
      seeked: sync,
      timeupdate: () => {
        setCurrentTime(element.currentTime);
      },
      volumechange: () => {
        setState((previous) => ({
          ...previous,
          muted: element.muted,
          volume: element.volume,
        }));
      },
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

  useEffect(() => {
    if (!element) {
      return;
    }
    if (crossOrigin) {
      element.crossOrigin = crossOrigin;
    }
    element.preload = preload;
    if (src) {
      element.src = src;
      element.load();
      if (autoPlay) {
        element.play().catch(() => {
          // Autoplay was blocked; the user can press play.
        });
      }
    } else {
      element.removeAttribute("src");
      setStatus("idle");
    }
    setCurrentTime(0);
  }, [autoPlay, crossOrigin, element, preload, src]);

  useEffect(() => {
    if (element) {
      element.volume = clamp(volume, 0, 1);
      element.muted = muted;
      element.loop = loop;
      element.playbackRate = playbackRate;
      setState({ loop, muted, playbackRate, volume });
    }
  }, [element, loop, muted, playbackRate, volume]);

  const playing = status === "playing";

  useEffect(() => {
    if (!(element && playing)) {
      return;
    }
    return subscribeFrame(() => {
      const time = element.currentTime;
      for (const subscriber of timeSubscribers) {
        subscriber(time);
      }
    });
  }, [element, playing, timeSubscribers]);

  const time = useMemo<FrameSource<number>>(
    () => ({
      subscribe: (callback) => {
        timeSubscribers.add(callback);
        if (element) {
          callback(element.currentTime);
        }
        return () => {
          timeSubscribers.delete(callback);
        };
      },
    }),
    [element, timeSubscribers]
  );

  const play = useCallback(async () => {
    if (!element) {
      return;
    }
    if (element.ended) {
      element.currentTime = 0;
    }
    try {
      await element.play();
    } catch {
      setStatus((previous) => (previous === "error" ? previous : "paused"));
    }
  }, [element]);

  const pause = useCallback(() => {
    element?.pause();
  }, [element]);

  const toggle = useCallback(async () => {
    if (element?.paused) {
      await play();
    } else {
      pause();
    }
  }, [element, pause, play]);

  const seek = useCallback(
    (seconds: number) => {
      if (!element) {
        return;
      }
      const target = clamp(seconds, 0, safeDuration(element) || seconds);
      element.currentTime = target;
      setCurrentTime(target);
      for (const subscriber of timeSubscribers) {
        subscriber(target);
      }
      if (status === "ended") {
        setStatus("paused");
      }
    },
    [element, status, timeSubscribers]
  );

  return {
    buffered,
    currentTime,
    duration,
    element,
    error,
    loop: state.loop,
    muted: state.muted,
    pause,
    play,
    playbackRate: state.playbackRate,
    playing,
    seek,
    setLoop: (next) => {
      if (element) {
        element.loop = next;
      }
      setState((previous) => ({ ...previous, loop: next }));
    },
    setMuted: (next) => {
      if (element) {
        element.muted = next;
      }
    },
    setPlaybackRate: (next) => {
      if (element) {
        element.playbackRate = next;
      }
    },
    setVolume: (next) => {
      if (element) {
        element.volume = clamp(next, 0, 1);
      }
    },
    status,
    time,
    toggle,
    volume: state.volume,
  };
};
