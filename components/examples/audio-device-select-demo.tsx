"use client";

import { useState } from "react";

import {
  AudioDeviceSelect,
  AudioDeviceSelectContent,
  AudioDeviceSelectPreview,
  AudioDeviceSelectTrigger,
  AudioDeviceSelectValue,
} from "@/components/ui/audio-device-select";
import { LiveWaveform } from "@/components/ui/live-waveform";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useAudioDevices } from "@/hooks/use-audio-devices";
import { useMicrophone } from "@/hooks/use-microphone";

const AudioDeviceSelectDemo = () => {
  const { devices, isLoading, permission, requestPermission } =
    useAudioDevices();
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const microphone = useMicrophone({ deviceId, enabled: deviceId !== null });
  const analyser = useAudioAnalyser(microphone.stream, { historySize: 120 });

  return (
    <div className="flex w-full max-w-sm flex-col gap-2">
      <AudioDeviceSelect
        devices={devices}
        loading={isLoading}
        onRequestPermission={requestPermission}
        onValueChange={setDeviceId}
        permission={permission === "unsupported" ? "denied" : permission}
        value={deviceId}
      >
        <AudioDeviceSelectTrigger>
          <AudioDeviceSelectValue placeholder="Select a microphone" />
        </AudioDeviceSelectTrigger>
        <AudioDeviceSelectContent />
      </AudioDeviceSelect>
      <AudioDeviceSelectPreview>
        <LiveWaveform
          active={microphone.status === "active"}
          aria-label="Microphone preview"
          barWidth={2}
          className="h-8"
          mode="scrolling"
          source={analyser.visual}
        />
      </AudioDeviceSelectPreview>
    </div>
  );
};

export default AudioDeviceSelectDemo;
