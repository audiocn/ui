"use client";

import { useState } from "react";

import {
  Knob,
  KnobDial,
  KnobLabel,
  KnobPointer,
  KnobRange,
  KnobTrack,
  KnobValue,
} from "@/components/ui/knob";
import { describePan, formatPan, parsePan } from "@/components/ui/pan-control";

const PanControlKnob = () => {
  const [pan, setPan] = useState(0);

  return (
    <Knob
      format={formatPan}
      largeStep={0.25}
      max={1}
      min={-1}
      onValueChange={setPan}
      origin={0}
      parse={parsePan}
      resetValue={0}
      step={0.05}
      value={pan}
    >
      <KnobDial aria-valuetext={describePan(pan)}>
        <KnobTrack />
        <KnobRange />
        <KnobPointer />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Pan</KnobLabel>
    </Knob>
  );
};

export default PanControlKnob;
