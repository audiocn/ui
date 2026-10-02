"use client";

import { VuMeter } from "@/components/ui/vu-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

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
  const signal = useDemoSignal({ kind: "music", seed: 4 });

  return (
    <div className="grid w-full max-w-3xl gap-4 sm:grid-cols-3">
      {meters.map((meter) => (
        <figure className="grid gap-2" key={meter.caption}>
          <VuMeter
            aria-label={`${meter.caption} VU meter`}
            className={meter.className}
            source={signal.meter}
            variant={meter.variant}
          />
          <figcaption className="text-muted-foreground text-xs">
            {meter.caption}
          </figcaption>
        </figure>
      ))}
    </div>
  );
};

export default VuMeterVariants;
