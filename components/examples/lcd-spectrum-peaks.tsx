"use client";

import { LcdDisplay } from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdSpectrumPeaks = () => {
  const signal = useDemoSignal({ bands: 32, kind: "speech", seed: 4 });
  return (
    <LcdDisplay className="w-full max-w-xl" matrix={false} variant="amber">
      <LcdSpectrum className="h-36" peakHold source={signal.visual} />
    </LcdDisplay>
  );
};

export default LcdSpectrumPeaks;
