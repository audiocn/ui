"use client";

import { SoundPad, SoundPadGrid, SoundPadLabel, SoundPadProgress, SoundPadShortcut } from "@/components/ui/sound-pad";
import type { SoundPadMode } from "@/components/ui/sound-pad";
import { useSound } from "@/hooks/use-sound";
import { useDemoSounds } from "@/lib/docs/use-demo-audio";
import type { DemoSoundSource } from "@/lib/docs/use-demo-audio";

const MODES: Record<string, SoundPadMode> = { drumroll: "hold", whoosh: "toggle" };

const Pad = ({ sound }: { sound: DemoSoundSource }) => {
  const mode = MODES[sound.id] ?? "one-shot";
  const player = useSound(sound.src, { loop: mode !== "one-shot" });
  return (
    <SoundPad
      accent={sound.accent}
      hotkey={sound.hotkey}
      loading={!player.isLoaded}
      mode={mode}
      onStop={player.stop}
      onTrigger={player.play}
      playing={player.isPlaying}
    >
      <SoundPadLabel>{sound.label}</SoundPadLabel>
      <SoundPadShortcut />
      <SoundPadProgress source={player.progress} variant={mode === "one-shot" ? "bar" : "ring"} />
    </SoundPad>
  );
};

const SoundPadGridDemo = () => {
  const sounds = useDemoSounds();
  return (
    <SoundPadGrid className="w-full max-w-lg" columns={4} hotkeys>
      {sounds.map((sound) => (
        <Pad key={sound.id} sound={sound} />
      ))}
    </SoundPadGrid>
  );
};

export default SoundPadGridDemo;
