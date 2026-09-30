"use client";

import {
  MusicNotesIcon,
  PauseIcon,
  PlayIcon,
  RepeatIcon,
  RepeatOnceIcon,
  ShuffleIcon,
  SkipBackIcon,
  SkipForwardIcon,
  SpeakerHighIcon,
  SpeakerXIcon,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";

import {
  AudioPlayer,
  AudioPlayerArtwork,
  AudioPlayerControls,
  AudioPlayerDescription,
  AudioPlayerNext,
  AudioPlayerPlay,
  AudioPlayerPrevious,
  AudioPlayerTime,
  AudioPlayerTitle,
  AudioPlayerVolume,
} from "@/components/ui/audio-player";
import { Card, CardContent } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Label } from "@/components/ui/label";
import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderHeader,
  ParameterSliderLabel,
  ParameterSliderValue,
} from "@/components/ui/parameter-slider";
import { Switch } from "@/components/ui/switch";
import { Toggle } from "@/components/ui/toggle";
import {
  TrackList,
  TrackListItem,
  TrackListItemContent,
  TrackListItemDescription,
  TrackListItemDuration,
  TrackListItemIndex,
  TrackListItemTitle,
} from "@/components/ui/track-list";
import {
  VolumeControlMute,
  VolumeControlSlider,
} from "@/components/ui/volume-control";
import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformHover,
} from "@/components/ui/waveform";
import { getMediaElementSource } from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useWaveformData } from "@/hooks/use-waveform-data";
import { dbToGain } from "@/lib/audio/decibels";
import { formatTime } from "@/lib/audio/time";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

export interface MusicTrack {
  id: string;
  title: string;
  artist?: string;
  src: string;
  artwork?: string;
  /** Seconds, shown before the file loads. */
  duration?: number;
}

export interface MusicPlayerProps {
  tracks?: MusicTrack[];
  defaultTracks?: MusicTrack[];
  onTrackChange?: (track: MusicTrack) => void;
  /** A level source, usually the microphone, that ducks the music. */
  duckingSource?: FrameSource<MeterFrame> | null;
  /** Route the music into a mixer instead of the speakers. */
  output?: AudioNode | null;
  className?: string;
}

type Repeat = "off" | "all" | "one";

const NO_TRACKS: MusicTrack[] = [];
const DUCK_THRESHOLD_DB = -35;
const DUCK_ATTACK = 0.02;
const DUCK_RELEASE = 0.15;

const nextRepeat: Record<Repeat, Repeat> = {
  all: "one",
  off: "all",
  one: "off",
};

export const MusicPlayer = ({
  tracks: tracksProp,
  defaultTracks = NO_TRACKS,
  onTrackChange,
  duckingSource,
  output,
  className,
}: MusicPlayerProps) => {
  const tracks = tracksProp ?? defaultTracks;
  const [index, setIndex] = useState(0);
  const [shuffle, setShuffle] = useState(false);
  const [repeat, setRepeat] = useState<Repeat>("all");
  const [ducking, setDucking] = useState(true);
  const [duckAmountDb, setDuckAmountDb] = useState(-12);
  const track = tracks[index];

  const go = (direction: number) => {
    if (tracks.length === 0) {
      return;
    }
    let next = (index + direction + tracks.length) % tracks.length;
    if (shuffle && tracks.length > 1) {
      next =
        (index + 1 + Math.floor(Math.random() * (tracks.length - 1))) %
        tracks.length;
    }
    setIndex(next);
    const nextTrack = tracks[next];
    if (nextTrack) {
      onTrackChange?.(nextTrack);
    }
  };

  const player = useAudioPlayer({
    autoPlay: index > 0,
    loop: repeat === "one",
    onEnded: () => {
      if (repeat === "all" || index < tracks.length - 1) {
        go(1);
      }
    },
    src: track?.src,
  });
  const waveform = useWaveformData(track?.src ?? null, { samples: 400 });

  const { context } = useAudioContext();
  const duckGain = useMemo(() => context?.createGain() ?? null, [context]);
  const routed = output !== undefined || duckingSource !== undefined;

  useEffect(() => {
    if (!(context && duckGain && player.element && routed)) {
      return;
    }
    const source = getMediaElementSource(context, player.element);
    try {
      source.disconnect(context.destination);
    } catch {
      // Not connected to the speakers.
    }
    source.connect(duckGain);
    const target = output === undefined ? context.destination : output;
    if (target) {
      duckGain.connect(target);
    }
    return () => {
      source.disconnect(duckGain);
      if (target) {
        duckGain.disconnect(target);
      }
      source.connect(context.destination);
    };
  }, [context, duckGain, output, player.element, routed]);

  useFrameSource(
    duckingSource,
    (frame) => {
      if (!(duckGain && context)) {
        return;
      }
      let loudest = Number.NEGATIVE_INFINITY;
      for (const level of frame.channels) {
        loudest = Math.max(loudest, level.peakDb);
      }
      const active = ducking && loudest >= DUCK_THRESHOLD_DB;
      duckGain.gain.setTargetAtTime(
        active ? dbToGain(duckAmountDb) : 1,
        context.currentTime,
        active ? DUCK_ATTACK : DUCK_RELEASE
      );
    },
    { enabled: routed && Boolean(duckGain) }
  );

  if (tracks.length === 0) {
    return (
      <Empty className={className}>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <MusicNotesIcon />
          </EmptyMedia>
          <EmptyTitle>No music</EmptyTitle>
          <EmptyDescription>Add tracks to start playing.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <Card className={className}>
      <CardContent>
        <div className="flex flex-col gap-4">
          <AudioPlayer
            className="flex-col items-stretch gap-3"
            onNext={() => go(1)}
            onPrevious={() => go(-1)}
            player={player}
          >
            <div className="flex items-center gap-3">
              {track?.artwork ? (
                <AudioPlayerArtwork src={track.artwork} />
              ) : (
                <span className="bg-muted text-muted-foreground flex size-12 items-center justify-center rounded-lg">
                  <MusicNotesIcon className="size-5" />
                </span>
              )}
              <div className="flex min-w-0 flex-1 flex-col">
                <AudioPlayerTitle>{track?.title}</AudioPlayerTitle>
                <AudioPlayerDescription>{track?.artist}</AudioPlayerDescription>
              </div>
            </div>
            <Waveform
              className="h-14"
              currentTime={player.currentTime}
              duration={waveform.duration || player.duration}
              loading={waveform.status === "loading"}
              onSeekCommitted={(value) => player.seek(value)}
              peaks={waveform.peaks}
              time={player.time}
            >
              <WaveformCanvas />
              <WaveformCursor />
              <WaveformHover />
            </Waveform>
            <div className="flex items-center justify-between">
              <AudioPlayerTime />
              <AudioPlayerTime type="remaining" />
            </div>
            <div className="flex items-center justify-between gap-2">
              <Toggle
                aria-label="Shuffle"
                onPressedChange={setShuffle}
                pressed={shuffle}
                size="sm"
              >
                <ShuffleIcon />
              </Toggle>
              <AudioPlayerControls>
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
              <Toggle
                aria-label={`Repeat: ${repeat}`}
                onPressedChange={() => setRepeat(nextRepeat[repeat])}
                pressed={repeat !== "off"}
                size="sm"
              >
                {repeat === "one" ? <RepeatOnceIcon /> : <RepeatIcon />}
              </Toggle>
            </div>
            <AudioPlayerVolume>
              <VolumeControlMute>
                {player.muted ? <SpeakerXIcon /> : <SpeakerHighIcon />}
              </VolumeControlMute>
              <VolumeControlSlider />
            </AudioPlayerVolume>
          </AudioPlayer>
          <TrackList variant="outline">
            {tracks.map((item, position) => (
              <TrackListItem
                active={position === index}
                key={item.id}
                onSelect={() => {
                  setIndex(position);
                  onTrackChange?.(item);
                  if (position === index) {
                    player.toggle();
                  }
                }}
                playing={position === index && player.playing}
              >
                <TrackListItemIndex>{position + 1}</TrackListItemIndex>
                <TrackListItemContent>
                  <TrackListItemTitle>{item.title}</TrackListItemTitle>
                  {item.artist ? (
                    <TrackListItemDescription>
                      {item.artist}
                    </TrackListItemDescription>
                  ) : null}
                </TrackListItemContent>
                <TrackListItemDuration>
                  {item.duration ? formatTime(item.duration) : null}
                </TrackListItemDuration>
              </TrackListItem>
            ))}
          </TrackList>
          {duckingSource === undefined ? null : (
            <div className="flex flex-col gap-3 rounded-xl border p-3">
              <div className="flex items-center justify-between">
                <Label htmlFor="music-ducking">
                  Lower music while you talk
                </Label>
                <Switch
                  checked={ducking}
                  id="music-ducking"
                  onCheckedChange={setDucking}
                  size="sm"
                />
              </div>
              <ParameterSlider
                disabled={!ducking}
                max={0}
                min={-30}
                onValueChange={setDuckAmountDb}
                unit="dB"
                value={duckAmountDb}
              >
                <ParameterSliderHeader>
                  <ParameterSliderLabel>Amount</ParameterSliderLabel>
                  <ParameterSliderValue />
                </ParameterSliderHeader>
                <ParameterSliderControl />
              </ParameterSlider>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
};
