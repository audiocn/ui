"use client";

import { Button } from "@/components/ui/button";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useDemoSignal } from "@/hooks/use-demo-signal";
import { useLevel } from "@/hooks/use-level";
import { useMicrophone } from "@/hooks/use-microphone";
import { formatDb } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

const LevelRow = ({
  label,
  source,
}: {
  label: string;
  source: FrameSource<MeterFrame>;
}) => {
  const { peakDb, rmsDb, zone } = useLevel(source, { intervalMs: 100 });

  return (
    <div className="grid grid-cols-1 items-center gap-1 font-mono text-sm tabular-nums sm:grid-cols-[8rem_1fr_1fr_4rem] sm:gap-3">
      <span className="text-muted-foreground font-sans">{label}</span>
      <span>peak {formatDb(peakDb, { floorDb: -90 })}</span>
      <span>
        rms {formatDb(rmsDb ?? Number.NEGATIVE_INFINITY, { floorDb: -90 })}
      </span>
      <span className="text-muted-foreground">{zone}</span>
    </div>
  );
};

const FrameSourceDemo = () => {
  const demo = useDemoSignal({ kind: "speech" });
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-xl flex-col gap-4">
      <LevelRow label="Demo signal" source={demo.meter} />
      <LevelRow label="Microphone" source={analyser.meter} />
      <div className="flex flex-wrap items-center gap-3">
        <Button
          onClick={listening ? microphone.stop : microphone.start}
          size="sm"
          variant="outline"
        >
          {listening ? "Stop microphone" : "Use my microphone"}
        </Button>
        <span className="text-muted-foreground text-xs">
          Microphone: {microphone.status}
        </span>
      </div>
    </div>
  );
};

export default FrameSourceDemo;
