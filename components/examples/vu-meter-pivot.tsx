"use client";

import {
  VuMeter,
  VuMeterFace,
  VuMeterLabel,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const meters = [
  { caption: "bottom (default)", pivot: "bottom" },
  { caption: "top", pivot: "top" },
] as const;

const VuMeterPivot = () => {
  const signal = useDemoSignal({ kind: "music", seed: 5 });

  return (
    <div className="grid w-full max-w-2xl gap-4 sm:grid-cols-2">
      {meters.map((meter) => (
        <figure className="grid gap-2" key={meter.pivot}>
          <VuMeter
            aria-label={`VU meter, ${meter.caption} pivot`}
            pivot={meter.pivot}
            source={signal.meter}
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
  );
};

export default VuMeterPivot;
