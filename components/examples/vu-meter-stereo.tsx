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

const sides = [
  { channel: 0, label: "L", name: "Left channel" },
  { channel: 1, label: "R", name: "Right channel" },
] as const;

const VuMeterStereo = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music", seed: 3 });

  return (
    <div className="grid w-full max-w-2xl gap-3 sm:grid-cols-2">
      {sides.map((side) => (
        <VuMeter
          aria-label={side.name}
          channel={side.channel}
          key={side.channel}
          source={signal.meter}
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
  );
};

export default VuMeterStereo;
