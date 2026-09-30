"use client";

import { DbReadout } from "@/components/ui/db-readout";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const DbReadoutDemo = () => {
  const signal = useDemoSignal({ kind: "speech" });

  return (
    <div className="grid grid-cols-[auto_auto] items-center gap-x-6 gap-y-2 text-sm">
      <span className="text-muted-foreground">Peak</span>
      <DbReadout source={signal.meter} />
      <span className="text-muted-foreground">RMS</span>
      <DbReadout measure="rms" source={signal.meter} />
      <span className="text-muted-foreground">Peak, held 1 s</span>
      <DbReadout holdMs={1000} source={signal.meter} />
      <span className="text-muted-foreground">Value</span>
      <DbReadout value={-6} />
    </div>
  );
};

export default DbReadoutDemo;
