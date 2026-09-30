"use client";

import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const settings = [
  { arcs: false, intensity: 0.2, label: "calm", sparks: false },
  { arcs: true, intensity: 0.6, label: "default", sparks: true },
  { arcs: true, intensity: 1, label: "charged", sparks: true },
] as const;

const ElectricBarVisualizerIntensity = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 5 });

  return (
    <div className="grid w-full max-w-md gap-6 sm:grid-cols-3">
      {settings.map((setting) => (
        <div className="grid gap-2" key={setting.label}>
          <ElectricBarVisualizer
            arcs={setting.arcs}
            aria-label={`Visualizer, ${setting.label}`}
            barCount={10}
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

export default ElectricBarVisualizerIntensity;
