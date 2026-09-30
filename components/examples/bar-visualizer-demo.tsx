"use client";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const BarVisualizerDemo = () => {
  const signal = useDemoSignal({ kind: "speech" });

  return (
    <BarVisualizer
      aria-label="Voice activity"
      className="h-24 max-w-sm text-primary"
      source={signal.visual}
    />
  );
};

export default BarVisualizerDemo;
