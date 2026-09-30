"use client";

import { useState } from "react";

import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderDescription,
  ParameterSliderHeader,
  ParameterSliderInput,
  ParameterSliderLabel,
  ParameterSliderReset,
} from "@/components/ui/parameter-slider";

const ParameterSliderDemo = () => {
  const [offsetMs, setOffsetMs] = useState(0);

  return (
    <ParameterSlider
      className="max-w-sm"
      largeStep={50}
      max={1000}
      min={-1000}
      onValueChange={setOffsetMs}
      origin={0}
      step={5}
      unit="ms"
      value={offsetMs}
    >
      <ParameterSliderHeader>
        <ParameterSliderLabel>Sync offset</ParameterSliderLabel>
        <ParameterSliderReset />
        <ParameterSliderInput />
      </ParameterSliderHeader>
      <ParameterSliderControl />
      <ParameterSliderDescription>
        Delays the microphone to line up with your camera.
      </ParameterSliderDescription>
    </ParameterSlider>
  );
};

export default ParameterSliderDemo;
