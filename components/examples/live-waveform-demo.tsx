"use client";

import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LiveWaveformDemo = () => {
  const signal = useDemoSignal({ historySize: 120, kind: "speech" });

  return (
    <LiveWaveform
      aria-label="Microphone waveform"
      className="h-20 max-w-md"
      mode="scrolling"
      source={signal.visual}
    />
  );
};

export default LiveWaveformDemo;
