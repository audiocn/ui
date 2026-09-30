"use client";

import { MusicPlayer } from "@/components/blocks/music-player/music-player";
import { Button } from "@/components/ui/button";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useMicrophone } from "@/hooks/use-microphone";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const MusicPlayerDucking = () => {
  const tracks = useDemoTracks();
  const microphone = useMicrophone();
  const analyser = useAudioAnalyser(microphone.stream);
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-3">
      <Button
        className="self-start"
        onClick={listening ? microphone.stop : microphone.start}
        size="sm"
        variant="outline"
      >
        {listening ? "Stop microphone" : "Use my microphone to duck"}
      </Button>
      {tracks.length > 0 ? <MusicPlayer duckingSource={analyser.meter} tracks={tracks} /> : null}
    </div>
  );
};

export default MusicPlayerDucking;
