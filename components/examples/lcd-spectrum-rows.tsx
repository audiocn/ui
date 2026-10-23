"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
} from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdSpectrumRows = () => {
  const signal = useDemoSignal({ bands: 32, kind: "music", seed: 2 });

  return (
    <LcdDisplay className="w-full max-w-xl" matrix={false}>
      <LcdDisplayRow>
        <LcdDisplayLabel>12</LcdDisplayLabel>
        <LcdSpectrum className="h-28" rows={12} source={signal.visual} />
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdDisplayLabel>24</LcdDisplayLabel>
        <LcdSpectrum className="h-28" source={signal.visual} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdSpectrumRows;
