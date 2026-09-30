"use client";

import { MicrophoneIcon } from "@phosphor-icons/react";

import { Badge } from "@/components/ui/badge";
import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const BarVisualizerMini = () => {
  const signal = useDemoSignal({ kind: "speech", seed: 12 });

  return (
    <Badge className="h-7 gap-2 px-3" variant="secondary">
      <MicrophoneIcon />
      Live
      <BarVisualizer
        aria-hidden
        barCount={5}
        className="h-3.5 w-7 [--bar-gap:2px] [--bar-width:3px]"
        minLevel={0.15}
        source={signal.visual}
      />
    </Badge>
  );
};

export default BarVisualizerMini;
