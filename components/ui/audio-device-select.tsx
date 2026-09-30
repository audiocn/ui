"use client";

import { createContext, useContext, useMemo, useState } from "react";
import type { ComponentProps, ReactNode } from "react";

import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

const NONE_VALUE = "__none__";

export type AudioDeviceStatus =
  | "available"
  | "unavailable"
  | "permission-required";

export interface AudioDevice {
  id: string;
  label: string;
  isDefault?: boolean;
  status?: AudioDeviceStatus;
  description?: string;
}

interface DeviceItem {
  value: string;
  label: string;
  device?: AudioDevice;
  missing?: boolean;
  none?: boolean;
}

interface AudioDeviceSelectContextValue {
  items: DeviceItem[];
  loading: boolean;
  permission: "granted" | "prompt" | "denied";
  missing: boolean;
  onRequestPermission?: () => void;
}

const AudioDeviceSelectContext =
  createContext<AudioDeviceSelectContextValue | null>(null);

const useAudioDeviceSelect = (part: string) => {
  const context = useContext(AudioDeviceSelectContext);
  if (!context) {
    throw new Error(`${part} must be used inside AudioDeviceSelect.`);
  }
  return context;
};

export const AudioDeviceSelectTrigger = ({
  className,
  ...props
}: ComponentProps<typeof SelectTrigger>) => {
  const { loading, missing, permission } = useAudioDeviceSelect(
    "AudioDeviceSelectTrigger"
  );
  return (
    <SelectTrigger
      className={cn("w-full min-w-0", className)}
      data-loading={loading ? "" : undefined}
      data-missing={missing ? "" : undefined}
      data-permission={permission}
      data-slot="audio-device-select-trigger"
      {...props}
    />
  );
};

export interface AudioDeviceSelectValueProps extends Omit<
  ComponentProps<typeof SelectValue>,
  "children"
> {
  /** Shown with no selection. Default "Select a device". */
  placeholder?: string;
}

export const AudioDeviceSelectValue = ({
  placeholder = "Select a device",
  className,
  ...props
}: AudioDeviceSelectValueProps) => {
  const { items, loading } = useAudioDeviceSelect("AudioDeviceSelectValue");
  return (
    <SelectValue
      className={cn("truncate", className)}
      data-slot="audio-device-select-value"
      {...props}
    >
      {(selected: string | null) => {
        if (selected === null) {
          return (
            <span className="text-muted-foreground">
              {loading ? "Finding devices…" : placeholder}
            </span>
          );
        }
        return (
          items.find((item) => item.value === selected)?.label ?? placeholder
        );
      }}
    </SelectValue>
  );
};

export interface AudioDeviceSelectItemProps extends ComponentProps<
  typeof SelectItem
> {
  device?: AudioDevice;
}

export const AudioDeviceSelectItem = ({
  device,
  className,
  children,
  ...props
}: AudioDeviceSelectItemProps) => (
  <SelectItem
    className={cn("items-start", className)}
    data-slot="audio-device-select-item"
    {...props}
  >
    {children ?? (
      <span className="flex min-w-0 flex-col">
        <span className="flex items-center gap-2">
          <span className="truncate">{device?.label}</span>
          {device?.isDefault && !device.label.startsWith("Default") ? (
            <span className="text-muted-foreground text-xs">Default</span>
          ) : null}
        </span>
        {device?.description ? (
          <span className="text-muted-foreground text-xs">
            {device.description}
          </span>
        ) : null}
      </span>
    )}
  </SelectItem>
);

export interface AudioDeviceSelectPermissionProps extends ComponentProps<"div"> {
  /** Default: "Allow microphone access to see your devices." */
  message?: string;
  /** Default: "Allow access". */
  actionLabel?: string;
}

export const AudioDeviceSelectPermission = ({
  message,
  actionLabel = "Allow access",
  className,
  ...props
}: AudioDeviceSelectPermissionProps) => {
  const { onRequestPermission, permission } = useAudioDeviceSelect(
    "AudioDeviceSelectPermission"
  );
  const denied = permission === "denied";
  const text =
    message ??
    (denied
      ? "Microphone access is blocked. Allow it in your browser's site settings."
      : "Allow microphone access to see your devices.");

  return (
    <div
      className={cn(
        "text-muted-foreground flex flex-col items-start gap-2 p-2 text-xs",
        className
      )}
      data-slot="audio-device-select-permission"
      {...props}
    >
      <p>{text}</p>
      {!denied && onRequestPermission ? (
        <Button onClick={onRequestPermission} size="xs" variant="outline">
          {actionLabel}
        </Button>
      ) : null}
    </div>
  );
};

export const AudioDeviceSelectContent = ({
  children,
  ...props
}: ComponentProps<typeof SelectContent>) => {
  const { items, loading, permission } = useAudioDeviceSelect(
    "AudioDeviceSelectContent"
  );
  const needsPermission = permission !== "granted";

  return (
    <SelectContent
      alignItemWithTrigger={false}
      data-slot="audio-device-select-content"
      {...props}
    >
      {children ?? (
        <SelectGroup>
          {needsPermission ? <AudioDeviceSelectPermission /> : null}
          {loading ? (
            <SelectItem disabled value="__loading__">
              Finding devices…
            </SelectItem>
          ) : null}
          {items.map((item) => {
            if (item.none || item.missing) {
              return (
                <SelectItem
                  disabled={item.missing}
                  key={item.value}
                  value={item.value}
                >
                  {item.label}
                </SelectItem>
              );
            }
            const status = item.device?.status ?? "available";
            return (
              <AudioDeviceSelectItem
                device={item.device}
                disabled={status !== "available"}
                key={item.value}
                value={item.value}
              />
            );
          })}
          {!loading && items.length === 0 && !needsPermission ? (
            <p className="text-muted-foreground p-2 text-xs">
              No devices found.
            </p>
          ) : null}
        </SelectGroup>
      )}
    </SelectContent>
  );
};

export const AudioDeviceSelectPreview = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn(
      "bg-muted/20 overflow-hidden rounded-lg border px-2",
      className
    )}
    data-slot="audio-device-select-preview"
    {...props}
  />
);

export interface AudioDeviceSelectProps {
  devices: AudioDevice[];
  /** The selected device id. `null` is no selection, or "None" with `allowNone`. */
  value?: string | null;
  defaultValue?: string | null;
  onValueChange?: (id: string | null) => void;
  /** Adds a "None" item. Default false. */
  allowNone?: boolean;
  /** Default "None". */
  noneLabel?: string;
  /** Shows "Finding devices…". Default false. */
  loading?: boolean;
  /** Microphone permission. Default `granted`. */
  permission?: "granted" | "prompt" | "denied";
  onRequestPermission?: () => void;
  disabled?: boolean;
  children?: ReactNode;
}

export const AudioDeviceSelect = ({
  devices,
  value: valueProp,
  defaultValue = null,
  onValueChange,
  allowNone = false,
  noneLabel = "None",
  loading = false,
  permission = "granted",
  onRequestPermission,
  disabled,
  children,
}: AudioDeviceSelectProps) => {
  const [uncontrolled, setUncontrolled] = useState<string | null>(defaultValue);
  const value = valueProp === undefined ? uncontrolled : valueProp;
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  if (devices.some((device) => knownLabels[device.id] !== device.label)) {
    setKnownLabels((previous) => {
      const next = { ...previous };
      for (const device of devices) {
        next[device.id] = device.label;
      }
      return next;
    });
  }

  const missing =
    value !== null &&
    !loading &&
    !devices.some((device) => device.id === value);

  const items = useMemo<DeviceItem[]>(() => {
    const list: DeviceItem[] = [];
    if (allowNone) {
      list.push({ label: noneLabel, none: true, value: NONE_VALUE });
    }
    for (const device of devices) {
      list.push({ device, label: device.label, value: device.id });
    }
    if (missing && value !== null) {
      const remembered = knownLabels[value] ?? "Unknown device";
      list.push({
        label: `${remembered} (disconnected)`,
        missing: true,
        value,
      });
    }
    return list;
  }, [allowNone, devices, knownLabels, missing, noneLabel, value]);

  let selected: string | null = value;
  if (value === null && allowNone) {
    selected = NONE_VALUE;
  }

  const contextValue = useMemo<AudioDeviceSelectContextValue>(
    () => ({ items, loading, missing, onRequestPermission, permission }),
    [items, loading, missing, onRequestPermission, permission]
  );

  return (
    <AudioDeviceSelectContext.Provider value={contextValue}>
      <Select<string | null>
        disabled={disabled}
        onValueChange={(next) => {
          const id = next === NONE_VALUE ? null : next;
          if (valueProp === undefined) {
            setUncontrolled(id);
          }
          onValueChange?.(id);
        }}
        value={selected}
      >
        {children ?? (
          <>
            <AudioDeviceSelectTrigger>
              <AudioDeviceSelectValue />
            </AudioDeviceSelectTrigger>
            <AudioDeviceSelectContent />
          </>
        )}
      </Select>
    </AudioDeviceSelectContext.Provider>
  );
};
