"use client";

import {
  SpeakerHighIcon,
  SpeakerLowIcon,
  SpeakerNoneIcon,
  SpeakerXIcon,
} from "@phosphor-icons/react";

import {
  AudioDeviceSelect,
  AudioDeviceSelectContent,
  AudioDeviceSelectTrigger,
  AudioDeviceSelectValue,
} from "@/components/ui/audio-device-select";
import type { AudioDevice } from "@/components/ui/audio-device-select";
import {
  VolumeControl,
  VolumeControlMute,
  VolumeControlSlider,
  VolumeControlValue,
} from "@/components/ui/volume-control";

const devices: AudioDevice[] = [
  { id: "default", isDefault: true, label: "MacBook Pro Speakers" },
  { description: "USB", id: "interface", label: "Scarlett 2i2" },
  { description: "Bluetooth", id: "headphones", label: "AirPods Pro" },
  {
    description: "Disconnected",
    id: "monitors",
    label: "Studio Display",
    status: "unavailable",
  },
];

const OutputTile = () => (
  <div className="flex w-full flex-col gap-4">
    <AudioDeviceSelect defaultValue="interface" devices={devices}>
      <AudioDeviceSelectTrigger aria-label="Output device">
        <AudioDeviceSelectValue />
      </AudioDeviceSelectTrigger>
      <AudioDeviceSelectContent />
    </AudioDeviceSelect>
    <VolumeControl className="w-full" defaultValue={0.7}>
      <VolumeControlMute className="group/mute">
        <SpeakerXIcon className="hidden group-data-[level=muted]/mute:block" />
        <SpeakerNoneIcon className="hidden group-data-[level=low]/mute:block" />
        <SpeakerLowIcon className="hidden group-data-[level=medium]/mute:block" />
        <SpeakerHighIcon className="hidden group-data-[level=high]/mute:block" />
      </VolumeControlMute>
      <VolumeControlSlider />
      <VolumeControlValue />
    </VolumeControl>
  </div>
);

export default OutputTile;
