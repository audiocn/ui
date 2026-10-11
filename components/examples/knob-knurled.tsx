"use client";

import {
  Knob,
  KnobCap,
  KnobDial,
  KnobLabel,
  KnobScale,
} from "@/components/ui/knob";

const KnobKnurled = () => (
  <Knob
    className="[--knob-size:10rem]"
    clickSound
    defaultValue={6}
    max={10}
    step={1}
  >
    <KnobDial>
      <KnobScale labelEvery={1} majorEvery={1} ticks={10} />
      <KnobCap variant="knurled" />
    </KnobDial>
    <KnobLabel className="sr-only">Drive</KnobLabel>
  </Knob>
);

export default KnobKnurled;
