"use client";

import {
  Knob,
  KnobDial,
  KnobLabel,
  KnobPointer,
  KnobRange,
  KnobTrack,
  KnobValue,
} from "@/components/ui/knob";
import { formatDb } from "@/lib/audio/decibels";

const directions = [
  { label: "Vertical", value: "vertical" },
  { label: "Horizontal", value: "horizontal" },
  { label: "Circular", value: "circular" },
] as const;

const KnobDragDirections = () => (
  <div className="flex flex-wrap items-start justify-center gap-8">
    {directions.map(({ label, value }) => (
      <Knob
        defaultValue={0}
        dragDirection={value}
        format={(db) => formatDb(db, { decimals: 1 })}
        key={value}
        max={24}
        min={-24}
        origin={0}
      >
        <KnobDial>
          <KnobTrack />
          <KnobRange />
          <KnobPointer />
        </KnobDial>
        <KnobValue />
        <KnobLabel>{label}</KnobLabel>
      </Knob>
    ))}
  </div>
);

export default KnobDragDirections;
