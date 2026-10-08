"use client";

import { KnobCap } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionMark,
} from "@/components/ui/rotary-selector";

const RANGES = ["low", "mid", "high"] as const;

const selectors = [
  { label: "sm", size: "sm" },
  { label: "default", size: "default" },
  { label: "lg", size: "lg" },
  { disabled: true, label: "disabled" },
] as const;

const RotarySelectorSizes = () => (
  <div className="flex flex-wrap items-end justify-center gap-x-8 gap-y-6">
    {selectors.map(({ label, ...props }) => (
      <RotarySelector
        defaultValue="mid"
        key={label}
        startAngle={-45}
        stepAngle={45}
        values={RANGES}
        {...props}
      >
        {({ positions }) => (
          <>
            <RotarySelectorDial hitRadius={44}>
              {positions.map((position) => (
                <RotarySelectorPositionMark
                  key={position.value}
                  position={position}
                  strokeWidth={3}
                />
              ))}
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
            <RotarySelectorLabel>{label}</RotarySelectorLabel>
          </>
        )}
      </RotarySelector>
    ))}
  </div>
);

export default RotarySelectorSizes;
