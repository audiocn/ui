"use client";

import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LiveWaveformTile = () => {
  const music = useDemoSignal({ kind: "music", seed: 6 });
  const voice = useDemoSignal({ historySize: 120, kind: "speech", seed: 6 });

  return (
    <div className="flex w-full flex-col gap-3">
      <LiveWaveform
        aria-label="Music oscilloscope"
        className="text-primary h-16"
        source={music.visual}
        variant="line"
      />
      <LiveWaveform
        aria-label="Voice level history"
        className="h-12 [--waveform:var(--meter-ok)]"
        mode="scrolling"
        source={voice.visual}
      />
    </div>
  );
};

export default LiveWaveformTile;
