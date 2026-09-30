"use client";

import { Spectrum } from "@/components/ui/spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const SpectrumDemo = () => {
  const signal = useDemoSignal({ bands: 48, kind: "music" });
  return (
    <Spectrum
      className="max-w-lg"
      minDb={-60}
      maxDb={0}
      peakHold
      source={signal.visual}
    />
  );
};

export default SpectrumDemo;
