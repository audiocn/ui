"use client";

import { useState } from "react";

import {
  Fader,
  FaderLabel,
  FaderRange,
  FaderScale,
  FaderThumb,
  FaderTrack,
  FaderValue,
} from "@/components/ui/fader";

const FaderSilence = () => {
  const [volumeDb, setVolumeDb] = useState(-10);

  return (
    <Fader
      className="max-w-sm"
      max={6}
      min={-80}
      onValueChange={setVolumeDb}
      silenceAtMin
      taper="audio"
      value={volumeDb}
    >
      <div className="flex items-center justify-between">
        <FaderLabel>Output</FaderLabel>
        <FaderValue />
      </div>
      <FaderTrack>
        <FaderRange />
        <FaderThumb />
      </FaderTrack>
      <FaderScale ticks={[6, 0, -10, -20, -40, -80]} />
    </Fader>
  );
};

export default FaderSilence;
