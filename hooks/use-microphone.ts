"use client";

import { useCallback, useEffect, useRef, useState } from "react";

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

const stopStream = (stream: MediaStream | null) => {
  if (!stream) {
    return;
  }
  for (const track of stream.getTracks()) {
    track.stop();
  }
};

const statusForError = (error: unknown): MicrophoneStatus => {
  if (!(error instanceof DOMException)) {
    return "error";
  }
  if (error.name === "NotAllowedError" || error.name === "SecurityError") {
    return "denied";
  }
  if (error.name === "NotFoundError" || error.name === "OverconstrainedError") {
    return "unavailable";
  }
  return "error";
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
  const [stream, setStream] = useState<MediaStream | null>(null);
  const [status, setStatus] = useState<MicrophoneStatus>("idle");
  const [error, setError] = useState<Error | null>(null);
  const [wanted, setWanted] = useState(enabled);
  const requestRef = useRef(0);

  useEffect(() => {
    setWanted(enabled);
  }, [enabled]);

  const start = useCallback(async () => {
    setWanted(true);
    await Promise.resolve();
  }, []);

  const stop = useCallback(() => {
    setWanted(false);
  }, []);

  useEffect(() => {
    if (!wanted) {
      requestRef.current += 1;
      setStream(null);
      setStatus("idle");
      return;
    }
    if (
      typeof navigator === "undefined" ||
      !navigator.mediaDevices?.getUserMedia
    ) {
      setStatus("unavailable");
      setError(new Error("This browser cannot open a microphone."));
      return;
    }

    requestRef.current += 1;
    const request = requestRef.current;
    let acquired: MediaStream | null = null;
    setStatus("acquiring");
    setError(null);

    const constraints: MediaTrackConstraints = {
      autoGainControl,
      echoCancellation,
      noiseSuppression,
    };
    if (deviceId) {
      constraints.deviceId = { exact: deviceId };
    }
    if (channelCount) {
      constraints.channelCount = channelCount;
    }

    navigator.mediaDevices
      .getUserMedia({ audio: constraints })
      .then((result) => {
        if (request !== requestRef.current) {
          stopStream(result);
          return;
        }
        acquired = result;
        for (const track of result.getAudioTracks()) {
          track.addEventListener("ended", () => {
            if (request === requestRef.current) {
              setStatus("unavailable");
              setStream(null);
            }
          });
        }
        setStream(result);
        setStatus("active");
      })
      .catch((error: unknown) => {
        if (request !== requestRef.current) {
          return;
        }
        setStatus(statusForError(error));
        setError(error instanceof Error ? error : new Error(String(error)));
      });

    return () => {
      stopStream(acquired);
    };
  }, [
    autoGainControl,
    channelCount,
    deviceId,
    echoCancellation,
    noiseSuppression,
    wanted,
  ]);

  return { error, start, status, stop, stream };
};
