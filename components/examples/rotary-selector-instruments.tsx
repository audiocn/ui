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
import type { RotarySelectorPosition } from "@/components/ui/rotary-selector";

const INSTRUMENTS = {
  AC: "Accent",
  BD: "Bass drum",
  CB: "Cowbell",
  CH: "Closed hi-hat",
  CP: "Hand clap",
  CY: "Cymbal",
  HT: "High tom",
  LT: "Low tom",
  MT: "Mid tom",
  OH: "Open hi-hat",
  RS: "Rim shot",
  SD: "Snare drum",
} as const;
type Instrument = keyof typeof INSTRUMENTS;

const VALUES = [
  "AC",
  "BD",
  "SD",
  "LT",
  "MT",
  "HT",
  "RS",
  "CP",
  "CB",
  "CY",
  "OH",
  "CH",
] as const satisfies readonly Instrument[];

const BOX = { gap: 20, height: 10, width: 20 } as const;

/** Boxes sit in a row above or below the dial, or in a column beside it. */
const boxCenter = ({ angle, pointAt }: RotarySelectorPosition<Instrument>) => {
  const { x, y } = pointAt(80);
  const turn = ((angle % 360) + 360) % 360;
  if (turn <= 30 || turn >= 330) {
    return { x, y: -BOX.gap };
  }
  if (turn >= 150 && turn <= 210) {
    return { x, y: 100 + BOX.gap };
  }
  return { x: turn < 180 ? 100 + BOX.gap : -BOX.gap, y };
};

const RotarySelectorInstruments = () => (
  <RotarySelector
    className="mx-12 mt-8 [--knob-size:8rem]"
    defaultValue="BD"
    format={(instrument) => `${instrument} ${INSTRUMENTS[instrument]}`}
    startAngle={-165}
    values={VALUES}
  >
    {({ positions }) => (
      <>
        <RotarySelectorDial>
          {positions.map((position) => {
            const number = position.pointAt(60);
            const box = boxCenter(position);
            return (
              <g key={position.value}>
                <RotarySelectorPositionMark
                  inner={51}
                  outer={54}
                  position={position}
                />
                <RotarySelectorPositionLabel position={position}>
                  <circle
                    cx={number.x}
                    cy={number.y}
                    fill="transparent"
                    r={4}
                  />
                  <text
                    dominantBaseline="central"
                    fontSize={5}
                    textAnchor="middle"
                    {...number}
                  >
                    {position.index + 1}
                  </text>
                </RotarySelectorPositionLabel>
                <RotarySelectorPositionLabel
                  className="group/box"
                  position={position}
                >
                  <rect
                    className="stroke-border group-data-selected/box:fill-primary group-data-selected/box:stroke-primary"
                    fill="transparent"
                    height={BOX.height}
                    rx={1.5}
                    strokeWidth={0.6}
                    width={BOX.width}
                    x={box.x - BOX.width / 2}
                    y={box.y - BOX.height / 2}
                  />
                  <text
                    className="group-data-selected/box:fill-primary-foreground font-semibold"
                    dominantBaseline="central"
                    fontSize={5}
                    textAnchor="middle"
                    {...box}
                  >
                    {position.value}
                  </text>
                </RotarySelectorPositionLabel>
              </g>
            );
          })}
          <KnobCap />
        </RotarySelectorDial>
        <RotarySelectorValue className="mt-8" />
        <RotarySelectorLabel>Instrument select</RotarySelectorLabel>
      </>
    )}
  </RotarySelector>
);

export default RotarySelectorInstruments;
