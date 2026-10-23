"use client";

import { useState } from "react";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySelect,
  LcdDisplaySelectContent,
  LcdDisplaySelectItem,
  LcdDisplaySelectTrigger,
  LcdDisplaySeparator,
  LcdDisplayStatus,
} from "@/components/ui/lcd-display";
import { LcdMeter } from "@/components/ui/lcd-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const INPUTS = [
  { kind: "music", label: "Line In", value: "line" },
  { kind: "speech", label: "USB Mic", value: "usb" },
  { kind: "tone", label: "Test Tone", value: "tone" },
] as const;

type Input = (typeof INPUTS)[number]["value"];

const LcdMeterDemo = () => {
  const [input, setInput] = useState<Input>("line");
  const kind =
    INPUTS.find((item) => item.value === input)?.kind ?? INPUTS[0].kind;
  const signal = useDemoSignal({ channels: 2, kind, seed: 3 });

  return (
    <LcdDisplay className="w-full max-w-96">
      <LcdDisplayRow>
        <LcdDisplayLabel>Input</LcdDisplayLabel>
        <LcdDisplaySelect
          items={INPUTS}
          onValueChange={(next) => setInput(next ?? input)}
          value={input}
        >
          <LcdDisplaySelectTrigger />
          <LcdDisplaySelectContent>
            {INPUTS.map((item) => (
              <LcdDisplaySelectItem key={item.value} value={item.value}>
                {item.label}
              </LcdDisplaySelectItem>
            ))}
          </LcdDisplaySelectContent>
        </LcdDisplaySelect>
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdMeter aria-label="Input level" source={signal.meter} />
      <LcdDisplayStatus />
    </LcdDisplay>
  );
};

export default LcdMeterDemo;
