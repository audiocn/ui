"use client";

import { KnobCap } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionLeader,
  RotarySelectorValue,
} from "@/components/ui/rotary-selector";

const WAVES = ["sine", "triangle", "saw", "square"] as const;
type Wave = (typeof WAVES)[number];

const WAVE_PATHS: Record<Wave, string> = {
  saw: "M-8 4 L0 -4 L0 4 L8 -4 L8 4",
  sine: "M-8 0 C-6 -5.3 -2 -5.3 0 0 S6 5.3 8 0",
  square: "M-8 4 V-4 H0 V4 H8 V-4",
  triangle: "M-8 0 L-4 -4 L4 4 L8 0",
};

const formatWave = (wave: Wave) => `${wave[0]?.toUpperCase()}${wave.slice(1)}`;

/** A wave glyph centred on (x, 0), with a box around it to click. */
const WaveIcon = ({ wave, x = 0 }: { wave: Wave; x?: number }) => (
  <g transform={`translate(${x} 0)`}>
    <rect fill="transparent" height={14} width={22} x={-11} y={-7} />
    <path
      d={WAVE_PATHS[wave]}
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.5}
    />
  </g>
);

const RotarySelectorDemo = () => (
  <RotarySelector
    className="my-2 ml-14 [--knob-size:7rem]"
    format={formatWave}
    startAngle={-45}
    stepAngle={-30}
    values={WAVES}
  >
    {({ positions }) => (
      <>
        <RotarySelectorDial>
          {positions.map((position) => (
            <RotarySelectorPositionLeader
              from={53}
              key={position.value}
              position={position}
              ray={8}
              to={{ x: -18 }}
            >
              <RotarySelectorPositionLabel position={position}>
                <WaveIcon wave={position.value} x={-13} />
              </RotarySelectorPositionLabel>
            </RotarySelectorPositionLeader>
          ))}
          <KnobCap />
        </RotarySelectorDial>
        <RotarySelectorValue />
        <RotarySelectorLabel>Waveform</RotarySelectorLabel>
      </>
    )}
  </RotarySelector>
);

export default RotarySelectorDemo;
