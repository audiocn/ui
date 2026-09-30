"use client";

import { SystemAudioMixer } from "@/components/blocks/system-audio-mixer/system-audio-mixer";
import { useDemoSounds, useDemoTracks } from "@/lib/docs/use-demo-audio";

const SystemAudioMixerConsole = () => {
  const tracks = useDemoTracks();
  const sounds = useDemoSounds();
  return (
    <SystemAudioMixer className="h-[30rem] w-full" defaultOrientation="vertical" sounds={sounds} tracks={tracks} />
  );
};

export default SystemAudioMixerConsole;
