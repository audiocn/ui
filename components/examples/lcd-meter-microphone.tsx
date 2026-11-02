"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { LcdDisplay, LcdDisplayStatus } from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterBar,
  LcdMeterChannel,
  LcdMeterClip,
  LcdMeterHold,
  LcdMeterLabel,
  LcdMeterScale,
  LcdMeterTrack,
  LcdMeterValue,
} from "@/components/ui/lcd-meter";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const LcdMeterMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-96 flex-col gap-4">
      <LcdDisplay>
        <LcdMeter aria-label="Microphone level" source={analyser.meter}>
          <LcdMeterChannel>
            <LcdMeterLabel>Mic</LcdMeterLabel>
            <LcdMeterTrack>
              <LcdMeterBar />
              <LcdMeterHold />
            </LcdMeterTrack>
            <LcdMeterValue />
          </LcdMeterChannel>
          <LcdMeterScale />
          <LcdMeterClip />
        </LcdMeter>
        <LcdDisplayStatus state={listening ? "idle" : "off"} />
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
