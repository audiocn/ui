"use client";

import { Button } from "@/components/ui/button";
import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformHover,
} from "@/components/ui/waveform";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import { useWaveformData } from "@/hooks/use-waveform-data";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const WaveformDemo = () => {
  const [track] = useDemoTracks();
  const player = useAudioPlayer({ src: track?.src });
  const waveform = useWaveformData(track?.src ?? null);

  return (
    <div className="flex w-full max-w-lg flex-col gap-3">
      <Waveform
        aria-label="Night Drive"
        className="h-20"
        duration={waveform.duration}
        loading={waveform.status !== "ready"}
        onSeekCommitted={(value) => player.seek(value)}
        peaks={waveform.peaks}
        time={player.time}
      >
        <WaveformCanvas />
        <WaveformCursor />
        <WaveformHover />
      </Waveform>
      <Button
        className="self-start"
        disabled={!track}
        onClick={() => player.toggle()}
        size="sm"
        variant="outline"
      >
        {player.playing ? "Pause" : "Play"}
      </Button>
    </div>
  );
};

export default WaveformDemo;
