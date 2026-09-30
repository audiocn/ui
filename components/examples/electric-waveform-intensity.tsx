"use client";

import { ElectricWaveform } from "@/components/ui/electric-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const settings = [
  { arcs: false, intensity: 0.15, label: "calm", sparks: false },
  { arcs: true, intensity: 0.6, label: "default", sparks: true },
  { arcs: true, intensity: 1, label: "charged", sparks: true },
] as const;

const ElectricWaveformIntensity = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 6 });

  return (
    <div className="grid w-full max-w-lg gap-4">
      {settings.map((setting) => (
        <div className="grid gap-1" key={setting.label}>
          <ElectricWaveform
            arcs={setting.arcs}
            aria-label={`Voice, ${setting.label}`}
            className="text-primary h-20"
            intensity={setting.intensity}
            source={signal.visual}
            sparks={setting.sparks}
          />
          <span className="text-muted-foreground text-center font-mono text-xs">
            {setting.label}
          </span>
        </div>
      ))}
    </div>
  );
};

export default ElectricWaveformIntensity;
