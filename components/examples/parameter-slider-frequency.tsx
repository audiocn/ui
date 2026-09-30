"use client";

import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderHeader,
  ParameterSliderLabel,
  ParameterSliderMarks,
  ParameterSliderValue,
} from "@/components/ui/parameter-slider";

const formatHz = (hz: number) =>
  hz >= 1000
    ? `${(hz / 1000).toFixed(hz >= 10_000 ? 0 : 1)} kHz`
    : `${Math.round(hz)} Hz`;

const marks = [
  { label: "20", value: 20 },
  { label: "200", value: 200 },
  { label: "2k", value: 2000 },
  { label: "20k", value: 20_000 },
];

const ParameterSliderFrequency = () => (
  <ParameterSlider
    className="max-w-sm"
    defaultValue={1000}
    format={formatHz}
    marks={marks}
    max={20_000}
    min={20}
    scale="log"
    step={1}
  >
    <ParameterSliderHeader>
      <ParameterSliderLabel>High-pass filter</ParameterSliderLabel>
      <ParameterSliderValue />
    </ParameterSliderHeader>
    <ParameterSliderControl />
    <ParameterSliderMarks />
  </ParameterSlider>
);

export default ParameterSliderFrequency;
