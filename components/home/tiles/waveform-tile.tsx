"use client";

import { PauseIcon, PlayIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformHover,
  WaveformMarker,
  WaveformRegion,
} from "@/components/ui/waveform";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import { useWaveformData } from "@/hooks/use-waveform-data";
import { formatTime } from "@/lib/audio/time";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const TRACK_INDEX = 1;
const MARKER_TIME = 17;

const WaveformTile = () => {
  const tracks = useDemoTracks();
  const track = tracks.at(TRACK_INDEX);
  const player = useAudioPlayer({ src: track?.src });
  const waveform = useWaveformData(track?.src ?? null, { samples: 400 });
  const [clip, setClip] = useState({ end: 13, start: 5 });

  return (
    <div className="flex w-full flex-col gap-3">
      <div className="flex items-center gap-3">
        <Button
          aria-label={player.playing ? "Pause" : "Play"}
          disabled={!track}
          onClick={() => player.toggle()}
          size="icon"
          variant="outline"
        >
          {player.playing ? (
            <PauseIcon weight="fill" />
          ) : (
            <PlayIcon weight="fill" />
          )}
        </Button>
        <div className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium">
            {track?.title ?? "Loading…"}
          </span>
          <span className="text-muted-foreground font-mono text-xs">
            Clip {formatTime(clip.start)} – {formatTime(clip.end)}
          </span>
        </div>
      </div>
      <Waveform
        aria-label={track?.title ?? "Track"}
        className="h-24"
        duration={waveform.duration}
        loading={waveform.status !== "ready"}
        onSeekCommitted={(time) => player.seek(time)}
        peaks={waveform.peaks}
        time={player.time}
        variant="mirror"
      >
        <WaveformCanvas />
        <WaveformRegion
          end={clip.end}
          onValueChange={setClip}
          start={clip.start}
        />
        <WaveformMarker time={MARKER_TIME}>Outro</WaveformMarker>
        <WaveformCursor />
        <WaveformHover />
      </Waveform>
    </div>
  );
};

export default WaveformTile;
