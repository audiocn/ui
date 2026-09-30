"use client";

import { Button } from "@/components/ui/button";
import { ElectricWaveform } from "@/components/ui/electric-waveform";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const ElectricWaveformMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-lg flex-col gap-4">
      <ElectricWaveform
        aria-label="Microphone"
        className="text-primary h-28"
        mode="scope"
        source={analyser.visual}
      />
      <Button
        className="self-start"
        onClick={listening ? microphone.stop : microphone.start}
        size="sm"
        variant="outline"
      >
        {listening ? "Stop microphone" : "Use my microphone"}
      </Button>
    </div>
  );
};

export default ElectricWaveformMicrophone;
