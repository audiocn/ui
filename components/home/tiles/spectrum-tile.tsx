"use client";

import { Spectrum } from "@/components/ui/spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const SpectrumTile = () => {
  const signal = useDemoSignal({ bands: 64, kind: "music", seed: 5 });

  return (
    <Spectrum
      aria-label="Music spectrum"
      className="h-32 w-full"
      peakHold
      source={signal.visual}
      variant="area"
    />
  );
};

export default SpectrumTile;
