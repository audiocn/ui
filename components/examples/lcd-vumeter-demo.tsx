"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdVuMeter,
  LcdVuMeterChannel,
  LcdVuMeterChannels,
  LcdVuMeterClip,
  LcdVuMeterScale,
  LcdVuMeterValue,
} from "@/components/ui/lcd-vumeter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdVuMeterDemo = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music" });

  return (
    <LcdDisplay className="w-full max-w-md">
      <LcdDisplayRow>
        <LcdDisplayLabel>Out</LcdDisplayLabel>
        <LcdVuMeter source={signal.meter}>
          <LcdVuMeterChannels>
            <LcdVuMeterChannel index={0} />
            <LcdVuMeterChannel index={1} />
            <LcdVuMeterScale />
          </LcdVuMeterChannels>
          <LcdVuMeterValue />
          <LcdVuMeterClip />
        </LcdVuMeter>
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdVuMeterDemo;
