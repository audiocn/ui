"use client";

import { SmoothWaveform } from "@/components/ui/smooth-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const modes = ["wave", "scope"] as const;

const SmoothWaveformModes = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 4 });

  return (
    <div className="grid w-full max-w-lg gap-6">
      {modes.map((mode) => (
        <div className="grid gap-2" key={mode}>
          <SmoothWaveform
            aria-label={`Voice, ${mode}`}
            className="text-primary h-24"
            mode={mode}
            source={signal.visual}
          />
          <span className="text-muted-foreground text-center font-mono text-xs">
            {mode}
          </span>
        </div>
      ))}
    </div>
  );
};

export default SmoothWaveformModes;
