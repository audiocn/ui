"use client";

import { MegaphoneIcon } from "@phosphor-icons/react";

import {
  SoundPad,
  SoundPadIcon,
  SoundPadLabel,
  SoundPadProgress,
  SoundPadShortcut,
} from "@/components/ui/sound-pad";
import { useSound } from "@/hooks/use-sound";
import { useDemoSounds } from "@/lib/docs/use-demo-audio";

const SoundPadDemo = () => {
  const sounds = useDemoSounds();
  const airhorn = useSound(sounds[0]?.src ?? null);

  return (
    <SoundPad
      accent="oklch(0.7 0.2 30)"
      className="w-40"
      hotkey="1"
      loading={!airhorn.isLoaded}
      onTrigger={airhorn.play}
      playing={airhorn.isPlaying}
    >
      <SoundPadIcon>
        <MegaphoneIcon />
      </SoundPadIcon>
      <SoundPadLabel>Airhorn</SoundPadLabel>
      <SoundPadShortcut />
      <SoundPadProgress source={airhorn.progress} />
    </SoundPad>
  );
};

export default SoundPadDemo;
