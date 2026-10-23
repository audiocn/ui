"use client";

import { LcdDisplay } from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterChannel,
  LcdMeterChannels,
  LcdMeterScale,
} from "@/components/ui/lcd-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdMeterVertical = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music", seed: 4 });

  return (
    <LcdDisplay variant="amber">
      <LcdMeter
        aria-label="Program level"
        className="h-48"
        orientation="vertical"
        segments={24}
        size="lg"
        source={signal.meter}
      >
        <LcdMeterChannels>
          <LcdMeterChannel index={0} />
          <LcdMeterChannel index={1} />
          <LcdMeterScale />
        </LcdMeterChannels>
      </LcdMeter>
    </LcdDisplay>
  );
};

export default LcdMeterVertical;
