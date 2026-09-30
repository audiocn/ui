"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  LevelMeter,
  LevelMeterBar,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterClip,
  LevelMeterHold,
  LevelMeterTrack,
  LevelMeterValue,
} from "@/components/ui/level-meter";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const LevelMeterMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <LevelMeter aria-label="Microphone level" source={analyser.meter}>
        <LevelMeterChannels>
          <LevelMeterChannel>
            <LevelMeterTrack>
              <LevelMeterBar />
              <LevelMeterHold />
            </LevelMeterTrack>
          </LevelMeterChannel>
        </LevelMeterChannels>
        <LevelMeterValue />
        <LevelMeterClip />
      </LevelMeter>
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

export default LevelMeterMicrophone;
