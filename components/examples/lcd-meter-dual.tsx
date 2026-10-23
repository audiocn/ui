"use client";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterBar,
  LcdMeterChannel,
  LcdMeterChannels,
  LcdMeterHold,
  LcdMeterScale,
  LcdMeterTrack,
} from "@/components/ui/lcd-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdMeterDual = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 7 });

  return (
    <LcdDisplay className="w-full max-w-md">
      <LcdDisplayRow>
        <LcdDisplayLabel>Voice</LcdDisplayLabel>
        <LcdMeter size="lg" source={signal.meter}>
          <LcdMeterChannels>
            <LcdMeterChannel>
              <LcdMeterTrack>
                <LcdMeterBar className="opacity-40" measure="peak" />
                <LcdMeterBar measure="rms" />
                <LcdMeterHold />
              </LcdMeterTrack>
            </LcdMeterChannel>
            <LcdMeterScale />
          </LcdMeterChannels>
        </LcdMeter>
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdMeterDual;
