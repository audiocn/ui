"use client";

import { LevelMeter, LevelMeterChannel } from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LevelMeterCssLevel = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 5 });

  return (
    <LevelMeter
      aria-label="Speaking indicator"
      className="w-auto"
      source={signal.meter}
    >
      <LevelMeterChannel className="relative size-20 items-center justify-center">
        <span className="bg-primary/15 absolute size-20 scale-[calc(0.6_+_var(--meter-level)_*_0.6)] rounded-full" />
        <span className="bg-primary/30 absolute size-14 scale-[calc(0.8_+_var(--meter-level)_*_0.4)] rounded-full" />
        <span className="bg-primary relative size-10 rounded-full" />
      </LevelMeterChannel>
    </LevelMeter>
  );
};

export default LevelMeterCssLevel;
