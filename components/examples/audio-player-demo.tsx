"use client";

import {
  MusicNotesIcon,
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
} from "@phosphor-icons/react";

import {
  AudioPlayer,
  AudioPlayerControls,
  AudioPlayerDescription,
  AudioPlayerPlay,
  AudioPlayerSeek,
  AudioPlayerSkipBack,
  AudioPlayerSkipForward,
  AudioPlayerTime,
  AudioPlayerTitle,
  AudioPlayerVolume,
} from "@/components/ui/audio-player";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const AudioPlayerDemo = () => {
  const [track] = useDemoTracks();

  return (
    <AudioPlayer
      className="w-full max-w-md flex-col items-stretch rounded-xl border p-4"
      src={track?.src}
    >
      <div className="flex items-center gap-3">
        <span className="bg-muted text-muted-foreground flex size-12 items-center justify-center rounded-lg">
          <MusicNotesIcon className="size-5" />
        </span>
        <div className="flex min-w-0 flex-col">
          <AudioPlayerTitle>{track?.title ?? "Loading…"}</AudioPlayerTitle>
          <AudioPlayerDescription>{track?.artist}</AudioPlayerDescription>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <AudioPlayerTime />
        <AudioPlayerSeek />
        <AudioPlayerTime type="remaining" />
      </div>
      <div className="flex items-center justify-between">
        <AudioPlayerControls>
          <AudioPlayerSkipBack>
            <SkipBackIcon />
          </AudioPlayerSkipBack>
          <AudioPlayerPlay>
            {({ playing }) =>
              playing ? <PauseIcon weight="fill" /> : <PlayIcon weight="fill" />
            }
          </AudioPlayerPlay>
          <AudioPlayerSkipForward>
            <SkipForwardIcon />
          </AudioPlayerSkipForward>
        </AudioPlayerControls>
        <AudioPlayerVolume />
      </div>
    </AudioPlayer>
  );
};

export default AudioPlayerDemo;
