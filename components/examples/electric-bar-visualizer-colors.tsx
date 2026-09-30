"use client";

import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const colors = [
  { className: "text-primary", label: "primary" },
  { className: "text-meter-ok", label: "meter-ok" },
  { className: "text-channel-solo", label: "channel-solo" },
  { className: "text-destructive", label: "destructive" },
] as const;

const ElectricBarVisualizerColors = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 3 });

  return (
    <div className="grid w-full max-w-lg grid-cols-2 gap-6 sm:grid-cols-4">
      {colors.map((color) => (
        <div className="grid gap-2" key={color.label}>
          <ElectricBarVisualizer
            align="end"
            aria-label={`Bars in ${color.label}`}
            barCount={9}
            className={`h-20 ${color.className}`}
            source={signal.visual}
          />
          <span className="text-muted-foreground text-center font-mono text-xs">
            {color.label}
          </span>
        </div>
      ))}
    </div>
  );
};

export default ElectricBarVisualizerColors;
