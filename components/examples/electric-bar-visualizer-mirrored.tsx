"use client";

import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const ElectricBarVisualizerMirrored = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 8 });

  return (
    <ElectricBarVisualizer
      aria-label="Assistant speaking"
      barCount={21}
      barGap={3}
      barWidth={4}
      className="text-foreground h-28 max-w-md"
      mirrored
      source={signal.visual}
    />
  );
};

export default ElectricBarVisualizerMirrored;
