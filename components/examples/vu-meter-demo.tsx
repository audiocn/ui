"use client";

import { WaveformIcon } from "@phosphor-icons/react";

import {
  VuMeter,
  VuMeterBadge,
  VuMeterFace,
  VuMeterLabel,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const VuMeterDemo = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music" });

  return (
    <VuMeter
      aria-label="Left channel"
      channel={0}
      className="max-w-md"
      source={signal.meter}
    >
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
  );
};

export default VuMeterDemo;
