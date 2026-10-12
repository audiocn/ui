"use client";

import { KnobCap, pointAt } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionMark,
  RotarySelectorValue,
} from "@/components/ui/rotary-selector";

const MEASURES = ["MANUAL", 16, 12, 8, 4, 2] as const;
type Measure = (typeof MEASURES)[number];

const formatMeasure = (measure: Measure) =>
  measure === "MANUAL" ? "Manual" : `${measure} measures`;

const BRACKET = { label: 81, overhang: 6, radius: 73, tick: 3.5 } as const;

/** An arc over the numbered positions, with end ticks and a caption. */
const Bracket = ({ from, to }: { from: number; to: number }) => {
  const low = Math.min(from, to) - BRACKET.overhang;
  const high = Math.max(from, to) + BRACKET.overhang;
  const start = pointAt(low, BRACKET.radius);
  const end = pointAt(high, BRACKET.radius);
  const middle = (low + high) / 2;
  const caption = pointAt(middle, BRACKET.label);
  return (
    <g className="stroke-muted-foreground" fill="none" strokeWidth={0.8}>
      <path
        d={`M ${start.x} ${start.y} A ${BRACKET.radius} ${BRACKET.radius} 0 0 1 ${end.x} ${end.y}`}
      />
      {[low, high].map((angle) => {
        const inner = pointAt(angle, BRACKET.radius - BRACKET.tick);
        const outer = pointAt(angle, BRACKET.radius);
        return (
          <line
            key={angle}
            x1={inner.x}
            x2={outer.x}
            y1={inner.y}
            y2={outer.y}
          />
        );
      })}
      <text
        className="fill-muted-foreground"
        stroke="none"
        dominantBaseline="central"
        fontSize={5}
        letterSpacing={1}
        textAnchor="middle"
        transform={`rotate(${middle} ${caption.x} ${caption.y})`}
        {...caption}
      >
        MEASURES
      </text>
    </g>
  );
};

const RotarySelectorMeasures = () => (
  <RotarySelector
    className="mt-10 [--knob-size:7rem]"
    format={formatMeasure}
    startAngle={-75}
    values={MEASURES}
  >
    {({ position, positions }) => (
      <>
        <RotarySelectorDial hitRadius={44}>
          {positions.map((item) => {
            const { x, y } = item.pointAt(62);
            return (
              <g key={item.value}>
                <RotarySelectorPositionMark
                  inner={51}
                  outer={55}
                  position={item}
                  strokeWidth={1.2}
                />
                <RotarySelectorPositionLabel position={item}>
                  <circle cx={x} cy={y} fill="transparent" r={5} />
                  <text
                    dominantBaseline="central"
                    fontSize={item.value === "MANUAL" ? 4.5 : 7}
                    fontWeight={600}
                    textAnchor="middle"
                    transform={`rotate(${item.angle} ${x} ${y})`}
                    x={x}
                    y={y}
                  >
                    {item.value}
                  </text>
                </RotarySelectorPositionLabel>
              </g>
            );
          })}
          <Bracket from={position(16).angle} to={position(2).angle} />
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
        <RotarySelectorLabel>Auto fill in</RotarySelectorLabel>
      </>
    )}
  </RotarySelector>
);

export default RotarySelectorMeasures;
