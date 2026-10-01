"use client";

import { PlayIcon, SkipBackIcon, SkipForwardIcon } from "@phosphor-icons/react";

import { MicSetup } from "@/components/blocks/mic-setup/mic-setup";
import { MusicPlayer } from "@/components/blocks/music-player/music-player";
import { QuickAudioPopover } from "@/components/blocks/quick-audio-popover/quick-audio-popover";
import { Soundboard } from "@/components/blocks/soundboard/soundboard";
import { SystemAudioMixer } from "@/components/blocks/system-audio-mixer/system-audio-mixer";
import { SystemAudioSettings } from "@/components/blocks/system-audio-settings/system-audio-settings";
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
import type { AudioPlayerController } from "@/hooks/use-audio-player";
import { useDemoSounds, useDemoTracks } from "@/lib/docs/use-demo-audio";
import { createStillSource } from "@/lib/social-signal";

import styles from "./social-card.module.css";

const noop = () => {
  // The poster displays a fixed playback state; its controls do not play audio.
};

const playerFixture: AudioPlayerController = {
  buffered: 60,
  currentTime: 24,
  duration: 60,
  element: null,
  error: null,
  loop: false,
  muted: false,
  pause: noop,
  play: () => Promise.resolve(),
  playbackRate: 1,
  playing: false,
  seek: noop,
  setLoop: noop,
  setMuted: noop,
  setPlaybackRate: noop,
  setVolume: noop,
  status: "paused",
  time: createStillSource(24),
  toggle: () => Promise.resolve(),
  volume: 0.75,
};

export const PlayerPreview = () => (
  <AudioPlayer
    className="w-full flex-col items-stretch gap-6"
    player={playerFixture}
  >
    <div className="flex flex-col gap-1">
      <AudioPlayerTitle>Night Drive</AudioPlayerTitle>
      <AudioPlayerDescription>
        audiocn / Late night sessions
      </AudioPlayerDescription>
    </div>
    <div className="flex items-center gap-4">
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
          <PlayIcon weight="fill" />
        </AudioPlayerPlay>
        <AudioPlayerSkipForward>
          <SkipForwardIcon />
        </AudioPlayerSkipForward>
      </AudioPlayerControls>
      <AudioPlayerVolume />
    </div>
  </AudioPlayer>
);

export const MicSetupPreview = () => <MicSetup className="w-full" />;
export const SystemSettingsPreview = () => (
  <SystemAudioSettings className="w-full" />
);

export const QuickPopoverPreview = () => (
  <div className={styles.quickPopover}>
    <QuickAudioPopover side="bottom" />
  </div>
);

export const MusicPlayerPreview = () => {
  const tracks = useDemoTracks();
  return (
    <div className="w-full" data-social-loaded={tracks.length > 0}>
      <MusicPlayer tracks={tracks} />
    </div>
  );
};

export const SoundboardPreview = () => {
  const sounds = useDemoSounds();
  return (
    <div className="w-full" data-social-loaded={sounds.length > 0}>
      <Soundboard sounds={sounds} />
    </div>
  );
};

export const SystemMixerPreview = () => {
  const tracks = useDemoTracks();
  const sounds = useDemoSounds();
  return (
    <div
      className="w-full"
      data-social-loaded={tracks.length > 0 && sounds.length > 0}
    >
      <SystemAudioMixer
        className="h-[30rem] w-full"
        defaultOrientation="vertical"
        sounds={sounds}
        tracks={tracks}
      />
    </div>
  );
};
