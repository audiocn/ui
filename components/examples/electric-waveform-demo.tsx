"use client";

import { ElectricWaveform } from "@/components/ui/electric-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const ElectricWaveformDemo = () => {
  const signal = useDemoSignal({ kind: "speech" });

  return (
    <ElectricWaveform
      aria-label="Voice activity"
      className="text-primary h-32 max-w-lg"
      source={signal.visual}
    />
  );
};

export default ElectricWaveformDemo;
