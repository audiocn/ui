"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdVuMeter,
  LcdVuMeterChannel,
  LcdVuMeterChannels,
  LcdVuMeterClip,
  LcdVuMeterValue,
} from "@/components/ui/lcd-vumeter";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const LcdVuMeterMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <LcdDisplay>
        <LcdDisplayRow>
          <LcdDisplayLabel>Mic</LcdDisplayLabel>
          <LcdVuMeter source={analyser.meter}>
            <LcdVuMeterChannels>
              <LcdVuMeterChannel />
            </LcdVuMeterChannels>
            <LcdVuMeterValue />
            <LcdVuMeterClip />
          </LcdVuMeter>
        </LcdDisplayRow>
      </LcdDisplay>
      <Button
        className="self-start"
        onClick={listening ? microphone.stop : microphone.start}
        size="sm"
        variant="outline"
      >
        {listening ? (
          <MicrophoneSlashIcon data-icon="inline-start" />
        ) : (
          <MicrophoneIcon data-icon="inline-start" />
        )}
        {listening ? "Stop microphone" : "Use my microphone"}
      </Button>
    </div>
  );
};

export default LcdVuMeterMicrophone;
