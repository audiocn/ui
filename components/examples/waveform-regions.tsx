"use client";

import { useState } from "react";

import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformMarker,
  WaveformRegion,
} from "@/components/ui/waveform";
import { formatTime } from "@/lib/audio/time";

const peaks = Float32Array.from({ length: 320 }, (_, index) =>
  Math.min(1, 0.2 + Math.abs(Math.sin(index * 0.21)) * 0.6 + (index % 40 < 4 ? 0.3 : 0))
);

const WaveformRegions = () => {
  const [region, setRegion] = useState({ end: 22, start: 9 });

  return (
    <div className="flex w-full max-w-lg flex-col gap-2">
      <Waveform aria-label="Episode" className="h-20" duration={60} peaks={peaks} variant="mirror">
        <WaveformCanvas />
        <WaveformRegion end={region.end} onValueChange={setRegion} start={region.start} />
        <WaveformMarker time={40}>Intro ends</WaveformMarker>
        <WaveformCursor />
      </Waveform>
      <p className="font-mono text-muted-foreground text-xs">
        Clip {formatTime(region.start)} – {formatTime(region.end)}
      </p>
    </div>
  );
};

export default WaveformRegions;
