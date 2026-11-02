"use client";

import { PlayIcon, StopIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  VuMeter,
  VuMeterFace,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { useSound } from "@/hooks/use-sound";

const BROADCAST_TICKS = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];

const meters = [
  { caption: "Reference −18 dBFS", minDb: -10, referenceDb: -18 },
  {
    caption: "Reference −20 dBFS: reads 2 VU higher",
    minDb: -10,
    referenceDb: -20,
  },
  { caption: "Broadcast scale, −20 to +3", minDb: -20, referenceDb: -18 },
] as const;

const VuMeterCalibration = () => {
  const sound = useSound("/sounds/peon-work-work-loop.wav", { loop: true });

  return (
    <div className="flex w-full max-w-3xl flex-col gap-4">
      <div className="grid gap-4 sm:grid-cols-3">
        {meters.map((meter) => (
          <figure className="grid gap-2" key={meter.caption}>
            <VuMeter
              aria-label={meter.caption}
              input={sound.output}
              minDb={meter.minDb}
              referenceDb={meter.referenceDb}
            >
              <VuMeterFace>
                <VuMeterScale
                  ticks={meter.minDb === -20 ? BROADCAST_TICKS : undefined}
                />
                <VuMeterNeedle />
                <VuMeterLegend>VU</VuMeterLegend>
              </VuMeterFace>
            </VuMeter>
            <figcaption className="text-muted-foreground text-xs">
              {meter.caption}
            </figcaption>
          </figure>
        ))}
      </div>
      <Button
        className="self-start"
        disabled={!sound.isLoaded}
        onClick={sound.isPlaying ? sound.stop : sound.play}
        size="sm"
        variant="outline"
      >
        {sound.isPlaying ? (
          <StopIcon data-icon="inline-start" />
        ) : (
          <PlayIcon data-icon="inline-start" />
        )}
        {sound.isPlaying ? "Stop" : "Play"}
      </Button>
    </div>
  );
};

export default VuMeterCalibration;
