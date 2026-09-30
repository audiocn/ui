"use client";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const alignments = ["center", "end", "start"] as const;

const BarVisualizerAlign = () => {
  const signal = useDemoSignal({ kind: "music" });

  return (
    <div className="grid w-full max-w-md grid-cols-3 gap-6">
      {alignments.map((align) => (
        <div className="grid gap-2" key={align}>
          <BarVisualizer
            align={align}
            aria-label={`Bars aligned to ${align}`}
            barCount={12}
            className="h-20 rounded-lg bg-muted/40 p-2"
            source={signal.visual}
          />
          <span className="text-center font-mono text-muted-foreground text-xs">
            {align}
          </span>
        </div>
      ))}
    </div>
  );
};

export default BarVisualizerAlign;
