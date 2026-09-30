"use client";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import type { AudioDevice } from "@/components/ui/audio-device-select";

const devices: AudioDevice[] = [
  { id: "default", isDefault: true, label: "MacBook Pro Microphone" },
  { description: "USB", id: "usb", label: "Shure MV7+" },
  { description: "In use by another app", id: "busy", label: "Elgato Wave:3", status: "unavailable" },
];

const AudioDeviceSelectStates = () => (
  <div className="grid w-full max-w-sm gap-4">
    <AudioDeviceSelect defaultValue="usb" devices={devices} />
    <AudioDeviceSelect allowNone devices={devices} noneLabel="No microphone" />
    <AudioDeviceSelect defaultValue="rode" devices={devices} />
    <AudioDeviceSelect devices={[]} loading />
  </div>
);

export default AudioDeviceSelectStates;
