"use client";

import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

/** The live waveform between the hero and the showcase, spreading from the middle. */
export const HeroWaveform = () => {
  const signal = useDemoSignal({
    historyIntervalMs: 40,
    historySize: 200,
    kind: "speech",
    seed: 3,
  });

  return (
    <LiveWaveform
      aria-hidden
      className="text-foreground/20 h-12 w-full"
      mode="scrolling"
      source={signal.visual}
      variant="mirror"
    />
  );
};
