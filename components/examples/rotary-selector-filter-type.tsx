"use client";

import { KnobPointer } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionMark,
  RotarySelectorValue,
} from "@/components/ui/rotary-selector";

const FILTERS = {
  BP: "Band pass",
  HP: "High pass",
  LP: "Low pass",
} as const;
type Filter = keyof typeof FILTERS;

const VALUES = ["LP", "BP", "HP"] as const satisfies readonly Filter[];

const RotarySelectorFilterType = () => (
  <RotarySelector
    className="my-6"
    size="lg"
    format={(filter) => FILTERS[filter]}
    startAngle={-30}
    stepAngle={30}
    values={VALUES}
  >
    {({ positions }) => (
      <>
        <RotarySelectorDial hitRadius={32}>
          {positions.map((position) => {
            const label = position.pointAt(72);
            return (
              <g key={position.value}>
                <RotarySelectorPositionMark
                  inner={44}
                  outer={54}
                  position={position}
                  strokeWidth={3}
                />
                <RotarySelectorPositionLabel position={position}>
                  <circle cx={label.x} cy={label.y} fill="transparent" r={16} />
                  <text
                    dominantBaseline="central"
                    fontSize={18}
                    fontWeight={600}
                    textAnchor="middle"
                    {...label}
                  >
                    {position.value}
                  </text>
                </RotarySelectorPositionLabel>
              </g>
            );
          })}
          <KnobPointer />
        </RotarySelectorDial>
        <RotarySelectorValue />
        <RotarySelectorLabel>Filter</RotarySelectorLabel>
      </>
    )}
  </RotarySelector>
);

export default RotarySelectorFilterType;
