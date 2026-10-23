"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
} from "@/components/ui/lcd-display";
import { LcdVuMeter } from "@/components/ui/lcd-vumeter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdVuMeterModes = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 3 });

  return (
    <LcdDisplay className="w-full max-w-md" variant="green">
      <LcdDisplayRow>
        <LcdDisplayLabel>Bar</LcdDisplayLabel>
        <LcdVuMeter source={signal.meter} />
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdDisplayLabel>Dot</LcdDisplayLabel>
        <LcdVuMeter mode="dot" source={signal.meter} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdVuMeterModes;
