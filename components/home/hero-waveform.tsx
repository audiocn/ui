"use client";

import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

// Enough entries to fill the widest divider: 1232px of 4px bars.
const HISTORY_SIZE = 320;

/** The live waveform between the hero and the showcase, scrolling left. */
export const HeroWaveform = () => {
  const signal = useDemoSignal({
    historyIntervalMs: 40,
    historySize: HISTORY_SIZE,
    kind: "speech",
    prefill: true,
    seed: 3,
  });

  return (
    <LiveWaveform
      aria-hidden
      className="text-foreground/20 h-12 w-full"
      mode="scrolling"
      source={signal.visual}
    />
  );
};
