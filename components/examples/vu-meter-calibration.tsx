"use client";

import {
  VuMeter,
  VuMeterFace,
  VuMeterLegend,
  VuMeterNeedle,
  VuMeterScale,
} from "@/components/ui/vu-meter";

const BROADCAST_TICKS = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
const INPUT_RMS_DB = -18;

const meters = [
  { caption: "Reference −18 dBFS: reads 0 VU", minDb: -10, referenceDb: -18 },
  { caption: "Reference −20 dBFS: reads +2 VU", minDb: -10, referenceDb: -20 },
  { caption: "Broadcast scale, −20 to +3", minDb: -20, referenceDb: -18 },
] as const;

const VuMeterCalibration = () => (
  <div className="grid w-full max-w-3xl gap-4 sm:grid-cols-3">
    {meters.map((meter) => (
      <figure className="grid gap-2" key={meter.caption}>
        <VuMeter
          aria-label={meter.caption}
          minDb={meter.minDb}
          referenceDb={meter.referenceDb}
          rmsDb={INPUT_RMS_DB}
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
);

export default VuMeterCalibration;
