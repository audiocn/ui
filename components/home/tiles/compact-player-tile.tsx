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

const TRACK_INDEX = 2;

const CompactPlayerTile = () => {
  const tracks = useDemoTracks();
  const track = tracks.at(TRACK_INDEX);

  return (
    <AudioPlayer
      className="w-full rounded-full border py-1 pr-3 pl-1"
      src={track?.src}
    >
      <AudioPlayerPlay className="size-8">
        {({ playing }) =>
          playing ? <PauseIcon weight="fill" /> : <PlayIcon weight="fill" />
        }
      </AudioPlayerPlay>
      <AudioPlayerSeek />
      <AudioPlayerTime type="remaining" />
      {/* The rate would wrap the pill in the narrowest cards. */}
      <AudioPlayerRate className="@max-2xs:hidden" />
    </AudioPlayer>
  );
};

export default CompactPlayerTile;
