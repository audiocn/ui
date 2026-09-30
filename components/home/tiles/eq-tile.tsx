"use client";

import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderHeader,
  ParameterSliderLabel,
  ParameterSliderValue,
} from "@/components/ui/parameter-slider";

const formatHz = (hz: number) =>
  hz >= 1000
    ? `${(hz / 1000).toFixed(hz >= 10_000 ? 0 : 1)} kHz`
    : `${Math.round(hz)} Hz`;

const EqTile = () => (
  <div className="flex w-full flex-col gap-5">
    <ParameterSlider
      defaultValue={2400}
      format={formatHz}
      max={20_000}
      min={20}
      scale="log"
      step={1}
    >
      <ParameterSliderHeader>
        <ParameterSliderLabel>Frequency</ParameterSliderLabel>
        <ParameterSliderValue />
      </ParameterSliderHeader>
      <ParameterSliderControl />
    </ParameterSlider>
    <ParameterSlider
      decimals={1}
      defaultValue={1.4}
      max={10}
      min={0.1}
      scale="log"
      step={0.1}
    >
      <ParameterSliderHeader>
        <ParameterSliderLabel>Q</ParameterSliderLabel>
        <ParameterSliderValue />
      </ParameterSliderHeader>
      <ParameterSliderControl />
    </ParameterSlider>
    <ParameterSlider
      decimals={1}
      defaultValue={3.5}
      max={12}
      min={-12}
      origin={0}
      step={0.5}
      unit="dB"
    >
      <ParameterSliderHeader>
        <ParameterSliderLabel>Gain</ParameterSliderLabel>
        <ParameterSliderValue />
      </ParameterSliderHeader>
      <ParameterSliderControl />
    </ParameterSlider>
  </div>
);

export default EqTile;
