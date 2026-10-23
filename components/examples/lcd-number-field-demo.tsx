"use client";

import { useState } from "react";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
} from "@/components/ui/lcd-display";
import {
  LcdNumberField,
  LcdNumberFieldGroup,
  LcdNumberFieldInput,
  LcdNumberFieldReset,
  LcdNumberFieldScrubArea,
  LcdNumberFieldStepper,
  LcdNumberFieldUnit,
} from "@/components/ui/lcd-number-field";

const ONE_DECIMAL = { maximumFractionDigits: 1, minimumFractionDigits: 1 };

const LcdNumberFieldDemo = () => {
  const [tempo, setTempo] = useState<number | null>(128);
  const [swing, setSwing] = useState<number | null>(54);

  return (
    <LcdDisplay className="w-full max-w-96">
      <LcdDisplayRow>
        <LcdNumberField
          format={ONE_DECIMAL}
          largeStep={10}
          max={300}
          min={20}
          onValueChange={setTempo}
          resetValue={128}
          smallStep={0.1}
          step={1}
          value={tempo}
        >
          <LcdNumberFieldScrubArea>
            <LcdDisplayLabel>Tempo</LcdDisplayLabel>
          </LcdNumberFieldScrubArea>
          <LcdNumberFieldGroup>
            <LcdNumberFieldReset />
            <LcdNumberFieldInput />
            <LcdNumberFieldUnit>Bpm</LcdNumberFieldUnit>
            <LcdNumberFieldStepper />
          </LcdNumberFieldGroup>
        </LcdNumberField>
      </LcdDisplayRow>
      <LcdDisplaySeparator />
      <LcdDisplayRow>
        <LcdNumberField
          max={75}
          min={50}
          onValueChange={setSwing}
          value={swing}
        >
          <LcdNumberFieldScrubArea>
            <LcdDisplayLabel>Swing</LcdDisplayLabel>
          </LcdNumberFieldScrubArea>
          <LcdNumberFieldGroup>
            <LcdNumberFieldInput />
            <LcdNumberFieldUnit>%</LcdNumberFieldUnit>
            <LcdNumberFieldStepper />
          </LcdNumberFieldGroup>
        </LcdNumberField>
      </LcdDisplayRow>
    </LcdDisplay>
  );
};

export default LcdNumberFieldDemo;
