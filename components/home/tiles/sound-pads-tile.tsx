"use client";

import { Skeleton } from "@/components/ui/skeleton";
import {
  SoundPad,
  SoundPadGrid,
  SoundPadLabel,
  SoundPadProgress,
} from "@/components/ui/sound-pad";
import type { SoundPadMode } from "@/components/ui/sound-pad";
import { useSound } from "@/hooks/use-sound";
import { useDemoSounds } from "@/lib/docs/use-demo-audio";
import type { DemoSoundSource } from "@/lib/docs/use-demo-audio";

const MODES: Record<string, SoundPadMode> = {
  drumroll: "hold",
  whoosh: "toggle",
};

const Pad = ({ sound }: { sound: DemoSoundSource }) => {
  const mode = MODES[sound.id] ?? "one-shot";
  const player = useSound(sound.src, { loop: mode !== "one-shot" });
  return (
    <SoundPad
      accent={sound.accent}
      loading={!player.isLoaded}
      mode={mode}
      onStop={() => player.stop()}
      onTrigger={() => player.play()}
      playing={player.isPlaying}
    >
      <SoundPadLabel>{sound.label}</SoundPadLabel>
      <SoundPadProgress
        source={player.progress}
        variant={mode === "one-shot" ? "bar" : "ring"}
      />
    </SoundPad>
  );
};

// Hotkeys stay off on the home page, so the pads never take keys from it.
const SoundPadsTile = () => {
  const sounds = useDemoSounds();
  if (sounds.length === 0) {
    return <Skeleton className="h-76 w-full @sm:h-50" />;
  }
  return (
    <SoundPadGrid className="w-full" columns={4}>
      {sounds.map((sound) => (
        <Pad key={sound.id} sound={sound} />
      ))}
    </SoundPadGrid>
  );
};

export default SoundPadsTile;
