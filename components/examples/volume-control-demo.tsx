"use client";

import {
  SpeakerHighIcon,
  SpeakerLowIcon,
  SpeakerNoneIcon,
  SpeakerXIcon,
} from "@phosphor-icons/react";

import {
  VolumeControl,
  VolumeControlMute,
  VolumeControlSlider,
  VolumeControlValue,
} from "@/components/ui/volume-control";

const VolumeControlDemo = () => (
  <VolumeControl className="w-full max-w-xs" defaultValue={0.6}>
    <VolumeControlMute className="group/mute">
      <SpeakerXIcon className="hidden group-data-[level=muted]/mute:block" />
      <SpeakerNoneIcon className="hidden group-data-[level=low]/mute:block" />
      <SpeakerLowIcon className="hidden group-data-[level=medium]/mute:block" />
      <SpeakerHighIcon className="hidden group-data-[level=high]/mute:block" />
    </VolumeControlMute>
    <VolumeControlSlider />
    <VolumeControlValue />
  </VolumeControl>
);

export default VolumeControlDemo;
