"use client";

import { PlayIcon, StopIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { VuMeter } from "@/components/ui/vu-meter";
import { useSound } from "@/hooks/use-sound";

const meters = [
  { caption: "classic", className: undefined, variant: "classic" },
  { caption: "flat", className: undefined, variant: "flat" },
  {
    caption: "classic, warm lamp",
    className:
      "[--vu-face-shade:oklch(0.74_0.11_68)] [--vu-face:oklch(0.95_0.07_88)] [--vu-ink:oklch(0.3_0.03_60)]",
    variant: "classic",
  },
] as const;

const VuMeterVariants = () => {
  const sound = useSound("/sounds/peon-work-work-loop.wav", { loop: true });

  return (
    <div className="flex w-full max-w-3xl flex-col gap-4">
      <div className="grid gap-4 sm:grid-cols-3">
        {meters.map((meter) => (
          <figure className="grid gap-2" key={meter.caption}>
            <VuMeter
              aria-label={`${meter.caption} VU meter`}
              className={meter.className}
              input={sound.output}
              variant={meter.variant}
            />
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

export default VuMeterVariants;
