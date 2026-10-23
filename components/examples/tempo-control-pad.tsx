"use client";

import { useState } from "react";

import { SoundPad, SoundPadLabel } from "@/components/ui/sound-pad";
import { TempoControl, TempoControlTap } from "@/components/ui/tempo-control";

const TempoControlPadDemo = () => {
  const [bpm, setBpm] = useState(128);

  return (
    <TempoControl className="w-36 flex-col" onValueChange={setBpm} value={bpm}>
      <output
        aria-label="Tempo"
        aria-live="off"
        className="font-mono text-lg tabular-nums"
      >
        {bpm} BPM
      </output>
      <TempoControlTap
        render={(props) => <SoundPad {...props} className="w-full" />}
      >
        <SoundPadLabel>Tap tempo</SoundPadLabel>
      </TempoControlTap>
    </TempoControl>
  );
};

export default TempoControlPadDemo;
