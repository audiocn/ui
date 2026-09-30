"use client";

import { SystemAudioMixer } from "@/components/blocks/system-audio-mixer/system-audio-mixer";
import { useDemoSounds, useDemoTracks } from "@/lib/docs/use-demo-audio";

const SystemAudioMixerDemo = () => {
  const tracks = useDemoTracks();
  const sounds = useDemoSounds();
  return (
    <SystemAudioMixer className="w-full" sounds={sounds} tracks={tracks} />
  );
};

export default SystemAudioMixerDemo;
