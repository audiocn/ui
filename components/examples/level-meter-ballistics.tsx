"use client";

import { LevelMeter } from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const presets = ["peak", "vu", "instant"] as const;

const LevelMeterBallistics = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 3 });

  return (
    <div className="grid w-full max-w-md gap-4">
      {presets.map((preset) => (
        <div className="grid grid-cols-[4rem_1fr] items-center gap-3" key={preset}>
          <span className="font-mono text-muted-foreground text-xs">{preset}</span>
          <LevelMeter
            aria-label={`${preset} ballistics`}
            ballistics={preset}
            source={signal.meter}
          />
        </div>
      ))}
    </div>
  );
};

export default LevelMeterBallistics;
