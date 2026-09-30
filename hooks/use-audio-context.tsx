"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import type { ReactNode } from "react";

export type AudioContextStatus = AudioContextState | "unsupported";

const ProvidedContext = createContext<AudioContext | null>(null);

let sharedContext: AudioContext | null = null;

/** The page-wide `AudioContext`, created on first use. Null on the server. */
export const getSharedAudioContext = (): AudioContext | null => {
  if (typeof window === "undefined" || typeof AudioContext === "undefined") {
    return null;
  }
  if (!sharedContext || sharedContext.state === "closed") {
    sharedContext = new AudioContext({ latencyHint: "interactive" });
  }
  return sharedContext;
};

export interface AudioContextProviderProps {
  /** Use your own context instead of the shared one. */
  context: AudioContext;
  children: ReactNode;
}

/** Makes every audiocn hook below it use `context`. */
export const AudioContextProvider = ({
  context,
  children,
}: AudioContextProviderProps) => (
  <ProvidedContext.Provider value={context}>
    {children}
  </ProvidedContext.Provider>
);

const GESTURE_EVENTS = ["pointerdown", "keydown", "touchend"] as const;

export interface UseAudioContextResult {
  context: AudioContext | null;
  status: AudioContextStatus;
  /** Resumes a suspended context. Call it from a user gesture. */
  resume: () => Promise<void>;
}

/**
 * The shared (or provided) `AudioContext`. It resumes automatically on the
 * first click or key press, which browsers require before audio can start.
 */
export const useAudioContext = (): UseAudioContextResult => {
  const provided = useContext(ProvidedContext);
  const [context, setContext] = useState<AudioContext | null>(provided);
  const [status, setStatus] = useState<AudioContextStatus>(
    provided?.state ?? "suspended"
  );

  useEffect(() => {
    const next = provided ?? getSharedAudioContext();
    setContext(next);
    if (!next) {
      setStatus("unsupported");
      return;
    }
    const update = () => {
      setStatus(next.state);
    };
    update();
    next.addEventListener("statechange", update);

    const resumeOnGesture = () => {
      if (next.state === "suspended") {
        next.resume().catch(() => {
          // The browser can still refuse; the next gesture tries again.
        });
      }
    };
    for (const event of GESTURE_EVENTS) {
      document.addEventListener(event, resumeOnGesture, { passive: true });
    }

    return () => {
      next.removeEventListener("statechange", update);
      for (const event of GESTURE_EVENTS) {
        document.removeEventListener(event, resumeOnGesture);
      }
    };
  }, [provided]);

  const resume = useCallback(async () => {
    if (context && context.state === "suspended") {
      await context.resume();
    }
  }, [context]);

  return { context, resume, status };
};
