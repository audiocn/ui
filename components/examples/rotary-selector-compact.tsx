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
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
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

/** A wave glyph centred on (x, y), with a box around it to click. */
const WaveIcon = ({ wave, x, y }: { wave: Wave; x: number; y: number }) => (
  <g transform={`translate(${x} ${y}) scale(1.4)`}>
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

const formatPercent = (percent: number) => `${Math.round(percent)}%`;

const RotarySelectorCompact = () => (
  <div className="flex items-start justify-center gap-10 pt-5">
    <Knob defaultValue={50} format={formatPercent}>
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobCap variant="mini" />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Shape</KnobLabel>
    </Knob>
    <RotarySelector format={formatWave} startAngle={-45} values={WAVES}>
      {({ positions }) => (
        <>
          <RotarySelectorDial hitRadius={44}>
            {positions.map((position) => (
              <RotarySelectorPositionLabel
                key={position.value}
                position={position}
              >
                <WaveIcon wave={position.value} {...position.pointAt(68)} />
              </RotarySelectorPositionLabel>
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
          <RotarySelectorValue />
          <RotarySelectorLabel>Wave</RotarySelectorLabel>
        </>
      )}
    </RotarySelector>
    <Knob defaultValue={20} format={formatPercent}>
      <KnobDial>
        <KnobTrack />
        <KnobRange />
        <KnobCap variant="mini" />
      </KnobDial>
      <KnobValue />
      <KnobLabel>Level</KnobLabel>
    </Knob>
  </div>
);

export default RotarySelectorCompact;
