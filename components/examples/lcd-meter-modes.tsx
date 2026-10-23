"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
} from "@/components/ui/lcd-display";
import { LcdMeter } from "@/components/ui/lcd-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdMeterModes = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 3 });

  return (
    <LcdDisplay className="w-full max-w-md" variant="green">
      <LcdDisplayRow>
        <LcdDisplayLabel>Bar</LcdDisplayLabel>
        <LcdMeter source={signal.meter} />
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdDisplayLabel>Dot</LcdDisplayLabel>
        <LcdMeter mode="dot" source={signal.meter} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdMeterModes;
