"use client";

import { DbReadout } from "@/components/ui/db-readout";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const DbReadoutZones = () => {
  const signal = useDemoSignal({ kind: "music", seed: 6 });

  return (
    <span className="bg-muted/40 has-data-[zone=clip]:bg-meter-clip/15 rounded-md px-2 py-1">
      <DbReadout
        className="data-[zone=clip]:text-meter-clip-foreground data-[zone=warn]:text-meter-warn-foreground data-silent:text-muted-foreground text-2xl font-semibold"
        holdMs={500}
        source={signal.meter}
      />
    </span>
  );
};

export default DbReadoutZones;
