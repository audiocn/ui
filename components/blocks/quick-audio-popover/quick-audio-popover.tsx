"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { useState } from "react";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useAudioDevices } from "@/hooks/use-audio-devices";
import { useMicrophone } from "@/hooks/use-microphone";
import { useSystemAudio } from "@/hooks/use-system-audio";

export interface QuickAudioPopoverProps {
  /** Extra content at the bottom, such as a link to full audio settings. */
  children?: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  align?: "start" | "center" | "end";
}

export const QuickAudioPopover = ({
  children,
  side = "bottom",
  align = "center",
}: QuickAudioPopoverProps) => {
  const devices = useAudioDevices();
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [muted, setMuted] = useState(false);
  const microphone = useMicrophone({ deviceId });
  const analyser = useAudioAnalyser(microphone.stream, { enabled: !muted });
  const system = useSystemAudio();
  const live = microphone.status === "active" && !muted;

  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            aria-label={live ? "Audio: microphone live" : "Audio settings"}
            variant="outline"
          />
        }
      >
        {live ? (
          <MicrophoneIcon data-icon="inline-start" />
        ) : (
          <MicrophoneSlashIcon data-icon="inline-start" />
        )}
        <BarVisualizer
          aria-hidden
          barCount={5}
          className="text-foreground h-4 w-8 [--bar-gap:2px] [--bar-width:3px]"
          minLevel={0.15}
          source={live ? analyser.visual : null}
        />
      </PopoverTrigger>
      <PopoverContent align={align} className="w-80" side={side}>
        <PopoverHeader>
          <PopoverTitle>Audio</PopoverTitle>
          <PopoverDescription>Microphone and system audio.</PopoverDescription>
        </PopoverHeader>
        <FieldGroup>
          <Field>
            <FieldLabel>Microphone</FieldLabel>
            <AudioDeviceSelect
              devices={devices.devices}
              loading={devices.isLoading}
              onRequestPermission={devices.requestPermission}
              onValueChange={setDeviceId}
              permission={
                devices.permission === "unsupported"
                  ? "denied"
                  : devices.permission
              }
              value={deviceId}
            />
          </Field>
          <div className="flex gap-2">
            <Button
              className="flex-1"
              onClick={
                microphone.status === "active"
                  ? microphone.stop
                  : microphone.start
              }
              size="sm"
              variant="outline"
            >
              {microphone.status === "active" ? "Turn off" : "Turn on"}
            </Button>
            <Button
              className="flex-1"
              disabled={microphone.status !== "active"}
              onClick={() => setMuted(!muted)}
              size="sm"
              variant={muted ? "destructive" : "outline"}
            >
              {muted ? "Unmute" : "Mute"}
            </Button>
          </div>
          <Separator />
          <Field orientation="horizontal">
            <FieldLabel htmlFor="quick-system-audio">System audio</FieldLabel>
            <Switch
              checked={system.status === "active"}
              disabled={!system.isSupported}
              id="quick-system-audio"
              onCheckedChange={(checked) =>
                checked ? system.start() : system.stop()
              }
            />
          </Field>
          {children}
        </FieldGroup>
      </PopoverContent>
    </Popover>
  );
};
