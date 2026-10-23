"use client";

import { LcdDisplay } from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const INKS = ["amber", "green", "cyan", "red"] as const;

const LcdSpectrumInks = () => {
  const signal = useDemoSignal({ bands: 24, kind: "music", seed: 6 });

  return (
    <div className="grid w-full max-w-xl gap-4 sm:grid-cols-2">
      {INKS.map((ink) => (
        <LcdDisplay key={ink} matrix={false} size="sm" variant={ink}>
          <LcdSpectrum className="h-24" rows={12} source={signal.visual} />
        </LcdDisplay>
      ))}
    </div>
  );
};

export default LcdSpectrumInks;
