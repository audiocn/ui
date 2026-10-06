"use client";

import {
  Knob,
  KnobCap,
  KnobDial,
  KnobLabel,
  KnobScale,
} from "@/components/ui/knob";

const VolumeKnobTile = () => (
  <Knob className="[--knob-size:11rem]" clickSound defaultValue={33}>
    <KnobDial>
      <KnobScale labelEvery={10} majorEvery={5} ticks={100} />
      <KnobCap />
    </KnobDial>
    <KnobLabel className="sr-only">Volume</KnobLabel>
  </Knob>
);

export default VolumeKnobTile;
