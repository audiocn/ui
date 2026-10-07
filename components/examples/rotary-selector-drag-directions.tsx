"use client";

import { KnobCap } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionMark,
  RotarySelectorValue,
} from "@/components/ui/rotary-selector";

const STEPS = [1, 2, 3, 4, 5] as const;

const directions = [
  { label: "Vertical", value: "vertical" },
  { label: "Horizontal", value: "horizontal" },
  { label: "Circular", value: "circular" },
] as const;

const RotarySelectorDragDirections = () => (
  <div className="flex flex-wrap items-start justify-center gap-x-12 gap-y-8 pt-4">
    {directions.map(({ label, value }) => (
      <RotarySelector
        dragDirection={value}
        format={(step) => `Step ${step}`}
        key={value}
        size="lg"
        startAngle={-60}
        values={STEPS}
      >
        {({ positions }) => (
          <>
            <RotarySelectorDial hitRadius={44}>
              {positions.map((position) => {
                const { x, y } = position.pointAt(66);
                return (
                  <g key={position.value}>
                    <RotarySelectorPositionMark position={position} />
                    <RotarySelectorPositionLabel position={position}>
                      <circle cx={x} cy={y} fill="transparent" r={8} />
                      <text
                        dominantBaseline="central"
                        fontSize={13}
                        textAnchor="middle"
                        x={x}
                        y={y}
                      >
                        {position.value}
                      </text>
                    </RotarySelectorPositionLabel>
                  </g>
                );
              })}
              <circle
                className="stroke-input"
                cx={50}
                cy={50}
                fill="none"
                r={40}
                strokeWidth={8}
              />
              <KnobCap variant="mini" />
            </RotarySelectorDial>
            <RotarySelectorValue />
            <RotarySelectorLabel>{label}</RotarySelectorLabel>
          </>
        )}
      </RotarySelector>
    ))}
  </div>
);

export default RotarySelectorDragDirections;
