"use client";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const BarVisualizerMirrored = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 8 });

  return (
    <BarVisualizer
      aria-label="Assistant speaking"
      barCount={31}
      className="text-foreground h-28 max-w-md [--bar-gap:2px] [--bar-width:4px]"
      mirrored
      source={signal.visual}
    />
  );
};

export default BarVisualizerMirrored;
