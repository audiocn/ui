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
import type { DemoSignalKind } from "@/hooks/use-demo-signal";

const MAX_DB = 6;
const MIN_DB = -60;

interface ConsoleFaderProps {
  label: string;
  kind: DemoSignalKind;
  seed: number;
  initialDb: number;
}

const ConsoleFader = ({ label, kind, seed, initialDb }: ConsoleFaderProps) => {
  const [gainDb, setGainDb] = useState(initialDb);
  const signal = useDemoSignal({ channels: 2, gainDb, kind, seed });

  return (
    <div className="flex flex-col items-center gap-2">
      <Fader
        aria-label={`${label} volume`}
        className="h-56 flex-col items-center"
        max={MAX_DB}
        min={MIN_DB}
        onValueChange={setGainDb}
        orientation="vertical"
        size="lg"
        value={gainDb}
        variant="console"
      >
        <FaderValue />
        <FaderTrack className="w-6 overflow-visible bg-transparent">
          <LevelMeter
            aria-label={`${label} level`}
            className="absolute inset-0 h-full min-h-0"
            maxDb={MAX_DB}
            orientation="vertical"
            size="sm"
            source={signal.meter}
          />
          <FaderThumb />
        </FaderTrack>
      </Fader>
      <span className="text-xs font-medium">{label}</span>
    </div>
  );
};

const FadersTile = () => (
  <div className="flex w-full justify-center gap-4">
    <ConsoleFader initialDb={-3} kind="music" label="Drums" seed={2} />
    <ConsoleFader initialDb={-8} kind="noise" label="Bass" seed={5} />
    <ConsoleFader initialDb={-14} kind="music" label="Keys" seed={7} />
  </div>
);

export default FadersTile;
