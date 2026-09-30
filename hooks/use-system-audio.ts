"use client";

import { useCallback, useEffect, useRef, useState } from "react";

export type SystemAudioStatus =
  | "idle"
  | "prompting"
  | "active"
  | "no-audio"
  | "denied"
  | "ended"
  | "unsupported";

export interface UseSystemAudioOptions {
  /** Ask for the whole system's audio, not only a tab. Default true. */
  systemAudio?: boolean;
  /** Offer the current tab in the picker. Default false. */
  preferCurrentTab?: boolean;
}

export interface UseSystemAudioResult {
  /** The browser can capture display media at all. */
  isSupported: boolean;
  stream: MediaStream | null;
  status: SystemAudioStatus;
  error: Error | null;
  start: () => Promise<void>;
  stop: () => void;
}

interface DisplayMediaOptions extends DisplayMediaStreamOptions {
  systemAudio?: "include" | "exclude";
  preferCurrentTab?: boolean;
  selfBrowserSurface?: "include" | "exclude";
}

const isDisplayMediaSupported = () =>
  typeof navigator !== "undefined" &&
  typeof navigator.mediaDevices?.getDisplayMedia === "function";

const stopStream = (stream: MediaStream | null) => {
  if (!stream) {
    return;
  }
  for (const track of stream.getTracks()) {
    track.stop();
  }
};

/**
 * Captures system or tab audio through the browser's screen-share picker.
 * The user must tick "share audio"; browsers differ in what they allow.
 */
export const useSystemAudio = ({
  systemAudio = true,
  preferCurrentTab = false,
}: UseSystemAudioOptions = {}): UseSystemAudioResult => {
  const [isSupported, setIsSupported] = useState(false);
  const [stream, setStream] = useState<MediaStream | null>(null);
  const [status, setStatus] = useState<SystemAudioStatus>("idle");
  const [error, setError] = useState<Error | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

  useEffect(() => {
    const supported = isDisplayMediaSupported();
    setIsSupported(supported);
    if (!supported) {
      setStatus("unsupported");
    }
  }, []);

  const stop = useCallback(() => {
    stopStream(streamRef.current);
    streamRef.current = null;
    setStream(null);
    setStatus((previous) => (previous === "unsupported" ? previous : "idle"));
  }, []);

  const start = useCallback(async () => {
    if (!isDisplayMediaSupported()) {
      setStatus("unsupported");
      return;
    }
    stopStream(streamRef.current);
    setStatus("prompting");
    setError(null);

    const options: DisplayMediaOptions = {
      audio: {
        autoGainControl: false,
        echoCancellation: false,
        noiseSuppression: false,
      },
      preferCurrentTab,
      selfBrowserSurface: "exclude",
      systemAudio: systemAudio ? "include" : "exclude",
      video: true,
    };

    try {
      const display = await navigator.mediaDevices.getDisplayMedia(options);
      for (const track of display.getVideoTracks()) {
        track.stop();
      }
      const audioTracks = display.getAudioTracks();
      if (audioTracks.length === 0) {
        setStatus("no-audio");
        return;
      }
      const audio = new MediaStream(audioTracks);
      for (const track of audioTracks) {
        track.addEventListener("ended", () => {
          if (streamRef.current === audio) {
            streamRef.current = null;
            setStream(null);
            setStatus("ended");
          }
        });
      }
      streamRef.current = audio;
      setStream(audio);
      setStatus("active");
    } catch (error) {
      const denied =
        error instanceof DOMException && error.name === "NotAllowedError";
      setStatus(denied ? "denied" : "idle");
      setError(error instanceof Error ? error : new Error(String(error)));
    }
  }, [preferCurrentTab, systemAudio]);

  useEffect(
    () => () => {
      stopStream(streamRef.current);
    },
    []
  );

  return { error, isSupported, start, status, stop, stream };
};
