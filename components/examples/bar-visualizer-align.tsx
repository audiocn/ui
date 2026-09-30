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
          <div className="bg-muted/40 rounded-lg p-2">
            <BarVisualizer
              align={align}
              aria-label={`Bars aligned to ${align}`}
              barCount={12}
              className="h-16"
              source={signal.visual}
            />
          </div>
          <span className="text-muted-foreground text-center font-mono text-xs">
            {align}
          </span>
        </div>
      ))}
    </div>
  );
};

export default BarVisualizerAlign;
