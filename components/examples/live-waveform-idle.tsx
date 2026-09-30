"use client";

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LiveWaveformIdle = () => {
  const [active, setActive] = useState(false);
  const signal = useDemoSignal({ historySize: 120, kind: "speech" });

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <LiveWaveform
        active={active}
        aria-label="Recording preview"
        className="h-16 rounded-lg border bg-muted/20"
        mode="scrolling"
        source={signal.visual}
      />
      <Button className="self-start" onClick={() => setActive(!active)} size="sm" variant="outline">
        {active ? "Pause" : "Listen"}
      </Button>
    </div>
  );
};

export default LiveWaveformIdle;
