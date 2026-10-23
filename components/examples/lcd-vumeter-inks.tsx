"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import { LcdVuMeter } from "@/components/ui/lcd-vumeter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const INKS = [
  { label: "Bus", variant: "amber" },
  { label: "Input", variant: "green" },
  { label: "Send", variant: "cyan" },
  { label: "Rec", variant: "red" },
] as const;

const LcdVuMeterInks = () => {
  const signal = useDemoSignal({ kind: "music", seed: 6 });

  return (
    <div className="grid w-full max-w-xl gap-4 sm:grid-cols-2">
      {INKS.map((ink) => (
        <LcdDisplay key={ink.variant} size="sm" variant={ink.variant}>
          <LcdDisplayRow>
            <LcdDisplayLabel>{ink.label}</LcdDisplayLabel>
            <LcdVuMeter segments={16} source={signal.meter} />
          </LcdDisplayRow>
        </LcdDisplay>
      ))}
    </div>
  );
};

export default LcdVuMeterInks;
