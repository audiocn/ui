"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
  SpeakerHighIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useMemo } from "react";

import KnobsTile from "@/components/home/tiles/knobs-tile";
import {
  ChannelStrip,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripTitle,
  ChannelStripMeter,
  ChannelStripFader,
  ChannelStripValue,
  ChannelStripControls,
} from "@/components/ui/channel-strip";
import { MuteToggle, SoloToggle } from "@/components/ui/channel-toggle";
import { ElectricWaveform } from "@/components/ui/electric-waveform";
import { Fader } from "@/components/ui/fader";
import {
  LevelMeter,
  LevelMeterBar,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterHold,
  LevelMeterScale,
  LevelMeterTrack,
  LevelMeterValue,
} from "@/components/ui/level-meter";
import {
  Mixer,
  MixerChannels,
  MixerMaster,
  MixerSeparator,
} from "@/components/ui/mixer";
import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformMarker,
  WaveformRegion,
} from "@/components/ui/waveform";
import { formatDb } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";
import type { SocialPreviewName } from "@/lib/social-catalog";

import styles from "./social-card.module.css";

const PEAKS = Float32Array.from({ length: 320 }, (_, index) =>
  Math.min(
    1,
    0.12 + Math.abs(Math.sin(index * 0.21) * Math.cos(index * 0.047)) * 0.8
  )
);

const visualFrame: VisualFrame = {
  bands: Float32Array.from(
    { length: 64 },
    (_, index) => 0.2 + Math.abs(Math.sin(index * 0.27)) * 0.65
  ),
  history: PEAKS,
  historyLength: PEAKS.length,
  historyStart: 0,
  peakDb: -8,
  timeDomain: Float32Array.from(
    { length: 512 },
    (_, index) => Math.sin(index * 0.12) * Math.cos(index * 0.031) * 0.8
  ),
};

const createStillSource = <T,>(frame: T): FrameSource<T> => ({
  subscribe: (listener) => subscribeFrame(() => listener(frame)),
});

const visualSource = createStillSource(visualFrame);

const METER_CHANNELS = [0, 1];

const MIXER_CHANNELS = [
  { gainDb: 0, icon: MicrophoneIcon, peakDb: -12, title: "Mic" },
  { gainDb: -8, icon: DesktopIcon, peakDb: -21, title: "System" },
  { gainDb: -14, icon: MusicNotesIcon, peakDb: -7, title: "Music" },
  { gainDb: -4, icon: WaveformIcon, peakDb: -16, title: "Sounds" },
];

const MASTER_CHANNEL = {
  gainDb: 0,
  icon: SpeakerHighIcon,
  peakDb: -8,
  title: "Master",
};

const PreviewStrip = ({
  channel,
  master = false,
}: {
  channel: (typeof MIXER_CHANNELS)[number];
  master?: boolean;
}) => (
  <ChannelStrip variant={master ? "master" : "default"}>
    <ChannelStripHeader>
      <ChannelStripIcon>
        <channel.icon />
      </ChannelStripIcon>
      <ChannelStripTitle>{channel.title}</ChannelStripTitle>
    </ChannelStripHeader>
    <ChannelStripMeter>
      <LevelMeter
        aria-label={`${channel.title} level`}
        channels={[{ peakDb: channel.peakDb }, { peakDb: channel.peakDb - 3 }]}
        size="sm"
      />
    </ChannelStripMeter>
    <ChannelStripFader>
      <Fader
        aria-label={`${channel.title} volume`}
        size="sm"
        value={channel.gainDb}
      />
    </ChannelStripFader>
    <ChannelStripValue>{formatDb(channel.gainDb)}</ChannelStripValue>
    {master ? null : (
      <ChannelStripControls>
        <MuteToggle
          aria-label={`Mute ${channel.title}`}
          pressed={false}
          size="sm"
        >
          M
        </MuteToggle>
        <SoloToggle
          aria-label={`Solo ${channel.title}`}
          pressed={false}
          size="sm"
        >
          S
        </SoloToggle>
      </ChannelStripControls>
    )}
  </ChannelStrip>
);

const MixerPreview = () => (
  <Mixer
    aria-label="Mixer"
    aria-labelledby={undefined}
    className="w-full"
    orientation="vertical"
  >
    <MixerChannels>
      {MIXER_CHANNELS.map((channel) => (
        <PreviewStrip channel={channel} key={channel.title} />
      ))}
    </MixerChannels>
    <MixerSeparator />
    <MixerMaster>
      <PreviewStrip channel={MASTER_CHANNEL} master />
    </MixerMaster>
  </Mixer>
);

const StereoMeter = ({
  label,
  peakDb,
  variant = "solid",
}: {
  label: string;
  peakDb: number;
  variant?: "solid" | "segmented";
}) => {
  const source = useMemo(
    () =>
      createStillSource<MeterFrame>({
        channels: [
          { peakDb, rmsDb: peakDb - 6 },
          { peakDb: peakDb - 3, rmsDb: peakDb - 9 },
        ],
      }),
    [peakDb]
  );
  return (
    <div className="flex flex-col items-center gap-4">
      <LevelMeter
        aria-label={`${label} level`}
        className="h-64"
        orientation="vertical"
        size="lg"
        source={source}
        variant={variant}
      >
        <LevelMeterChannels>
          {METER_CHANNELS.map((index) => (
            <LevelMeterChannel index={index} key={index}>
              <LevelMeterTrack>
                <LevelMeterBar />
                <LevelMeterHold />
              </LevelMeterTrack>
            </LevelMeterChannel>
          ))}
          <LevelMeterScale />
        </LevelMeterChannels>
        <LevelMeterValue />
      </LevelMeter>
      <span className="text-muted-foreground font-mono text-sm">{label}</span>
    </div>
  );
};

const MeterPreview = () => (
  <div className="flex items-center justify-center gap-16">
    <StereoMeter label="Program" peakDb={-7} />
    <StereoMeter label="Voice" peakDb={-16} variant="segmented" />
  </div>
);

const WaveformPreview = () => (
  <div className="flex w-full flex-col gap-6">
    <div className="flex items-center justify-between font-mono text-sm">
      <span>Night Drive</span>
      <span className="text-muted-foreground">0:24 / 1:00</span>
    </div>
    <Waveform
      aria-label="Night Drive"
      className="h-36 w-full"
      duration={60}
      peaks={PEAKS}
      currentTime={24}
      variant="mirror"
    >
      <WaveformCanvas />
      <WaveformRegion end={38} start={24} />
      <WaveformMarker time={46}>Outro</WaveformMarker>
      <WaveformCursor />
    </Waveform>
    <span className="text-muted-foreground font-mono text-xs">
      SEEK / REGIONS / MARKERS
    </span>
  </div>
);

const HomePreview = () => (
  <div className={styles.homePreview}>
    <div className={styles.homeMixer}>
      <MixerPreview />
    </div>
    <div className={styles.homeKnobs}>
      <KnobsTile />
    </div>
    <div className={styles.homeWaveform}>
      <Waveform
        aria-label="Track waveform"
        className="h-20"
        duration={60}
        peaks={PEAKS}
        currentTime={24}
        variant="mirror"
      />
    </div>
  </div>
);

export const SocialPreview = ({ name }: { name: SocialPreviewName }) => {
  switch (name) {
    case "home": {
      return <HomePreview />;
    }
    case "mixer": {
      return <MixerPreview />;
    }
    case "meters": {
      return <MeterPreview />;
    }
    case "knobs": {
      return <KnobsTile />;
    }
    case "waveform": {
      return <WaveformPreview />;
    }
    case "electric-waveform": {
      return (
        <ElectricWaveform
          aria-label="Electric audio trace"
          className="h-64 w-full"
          intensity={0.7}
          source={visualSource}
        />
      );
    }
    default: {
      return null;
    }
  }
};
