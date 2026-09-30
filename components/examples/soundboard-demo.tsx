"use client";

import { Soundboard } from "@/components/blocks/soundboard/soundboard";
import { useDemoSounds } from "@/lib/docs/use-demo-audio";

const SoundboardDemo = () => {
  const sounds = useDemoSounds();
  if (sounds.length === 0) {
    return <p className="text-muted-foreground text-sm">Preparing sounds{"…"}</p>;
  }
  return <Soundboard className="w-full max-w-2xl" defaultSounds={sounds} />;
};

export default SoundboardDemo;
