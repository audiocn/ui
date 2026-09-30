"use client";

import {
  Knob,
  KnobDial,
  KnobLabel,
  KnobPointer,
  KnobRange,
  KnobTrack,
} from "@/components/ui/knob";

const sizes = ["sm", "default", "lg"] as const;

const KnobSizes = () => (
  <div className="flex items-end gap-8">
    {sizes.map((size) => (
      <Knob defaultValue={65} key={size} size={size}>
        <KnobDial>
          <KnobTrack />
          <KnobRange />
          <KnobPointer />
        </KnobDial>
        <KnobLabel className="text-muted-foreground">{size}</KnobLabel>
      </Knob>
    ))}
    <Knob defaultValue={30} disabled>
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobPointer />
      </KnobDial>
      <KnobLabel className="text-muted-foreground">disabled</KnobLabel>
    </Knob>
  </div>
);

export default KnobSizes;
