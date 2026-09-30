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
import { formatPan } from "@/components/ui/pan-control";
import { formatDb } from "@/lib/audio/decibels";

const formatHz = (hz: number) =>
  hz >= 1000 ? `${(hz / 1000).toFixed(1)}k` : `${Math.round(hz)}`;

const KnobDemo = () => (
  <div className="flex items-start gap-8">
    <Knob
      defaultValue={0}
      format={(db) => formatDb(db, { decimals: 0 })}
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
      <KnobLabel>Gain</KnobLabel>
    </Knob>
    <Knob
      defaultValue={0}
      format={formatPan}
      max={1}
      min={-1}
      origin={0}
      step={0.05}
    >
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobPointer />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Pan</KnobLabel>
    </Knob>
    <Knob
      defaultValue={120}
      format={formatHz}
      max={20_000}
      min={20}
      scale="log"
    >
      <KnobDial>
        <KnobTrack />
        <KnobRange className="stroke-meter-ok" />
        <KnobPointer />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Low cut</KnobLabel>
    </Knob>
  </div>
);

export default KnobDemo;
