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

const MODELS = [
  { label: "Opus 5.5", value: "opus" },
  { label: "Sonnet 5.5", value: "sonnet" },
  { label: "Haiku 5.5", value: "haiku" },
];

const EFFORTS = [
  { label: "Low", value: "low" },
  { label: "Med", value: "med" },
  { label: "High", value: "high" },
  { label: "Ultra", value: "ultra" },
];

const LcdDisplayDemo = () => {
  const [model, setModel] = useState("opus");
  const [effort, setEffort] = useState("high");

  return (
    <LcdDisplay className="w-full max-w-80">
      <LcdDisplayRow>
        <LcdDisplayLabel>Model</LcdDisplayLabel>
        <LcdDisplaySelect
          items={MODELS}
          onValueChange={(next) => setModel(next ?? model)}
          value={model}
        >
          <LcdDisplaySelectTrigger />
          <LcdDisplaySelectContent>
            {MODELS.map((item) => (
              <LcdDisplaySelectItem key={item.value} value={item.value}>
                {item.label}
              </LcdDisplaySelectItem>
            ))}
          </LcdDisplaySelectContent>
        </LcdDisplaySelect>
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdDisplayLabel>Effort</LcdDisplayLabel>
        <LcdDisplaySelect
          items={EFFORTS}
          onValueChange={(next) => setEffort(next ?? effort)}
          value={effort}
        >
          <LcdDisplaySelectTrigger />
          <LcdDisplaySelectContent>
            {EFFORTS.map((item) => (
              <LcdDisplaySelectItem key={item.value} value={item.value}>
                {item.label}
              </LcdDisplaySelectItem>
            ))}
          </LcdDisplaySelectContent>
        </LcdDisplaySelect>
      </LcdDisplayRow>
      <LcdDisplayStatus />
    </LcdDisplay>
  );
};

export default LcdDisplayDemo;
