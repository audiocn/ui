"use client";

import { AsteriskIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { KnobCap, pointAt } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionMark,
} from "@/components/ui/rotary-selector";
import type { RotarySelectorPosition } from "@/components/ui/rotary-selector";
import { cn } from "@/lib/utils";

const EFFORTS = ["low", "med", "high", "ultra"] as const;
type Effort = (typeof EFFORTS)[number];

const START_ANGLE = -120;
const STEP_ANGLE = 80;
/** Short ticks between two positions. */
const MINOR_TICKS = 3;
const PRINT = {
  dot: 44,
  dotRadius: 1.9,
  label: 54,
  labelSize: 7.5,
  majorInner: 35.5,
  majorOuter: 40.5,
  minorInner: 37,
  minorOuter: 39.5,
} as const;
/** Labels this close to the centre line are centred over their dot. */
const CENTRED_SPAN = 6;
/** Rough width of a mono capital with the label's tracking, in font sizes. */
const GLYPH_WIDTH = 0.82;

const MINOR_ANGLES = Array.from(
  { length: (EFFORTS.length - 1) * MINOR_TICKS },
  (_, index) => {
    const gap = Math.floor(index / MINOR_TICKS);
    const tick = (index % MINOR_TICKS) + 1;
    return START_ANGLE + STEP_ANGLE * (gap + tick / (MINOR_TICKS + 1));
  }
);

/** Labels hang off the side of their dot that faces away from the dial. */
const labelAnchor = (x: number) => {
  if (x < 50 - CENTRED_SPAN) {
    return "end";
  }
  return x > 50 + CENTRED_SPAN ? "start" : "middle";
};

const EffortPrint = ({
  position,
}: {
  position: RotarySelectorPosition<Effort>;
}) => {
  const dot = position.pointAt(PRINT.dot);
  const label = position.pointAt(PRINT.label);
  const anchor = labelAnchor(label.x);
  const width = position.value.length * PRINT.labelSize * GLYPH_WIDTH;
  const offsets = { end: -width, middle: -width / 2, start: 0 };
  return (
    <g>
      <RotarySelectorPositionMark
        inner={PRINT.majorInner}
        outer={PRINT.majorOuter}
        position={position}
        strokeLinecap="round"
        strokeWidth={1.6}
      />
      <RotarySelectorPositionLabel position={position}>
        {/* A box around the dot and the word, to click. */}
        <rect
          fill="transparent"
          height={PRINT.labelSize * 2.6}
          width={width + PRINT.labelSize}
          x={label.x + offsets[anchor] - PRINT.labelSize / 2}
          y={Math.min(dot.y, label.y) - PRINT.labelSize}
        />
        <circle
          className="stroke-current in-data-selected:fill-current"
          cx={dot.x}
          cy={dot.y}
          fill="transparent"
          r={PRINT.dotRadius}
          strokeWidth={0.8}
        />
        <text
          className="font-mono font-semibold tracking-widest uppercase"
          dominantBaseline="central"
          fontSize={PRINT.labelSize}
          textAnchor={anchor}
          x={label.x}
          y={label.y}
        >
          {position.value}
        </text>
      </RotarySelectorPositionLabel>
    </g>
  );
};

const RotarySelectorEffort = () => {
  const [auto, setAuto] = useState(false);
  return (
    <div className="flex flex-col items-center gap-2 py-4 [--lamp-glow:0_0_0_4px_color-mix(in_oklab,var(--lamp)_25%,transparent),0_0_18px_4px_color-mix(in_oklab,var(--lamp)_55%,transparent)] [--lamp:oklch(0.72_0.18_45)]">
      <RotarySelector
        className="mx-20 mt-10 mb-2 [--knob-size:10rem]"
        clickSound
        defaultValue="high"
        disabled={auto}
        format={(effort) => effort.toUpperCase()}
        startAngle={START_ANGLE}
        stepAngle={STEP_ANGLE}
        values={EFFORTS}
      >
        {({ positions }) => (
          <>
            <RotarySelectorDial>
              {MINOR_ANGLES.map((angle) => {
                const inner = pointAt(angle, PRINT.minorInner);
                const outer = pointAt(angle, PRINT.minorOuter);
                return (
                  <line
                    className="stroke-muted-foreground/50"
                    key={angle}
                    strokeLinecap="round"
                    strokeWidth={0.8}
                    x1={inner.x}
                    x2={outer.x}
                    y1={inner.y}
                    y2={outer.y}
                  />
                );
              })}
              {positions.map((position) => (
                <EffortPrint key={position.value} position={position} />
              ))}
              <KnobCap variant="knurled" />
            </RotarySelectorDial>
            <RotarySelectorLabel className="sr-only">
              Effort
            </RotarySelectorLabel>
          </>
        )}
      </RotarySelector>
      {/* The lamp hangs off the knob on a short lead, lit when on. */}
      <div
        className={cn(
          "bg-border h-5 w-0.5 rounded-full transition-colors",
          auto && "bg-(--lamp)"
        )}
      />
      <button
        aria-pressed={auto}
        className="group text-muted-foreground focus-visible:ring-ring/50 flex items-center gap-3 rounded-md px-2 py-1 font-mono text-xs font-semibold tracking-widest uppercase outline-none focus-visible:ring-3 aria-pressed:text-(--lamp)"
        onClick={() => setAuto((on) => !on)}
        type="button"
      >
        <span className="border-border bg-muted text-muted-foreground group-aria-pressed:text-background grid size-7 place-items-center rounded-full border transition group-aria-pressed:border-transparent group-aria-pressed:bg-(--lamp) group-aria-pressed:shadow-(--lamp-glow)">
          <AsteriskIcon className="size-4" weight="bold" />
        </span>
        Auto effort
      </button>
      <p className="text-muted-foreground font-mono text-xs tracking-widest uppercase">
        {auto ? "Picks the effort for each task" : "Turn the knob to choose"}
      </p>
    </div>
  );
};

export default RotarySelectorEffort;
