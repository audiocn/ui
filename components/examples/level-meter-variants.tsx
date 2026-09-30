"use client";

import { LevelMeter } from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const variants = ["solid", "segmented", "gradient"] as const;

const LevelMeterVariants = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music", seed: 2 });

  return (
    <div className="grid w-full max-w-md gap-5">
      {variants.map((variant) => (
        <div className="grid gap-1.5" key={variant}>
          <span className="text-muted-foreground text-xs">{variant}</span>
          <LevelMeter
            aria-label={`${variant} meter`}
            size="lg"
            source={signal.meter}
            variant={variant}
          />
        </div>
      ))}
    </div>
  );
};

export default LevelMeterVariants;
