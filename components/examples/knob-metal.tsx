"use client";

import {
  Knob,
  KnobCap,
  KnobDial,
  KnobLabel,
  KnobRange,
  KnobTrack,
  KnobValue,
} from "@/components/ui/knob";
import { formatPan, parsePan } from "@/components/ui/pan-control";
import { formatDb } from "@/lib/audio/decibels";

const formatHz = (hz: number) =>
  hz >= 1000 ? `${(hz / 1000).toFixed(1)}k` : `${Math.round(hz)}`;

const formatPercent = (percent: number) => `${Math.round(percent)}%`;

const KnobMetal = () => (
  <div className="flex flex-wrap items-start justify-center gap-8">
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
        <KnobCap variant="mini" />
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
      parse={parsePan}
      step={0.05}
    >
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobCap variant="mini" />
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
        <KnobCap variant="mini" />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Low cut</KnobLabel>
    </Knob>
    <Knob defaultValue={65} format={formatPercent}>
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobCap variant="mini" />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Mix</KnobLabel>
    </Knob>
  </div>
);

export default KnobMetal;
