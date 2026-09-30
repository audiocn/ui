"use client";

import { useState } from "react";

import {
  Fader,
  FaderThumb,
  FaderTrack,
  FaderValue,
} from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const FaderWithMeter = () => {
  const [gainDb, setGainDb] = useState(0);
  const signal = useDemoSignal({ channels: 2, kind: "music" });

  return (
    <Fader
      className="h-64 flex-col items-center"
      max={6}
      min={-60}
      onValueChange={setGainDb}
      orientation="vertical"
      size="lg"
      value={gainDb}
      variant="console"
    >
      <FaderValue />
      <FaderTrack className="w-6 overflow-visible bg-transparent">
        <LevelMeter
          aria-label="Program level"
          className="absolute inset-0 h-full min-h-0"
          maxDb={6}
          orientation="vertical"
          size="sm"
          source={signal.meter}
        />
        <FaderThumb />
      </FaderTrack>
    </Fader>
  );
};

export default FaderWithMeter;
