"use client";

import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const ElectricBarVisualizerDemo = () => {
  const signal = useDemoSignal({ kind: "speech" });

  return (
    <ElectricBarVisualizer
      aria-label="Voice activity"
      className="text-primary h-28 max-w-sm"
      source={signal.visual}
    />
  );
};

export default ElectricBarVisualizerDemo;
