"use client";

import { PlayIcon, StopIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  VuMeter,
  VuMeterFace,
  VuMeterLabel,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { useSound } from "@/hooks/use-sound";

const meters = [
  { caption: "bottom (default)", pivot: "bottom" },
  { caption: "top", pivot: "top" },
] as const;

const VuMeterPivot = () => {
  const sound = useSound("/sounds/peon-work-work-loop.wav", { loop: true });

  return (
    <div className="flex w-full max-w-2xl flex-col gap-4">
      <div className="grid gap-4 sm:grid-cols-2">
        {meters.map((meter) => (
          <figure className="grid gap-2" key={meter.pivot}>
            <VuMeter
              aria-label={`VU meter, ${meter.caption} pivot`}
              input={sound.output}
              pivot={meter.pivot}
            >
              <VuMeterFace>
                <VuMeterScale />
                <VuMeterNeedle />
                <VuMeterLegend>VU</VuMeterLegend>
                <VuMeterLabel>L</VuMeterLabel>
              </VuMeterFace>
            </VuMeter>
            <figcaption className="text-muted-foreground text-xs">
              pivot: {meter.caption}
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

export default VuMeterPivot;
