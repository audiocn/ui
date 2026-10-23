"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterChannel,
  LcdMeterChannels,
  LcdMeterClip,
  LcdMeterValue,
} from "@/components/ui/lcd-meter";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const LcdMeterMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <LcdDisplay>
        <LcdDisplayRow>
          <LcdDisplayLabel>Mic</LcdDisplayLabel>
          <LcdMeter source={analyser.meter}>
            <LcdMeterChannels>
              <LcdMeterChannel />
            </LcdMeterChannels>
            <LcdMeterValue />
            <LcdMeterClip />
          </LcdMeter>
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

export default LcdMeterMicrophone;
