"use client";

import { Button } from "@/components/ui/button";
import { LiveWaveform } from "@/components/ui/live-waveform";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const LiveWaveformMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream, { historySize: 120 });
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <LiveWaveform
        active={listening}
        aria-label="Microphone waveform"
        className="h-20"
        mode="scrolling"
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

export default LiveWaveformMicrophone;
