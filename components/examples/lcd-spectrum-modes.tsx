"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
} from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdSpectrumModes = () => {
  const signal = useDemoSignal({ bands: 32, kind: "speech" });

  return (
    <LcdDisplay className="w-full max-w-xl" matrix={false} variant="green">
      <LcdDisplayRow>
        <LcdDisplayLabel>Bar</LcdDisplayLabel>
        <LcdSpectrum className="h-28" source={signal.visual} />
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdDisplayLabel>Dot</LcdDisplayLabel>
        <LcdSpectrum className="h-28" mode="dot" source={signal.visual} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdSpectrumModes;
