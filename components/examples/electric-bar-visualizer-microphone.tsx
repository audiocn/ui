"use client";

import { Button } from "@/components/ui/button";
import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";

const ElectricBarVisualizerMicrophone = () => {
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-sm flex-col gap-4">
      <ElectricBarVisualizer
        align="end"
        aria-label="Microphone"
        className="text-primary h-24"
        idle="wave"
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

export default ElectricBarVisualizerMicrophone;
