"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

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

const subscribeNothing = () => () => {
  // Display media support does not change during a visit.
};

const getServerSupport = () => false;

/**
 * Captures system or tab audio through the browser's screen-share picker.
 * The user must tick "share audio"; browsers differ in what they allow.
 */
export const useSystemAudio = ({
  systemAudio = true,
  preferCurrentTab = false,
}: UseSystemAudioOptions = {}): UseSystemAudioResult => {
  const isSupported = useSyncExternalStore(
    subscribeNothing,
    isDisplayMediaSupported,
    getServerSupport
  );
  const [stream, setStream] = useState<MediaStream | null>(null);
  const [captureStatus, setCaptureStatus] = useState<SystemAudioStatus>("idle");
  const [failure, setFailure] = useState<Error | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const status: SystemAudioStatus = isSupported ? captureStatus : "unsupported";

  const stop = useCallback(() => {
    stopStream(streamRef.current);
    streamRef.current = null;
    setStream(null);
    setCaptureStatus("idle");
  }, []);

  const start = useCallback(async () => {
    if (!isDisplayMediaSupported()) {
      setCaptureStatus("unsupported");
      return;
    }
    stopStream(streamRef.current);
    setCaptureStatus("prompting");
    setFailure(null);

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
        setCaptureStatus("no-audio");
        return;
      }
      const audio = new MediaStream(audioTracks);
      for (const track of audioTracks) {
        track.addEventListener(
          "ended",
          () => {
            if (streamRef.current === audio) {
              streamRef.current = null;
              setStream(null);
              setCaptureStatus("ended");
            }
          },
          { once: true }
        );
      }
      streamRef.current = audio;
      setStream(audio);
      setCaptureStatus("active");
    } catch (error) {
      const denied =
        error instanceof DOMException && error.name === "NotAllowedError";
      setCaptureStatus(denied ? "denied" : "idle");
      setFailure(error instanceof Error ? error : new Error(String(error)));
    }
  }, [preferCurrentTab, systemAudio]);

  useEffect(
    () => () => {
      stopStream(streamRef.current);
    },
    []
  );

  return { error: failure, isSupported, start, status, stop, stream };
};
