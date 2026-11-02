"use client";

import { PlayIcon, StopIcon, WaveformIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  VuMeter,
  VuMeterBadge,
  VuMeterFace,
  VuMeterLabel,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { useSound } from "@/hooks/use-sound";

const VuMeterDemo = () => {
  const sound = useSound("/sounds/peon-work-work-loop.wav", { loop: true });

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <VuMeter aria-label="Program level" channel={0} input={sound.output}>
        <VuMeterFace>
          <VuMeterScale />
          <VuMeterNeedle />
          <VuMeterBadge>
            <span className="rounded-md border-2 border-(--vu-ink)/70 bg-(--vu-face) p-1 shadow-sm">
              <WaveformIcon className="size-[5cqw]" weight="bold" />
            </span>
          </VuMeterBadge>
          <VuMeterLegend>
            RMS <span className="text-(--vu-zone-clip)">−18</span>
          </VuMeterLegend>
          <VuMeterLabel>L</VuMeterLabel>
        </VuMeterFace>
      </VuMeter>
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

export default VuMeterDemo;
