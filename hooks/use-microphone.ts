"use client";

import { useCallback, useEffect, useState } from "react";

export type MicrophoneStatus =
  | "idle"
  | "acquiring"
  | "active"
  | "denied"
  | "unavailable"
  | "error";

export interface UseMicrophoneOptions {
  /** The device to open. Omit for the system default. */
  deviceId?: string | null;
  /** Open the microphone as soon as possible. Default false. */
  enabled?: boolean;
  /** Browser echo cancellation. Default false, so meters show the real signal. */
  echoCancellation?: boolean;
  /** Browser noise suppression. Default false. */
  noiseSuppression?: boolean;
  /** Browser automatic gain control. Default false. */
  autoGainControl?: boolean;
  /** Requested channel count. */
  channelCount?: number;
}

export interface UseMicrophoneResult {
  stream: MediaStream | null;
  status: MicrophoneStatus;
  error: Error | null;
  start: () => Promise<void>;
  stop: () => void;
}

interface MicrophoneResult {
  key: string;
  stream: MediaStream | null;
  status: MicrophoneStatus;
  failure: Error | null;
}

const stopStream = (stream: MediaStream | null) => {
  if (!stream) {
    return;
  }
  for (const track of stream.getTracks()) {
    track.stop();
  }
};

const statusForError = (caught: unknown): MicrophoneStatus => {
  if (!(caught instanceof DOMException)) {
    return "error";
  }
  if (caught.name === "NotAllowedError" || caught.name === "SecurityError") {
    return "denied";
  }
  if (
    caught.name === "NotFoundError" ||
    caught.name === "OverconstrainedError"
  ) {
    return "unavailable";
  }
  return "error";
};

const canOpenMicrophone = () =>
  typeof navigator !== "undefined" &&
  Boolean(navigator.mediaDevices?.getUserMedia);

const buildConstraints = (
  options: Required<
    Pick<
      UseMicrophoneOptions,
      "autoGainControl" | "echoCancellation" | "noiseSuppression"
    >
  > &
    Pick<UseMicrophoneOptions, "channelCount" | "deviceId">
): MediaTrackConstraints => {
  const constraints: MediaTrackConstraints = {
    autoGainControl: options.autoGainControl,
    echoCancellation: options.echoCancellation,
    noiseSuppression: options.noiseSuppression,
  };
  if (options.deviceId) {
    constraints.deviceId = { exact: options.deviceId };
  }
  if (options.channelCount) {
    constraints.channelCount = options.channelCount;
  }
  return constraints;
};

/** Opens a microphone as a `MediaStream`, with browser processing off by default. */
export const useMicrophone = ({
  deviceId,
  enabled = false,
  echoCancellation = false,
  noiseSuppression = false,
  autoGainControl = false,
  channelCount,
}: UseMicrophoneOptions = {}): UseMicrophoneResult => {
  const [manual, setManual] = useState<boolean | null>(null);
  const [previousEnabled, setPreviousEnabled] = useState(enabled);
  if (previousEnabled !== enabled) {
    setPreviousEnabled(enabled);
    setManual(null);
  }
  const wanted = manual ?? enabled;
  const [result, setResult] = useState<MicrophoneResult | null>(null);

  const constraints = buildConstraints({
    autoGainControl,
    channelCount,
    deviceId,
    echoCancellation,
    noiseSuppression,
  });
  const key = JSON.stringify(constraints);

  useEffect(() => {
    if (!(wanted && canOpenMicrophone())) {
      return;
    }
    let cancelled = false;
    const listeners = new AbortController();
    let acquired: MediaStream | null = null;

    const open = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: JSON.parse(key) as MediaTrackConstraints,
        });
        if (cancelled) {
          stopStream(stream);
          return;
        }
        acquired = stream;
        const handleEnded = () => {
          setResult({
            failure: null,
            key,
            status: "unavailable",
            stream: null,
          });
        };
        for (const track of stream.getAudioTracks()) {
          track.addEventListener("ended", handleEnded, {
            signal: listeners.signal,
          });
        }
        setResult({ failure: null, key, status: "active", stream });
      } catch (error) {
        if (!cancelled) {
          setResult({
            failure: error instanceof Error ? error : new Error(String(error)),
            key,
            status: statusForError(error),
            stream: null,
          });
        }
      }
    };
    open();

    return () => {
      cancelled = true;
      listeners.abort();
      stopStream(acquired);
      // The stream is dead now; don't hand it out on the next start.
      setResult((current) => (current?.key === key ? null : current));
    };
  }, [key, wanted]);

  const start = useCallback(async () => {
    setManual(true);
    await Promise.resolve();
  }, []);

  const stop = useCallback(() => {
    setManual(false);
  }, []);

  if (!wanted) {
    return { error: null, start, status: "idle", stop, stream: null };
  }
  if (!canOpenMicrophone()) {
    return {
      error: new Error("This browser cannot open a microphone."),
      start,
      status: "unavailable",
      stop,
      stream: null,
    };
  }
  if (result?.key !== key) {
    return { error: null, start, status: "acquiring", stop, stream: null };
  }
  return {
    error: result.failure,
    start,
    status: result.status,
    stop,
    stream: result.stream,
  };
};
