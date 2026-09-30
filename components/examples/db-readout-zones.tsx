"use client";

import { DbReadout } from "@/components/ui/db-readout";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const DbReadoutZones = () => {
  const signal = useDemoSignal({ kind: "music", seed: 6 });

  return (
    <DbReadout
      className="rounded-md px-2 py-1 font-semibold text-2xl transition-colors data-[zone=clip]:bg-meter-clip/15 data-[zone=clip]:text-meter-clip data-[zone=warn]:text-meter-warn data-silent:text-muted-foreground"
      holdMs={500}
      source={signal.meter}
    />
  );
};

export default DbReadoutZones;
