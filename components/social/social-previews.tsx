"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
  SpeakerHighIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useMemo } from "react";
import type { ComponentType } from "react";

import KnobsTile from "@/components/home/tiles/knobs-tile";
import { BarVisualizer } from "@/components/ui/bar-visualizer";
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
import { ElectricBarVisualizer } from "@/components/ui/electric-bar-visualizer";
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
import { LiveWaveform } from "@/components/ui/live-waveform";
import {
  Mixer,
  MixerChannels,
  MixerMaster,
  MixerSeparator,
} from "@/components/ui/mixer";
import { SmoothWaveform } from "@/components/ui/smooth-waveform";
import { Spectrum } from "@/components/ui/spectrum";
import {
  Waveform,
  WaveformCanvas,
  WaveformCursor,
  WaveformMarker,
  WaveformRegion,
} from "@/components/ui/waveform";
import { formatDb } from "@/lib/audio/decibels";
import type { MeterFrame } from "@/lib/audio/types";
import type { SocialPreviewName } from "@/lib/social-catalog";
import {
  createStillSource,
  socialPeaks,
  socialVisualSource,
} from "@/lib/social-signal";

import {
  MicSetupPreview,
  MusicPlayerPreview,
  PlayerPreview,
  QuickPopoverPreview,
  SoundboardPreview,
  SystemMixerPreview,
  SystemSettingsPreview,
} from "./block-previews";
import {
  ClipPreview,
  DevicesPreview,
  FadersPreview,
  PadsPreview,
  PanPreview,
  ParametersPreview,
  ReadoutPreview,
  ScalePreview,
  ThemingPreview,
  TogglesPreview,
  TracksPreview,
  VolumePreview,
} from "./control-previews";

import styles from "./social-card.module.css";

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
      peaks={socialPeaks}
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
        peaks={socialPeaks}
        currentTime={24}
        variant="mirror"
      />
    </div>
  </div>
);

const ElectricWaveformPreview = () => (
  <ElectricWaveform
    aria-label="Electric audio trace"
    className="h-64 w-full"
    intensity={0.7}
    source={socialVisualSource}
  />
);
const BarsPreview = () => (
  <BarVisualizer
    aria-label="Audio frequency bands"
    barCount={32}
    className="h-56 w-full"
    source={socialVisualSource}
  />
);
const ElectricBarsPreview = () => (
  <ElectricBarVisualizer
    aria-label="Electric frequency bars"
    barCount={24}
    barWidth={12}
    className="h-64 w-full"
    source={socialVisualSource}
  />
);
const LivePreview = () => (
  <div className="flex w-full flex-col gap-8">
    <LiveWaveform
      aria-label="Oscilloscope trace"
      className="h-24"
      source={socialVisualSource}
      variant="line"
      lineWidth={3}
    />
    <LiveWaveform
      aria-label="Signal history"
      className="h-24"
      mode="scrolling"
      source={socialVisualSource}
    />
  </div>
);
const SmoothPreview = () => (
  <div className="flex w-full flex-col gap-8">
    <SmoothWaveform
      aria-label="Flowing audio wave"
      className="h-24"
      source={socialVisualSource}
      lineWidth={3}
    />
    <SmoothWaveform
      aria-label="Oscilloscope line"
      className="h-24"
      mode="scope"
      source={socialVisualSource}
      lineWidth={3}
    />
  </div>
);
const SpectrumPreview = () => (
  <Spectrum className="h-64 w-full" peakHold source={socialVisualSource} />
);
const ChannelPreview = () => (
  <div className="w-full">
    <PreviewStrip channel={MIXER_CHANNELS[0]} />
  </div>
);
const CollectionPreview = () => (
  <div className={styles.collection}>
    <div className={styles.collectionKnobs}>
      <KnobsTile />
    </div>
    <div>
      <LevelMeter
        aria-label="Stereo level"
        channels={[{ peakDb: -7 }, { peakDb: -16 }]}
        size="lg"
      />
      <Fader aria-label="Gain" className="mt-5" value={-8} />
    </div>
    <div className={styles.collectionWaveform}>
      <Waveform
        aria-label="Track waveform"
        className="h-28"
        currentTime={24}
        duration={60}
        peaks={socialPeaks}
        variant="mirror"
      />
    </div>
  </div>
);
const BlocksPreview = () => (
  <div className="flex w-full flex-col gap-8">
    <MixerPreview />
    <PadsPreview />
  </div>
);

const previews: Record<SocialPreviewName, ComponentType> = {
  bars: BarsPreview,
  blocks: BlocksPreview,
  channel: ChannelPreview,
  clip: ClipPreview,
  collection: CollectionPreview,
  devices: DevicesPreview,
  "electric-bars": ElectricBarsPreview,
  "electric-waveform": ElectricWaveformPreview,
  faders: FadersPreview,
  home: HomePreview,
  knobs: KnobsTile,
  "live-waveform": LivePreview,
  meters: MeterPreview,
  "mic-setup": MicSetupPreview,
  mixer: MixerPreview,
  "music-player": MusicPlayerPreview,
  pads: PadsPreview,
  pan: PanPreview,
  parameters: ParametersPreview,
  player: PlayerPreview,
  "quick-popover": QuickPopoverPreview,
  readout: ReadoutPreview,
  scale: ScalePreview,
  "smooth-waveform": SmoothPreview,
  soundboard: SoundboardPreview,
  spectrum: SpectrumPreview,
  "system-mixer": SystemMixerPreview,
  "system-settings": SystemSettingsPreview,
  theming: ThemingPreview,
  toggles: TogglesPreview,
  tracks: TracksPreview,
  volume: VolumePreview,
  waveform: WaveformPreview,
};

export const SocialPreview = ({ name }: { name: SocialPreviewName }) => {
  const Preview = previews[name];
  return <Preview />;
};
