"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterChannel,
  LcdMeterChannels,
  LcdMeterClip,
  LcdMeterScale,
  LcdMeterValue,
} from "@/components/ui/lcd-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdMeterDemo = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music" });

  return (
    <LcdDisplay className="w-full max-w-md">
      <LcdDisplayRow>
        <LcdDisplayLabel>Out</LcdDisplayLabel>
        <LcdMeter source={signal.meter}>
          <LcdMeterChannels>
            <LcdMeterChannel index={0} />
            <LcdMeterChannel index={1} />
            <LcdMeterScale />
          </LcdMeterChannels>
          <LcdMeterValue />
          <LcdMeterClip />
        </LcdMeter>
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdMeterDemo;
