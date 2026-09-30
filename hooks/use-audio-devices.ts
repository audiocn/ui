"use client";

import { useCallback, useEffect, useState } from "react";

export type AudioDeviceKind = "audioinput" | "audiooutput";

export type AudioPermission = "granted" | "prompt" | "denied" | "unsupported";

export interface AudioDeviceInfo {
  id: string;
  label: string;
  kind: AudioDeviceKind;
  groupId: string;
  isDefault: boolean;
}

export interface UseAudioDevicesOptions {
  /** Which devices to list. Default `audioinput`. */
  kind?: AudioDeviceKind;
}

export interface UseAudioDevicesResult {
  devices: AudioDeviceInfo[];
  permission: AudioPermission;
  isLoading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
  /** Asks for microphone access so device labels become readable. */
  requestPermission: () => Promise<boolean>;
}

const DEFAULT_DEVICE_ID = "default";

const fallbackLabel = (kind: AudioDeviceKind, index: number) =>
  `${kind === "audioinput" ? "Microphone" : "Speaker"} ${index + 1}`;

const hasMediaDevices = () =>
  typeof navigator !== "undefined" &&
  Boolean(navigator.mediaDevices?.enumerateDevices);

const toError = (error: unknown) =>
  error instanceof Error ? error : new Error(String(error));

/** Lists audio devices and keeps the list current as devices come and go. */
export const useAudioDevices = ({
  kind = "audioinput",
}: UseAudioDevicesOptions = {}): UseAudioDevicesResult => {
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [permission, setPermission] = useState<AudioPermission>("prompt");
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const refresh = useCallback(async () => {
    if (!hasMediaDevices()) {
      setPermission("unsupported");
      setIsLoading(false);
      return;
    }
    try {
      const all = await navigator.mediaDevices.enumerateDevices();
      const matching = all.filter((device) => device.kind === kind);
      const labelled = matching.some((device) => device.label !== "");
      if (labelled) {
        setPermission("granted");
      }
      setDevices(
        matching.map((device, index) => ({
          groupId: device.groupId,
          id: device.deviceId,
          isDefault: device.deviceId === DEFAULT_DEVICE_ID,
          kind,
          label: device.label || fallbackLabel(kind, index),
        }))
      );
      setError(null);
    } catch (error) {
      setError(toError(error));
    } finally {
      setIsLoading(false);
    }
  }, [kind]);

  const requestPermission = useCallback(async () => {
    if (!hasMediaDevices()) {
      return false;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      for (const track of stream.getTracks()) {
        track.stop();
      }
      setPermission("granted");
      await refresh();
      return true;
    } catch (error) {
      const denied =
        error instanceof DOMException && error.name === "NotAllowedError";
      setPermission(denied ? "denied" : "prompt");
      setError(toError(error));
      return false;
    }
  }, [refresh]);

  useEffect(() => {
    if (!hasMediaDevices()) {
      setPermission("unsupported");
      setIsLoading(false);
      return;
    }
    refresh();
    const onChange = () => {
      refresh();
    };
    navigator.mediaDevices.addEventListener("devicechange", onChange);

    let status: PermissionStatus | null = null;
    const onPermissionChange = () => {
      if (status) {
        setPermission(status.state);
        refresh();
      }
    };
    navigator.permissions
      ?.query({ name: "microphone" as PermissionName })
      .then((result) => {
        status = result;
        setPermission(result.state);
        result.addEventListener("change", onPermissionChange);
      })
      .catch(() => {
        // Some browsers cannot query microphone permission; labels tell us instead.
      });

    return () => {
      navigator.mediaDevices.removeEventListener("devicechange", onChange);
      status?.removeEventListener("change", onPermissionChange);
    };
  }, [refresh]);

  return { devices, error, isLoading, permission, refresh, requestPermission };
};
