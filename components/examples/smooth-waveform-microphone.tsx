"use client";

import { Button } from "@/components/ui/button";
import { SmoothWaveform } from "@/components/ui/smooth-waveform";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const SmoothWaveformMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-lg flex-col gap-4">
      <SmoothWaveform
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

export default SmoothWaveformMicrophone;
