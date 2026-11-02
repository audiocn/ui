"use client";

import { PlayIcon, StopIcon } from "@phosphor-icons/react";
import { useEffect, useMemo } from "react";

import { Button } from "@/components/ui/button";
import {
  VuMeter,
  VuMeterFace,
  VuMeterLabel,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { disconnectFrom } from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import { useSound } from "@/hooks/use-sound";

const sides = [
  { channel: 0, label: "L", name: "Left channel" },
  { channel: 1, label: "R", name: "Right channel" },
] as const;

const PAN = -0.3;

const VuMeterStereo = () => {
  const { context } = useAudioContext();
  const sound = useSound("/sounds/peon-work-work-loop.wav", {
    destination: null,
    loop: true,
  });
  const panner = useMemo(() => {
    const node = context?.createStereoPanner() ?? null;
    if (node) {
      node.pan.value = PAN;
    }
    return node;
  }, [context]);

  // The sample is mono: pan it left so the two needles differ.
  const { output } = sound;
  useEffect(() => {
    if (!(context && panner && output)) {
      return;
    }
    output.connect(panner);
    panner.connect(context.destination);
    return () => {
      disconnectFrom(output, panner);
      disconnectFrom(panner, context.destination);
    };
  }, [context, output, panner]);

  return (
    <div className="flex w-full max-w-2xl flex-col gap-4">
      <div className="grid gap-3 sm:grid-cols-2">
        {sides.map((side) => (
          <VuMeter
            aria-label={side.name}
            channel={side.channel}
            input={panner}
            key={side.channel}
          >
            <VuMeterFace>
              <VuMeterScale />
              <VuMeterNeedle />
              <VuMeterLegend>VU</VuMeterLegend>
              <VuMeterLabel>{side.label}</VuMeterLabel>
            </VuMeterFace>
          </VuMeter>
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

export default VuMeterStereo;
