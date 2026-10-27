"use client";

import { useState } from "react";

import {
  TempoControl,
  TempoControlInput,
  TempoControlTap,
} from "@/components/ui/tempo-control";

const TempoControlControlled = () => {
  const [bpm, setBpm] = useState(96);

  return (
    <div className="flex flex-col items-center gap-3">
      <TempoControl onValueChange={setBpm} value={bpm}>
        <TempoControlInput />
        <TempoControlTap>Tap tempo</TempoControlTap>
      </TempoControl>
      <span className="text-muted-foreground text-sm tabular-nums">
        Beat duration: {Math.round(60_000 / bpm)} ms
      </span>
    </div>
  );
};

export default TempoControlControlled;
