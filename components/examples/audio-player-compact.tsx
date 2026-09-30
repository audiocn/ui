"use client";

import { PauseIcon, PlayIcon } from "@phosphor-icons/react";

import {
  AudioPlayer,
  AudioPlayerPlay,
  AudioPlayerRate,
  AudioPlayerSeek,
  AudioPlayerTime,
} from "@/components/ui/audio-player";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const AudioPlayerCompact = () => {
  const tracks = useDemoTracks();
  const track = tracks[1];

  return (
    <AudioPlayer className="w-full max-w-md rounded-full border py-1 pr-3 pl-1" src={track?.src}>
      <AudioPlayerPlay className="size-8">
        {({ playing }) => (playing ? <PauseIcon weight="fill" /> : <PlayIcon weight="fill" />)}
      </AudioPlayerPlay>
      <AudioPlayerSeek />
      <AudioPlayerTime type="remaining" />
      <AudioPlayerRate />
    </AudioPlayer>
  );
};

export default AudioPlayerCompact;
