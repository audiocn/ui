"use client";

import { LcdDisplay, LcdDisplayStatus } from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdSpectrumDemo = () => {
  const signal = useDemoSignal({ bands: 32, kind: "music" });
  return (
    <LcdDisplay className="w-full max-w-xl" matrix={false}>
      <LcdSpectrum className="h-40 w-full" peakHold source={signal.visual} />
      <LcdDisplayStatus />
    </LcdDisplay>
  );
};

export default LcdSpectrumDemo;
