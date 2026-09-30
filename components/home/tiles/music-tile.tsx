"use client";

import {
  MusicNotesIcon,
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
} from "@phosphor-icons/react";
import { useState } from "react";

import {
  AudioPlayer,
  AudioPlayerControls,
  AudioPlayerDescription,
  AudioPlayerNext,
  AudioPlayerPlay,
  AudioPlayerPrevious,
  AudioPlayerSeek,
  AudioPlayerTime,
  AudioPlayerTitle,
} from "@/components/ui/audio-player";
import {
  TrackList,
  TrackListItem,
  TrackListItemContent,
  TrackListItemDescription,
  TrackListItemDuration,
  TrackListItemIndex,
  TrackListItemTitle,
} from "@/components/ui/track-list";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import { formatTime } from "@/lib/audio/time";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const MusicTile = () => {
  const tracks = useDemoTracks();
  const [index, setIndex] = useState(0);
  // Only a track the visitor picked starts on its own.
  const [picked, setPicked] = useState(false);
  const track = tracks[index];

  const go = (direction: number) => {
    if (tracks.length === 0) {
      return;
    }
    setIndex((index + direction + tracks.length) % tracks.length);
    setPicked(true);
  };

  const player = useAudioPlayer({
    autoPlay: picked,
    onEnded: () => go(1),
    src: track?.src,
  });

  return (
    <div className="@container w-full">
      <div className="grid gap-4 @lg:grid-cols-2">
        <AudioPlayer
          className="flex-col items-stretch gap-3"
          onNext={() => go(1)}
          onPrevious={() => go(-1)}
          player={player}
        >
          <div className="flex items-center gap-3">
            <span className="bg-muted text-muted-foreground flex size-12 shrink-0 items-center justify-center rounded-lg">
              <MusicNotesIcon className="size-5" />
            </span>
            <div className="flex min-w-0 flex-1 flex-col">
              <AudioPlayerTitle>{track?.title ?? "Loading…"}</AudioPlayerTitle>
              <AudioPlayerDescription>{track?.artist}</AudioPlayerDescription>
            </div>
          </div>
          <AudioPlayerSeek />
          <div className="flex items-center justify-between">
            <AudioPlayerTime />
            <AudioPlayerTime type="remaining" />
          </div>
          <AudioPlayerControls className="justify-center">
            <AudioPlayerPrevious>
              <SkipBackIcon />
            </AudioPlayerPrevious>
            <AudioPlayerPlay>
              {({ playing }) =>
                playing ? (
                  <PauseIcon weight="fill" />
                ) : (
                  <PlayIcon weight="fill" />
                )
              }
            </AudioPlayerPlay>
            <AudioPlayerNext>
              <SkipForwardIcon />
            </AudioPlayerNext>
          </AudioPlayerControls>
        </AudioPlayer>
        <TrackList variant="outline">
          {tracks.map((item, position) => (
            <TrackListItem
              active={position === index}
              key={item.id}
              onSelect={() => {
                if (position === index) {
                  player.toggle();
                  return;
                }
                setIndex(position);
                setPicked(true);
              }}
              playing={position === index && player.playing}
            >
              <TrackListItemIndex>{position + 1}</TrackListItemIndex>
              <TrackListItemContent>
                <TrackListItemTitle>{item.title}</TrackListItemTitle>
                <TrackListItemDescription>
                  {item.artist}
                </TrackListItemDescription>
              </TrackListItemContent>
              <TrackListItemDuration>
                {formatTime(item.duration)}
              </TrackListItemDuration>
            </TrackListItem>
          ))}
        </TrackList>
      </div>
    </div>
  );
};

export default MusicTile;
