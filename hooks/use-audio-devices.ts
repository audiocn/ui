"use client";

import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

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

const toError = (caught: unknown) =>
  caught instanceof Error ? caught : new Error(String(caught));

const subscribeNothing = () => () => {
  // Media device support does not change during a visit.
};

const getServerSupport = () => false;

const listDevices = async (kind: AudioDeviceKind) => {
  const all = await navigator.mediaDevices.enumerateDevices();
  const matching = all.filter((device) => device.kind === kind);
  return {
    devices: matching.map((device, index) => ({
      groupId: device.groupId,
      id: device.deviceId,
      isDefault: device.deviceId === DEFAULT_DEVICE_ID,
      kind,
      label: device.label || fallbackLabel(kind, index),
    })),
    labelled: matching.some((device) => device.label !== ""),
  };
};

const queryMicrophonePermission =
  async (): Promise<PermissionStatus | null> => {
    try {
      return (
        (await navigator.permissions?.query({
          name: "microphone" as PermissionName,
        })) ?? null
      );
    } catch {
      // Some browsers cannot query microphone permission; labels tell us instead.
      return null;
    }
  };

/** Lists audio devices and keeps the list current as devices come and go. */
export const useAudioDevices = ({
  kind = "audioinput",
}: UseAudioDevicesOptions = {}): UseAudioDevicesResult => {
  const supported = useSyncExternalStore(
    subscribeNothing,
    hasMediaDevices,
    getServerSupport
  );
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [permissionState, setPermissionState] =
    useState<AudioPermission>("prompt");
  const [loaded, setLoaded] = useState(false);
  const [failure, setFailure] = useState<Error | null>(null);

  const refresh = useCallback(async () => {
    if (!hasMediaDevices()) {
      return;
    }
    try {
      const result = await listDevices(kind);
      if (result.labelled) {
        setPermissionState("granted");
      }
      setDevices(result.devices);
      setFailure(null);
    } catch (error) {
      setFailure(toError(error));
    }
    setLoaded(true);
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
      setPermissionState("granted");
      await refresh();
      return true;
    } catch (error) {
      const denied =
        error instanceof DOMException && error.name === "NotAllowedError";
      setPermissionState(denied ? "denied" : "prompt");
      setFailure(toError(error));
      return false;
    }
  }, [refresh]);

  useEffect(() => {
    if (!hasMediaDevices()) {
      return;
    }
    let disposed = false;
    const listeners = new AbortController();
    const watch = async () => {
      await refresh();
      const status = await queryMicrophonePermission();
      if (!status || disposed) {
        return;
      }
      setPermissionState(status.state);
      status.addEventListener(
        "change",
        () => {
          setPermissionState(status.state);
          refresh();
        },
        { signal: listeners.signal }
      );
    };
    navigator.mediaDevices.addEventListener(
      "devicechange",
      () => {
        refresh();
      },
      { signal: listeners.signal }
    );
    watch();

    return () => {
      disposed = true;
      listeners.abort();
    };
  }, [refresh]);

  return {
    devices,
    error: failure,
    isLoading: supported && !loaded,
    permission: supported ? permissionState : "unsupported",
    refresh,
    requestPermission,
  };
};
