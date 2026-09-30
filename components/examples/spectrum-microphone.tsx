"use client";

import { Button } from "@/components/ui/button";
import { Spectrum } from "@/components/ui/spectrum";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const SpectrumMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream, { bands: 64, fftSize: 4096 });
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-lg flex-col gap-4">
      <Spectrum peakHold source={analyser.visual} />
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

export default SpectrumMicrophone;
