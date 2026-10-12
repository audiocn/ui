"use client";

import { HeadphonesIcon } from "@phosphor-icons/react";

import PanControlDemo from "@/components/examples/pan-control-demo";
import RotarySelectorDemo from "@/components/examples/rotary-selector-demo";
import TrackListDemo from "@/components/examples/track-list-demo";
import VolumeControlDemo from "@/components/examples/volume-control-demo";
import EqTile from "@/components/home/tiles/eq-tile";
import {
  AudioDeviceSelect,
  AudioDeviceSelectPreview,
} from "@/components/ui/audio-device-select";
import {
  MonitorToggle,
  MuteToggle,
  SoloToggle,
} from "@/components/ui/channel-toggle";
import { ClipIndicator } from "@/components/ui/clip-indicator";
import { DbReadout } from "@/components/ui/db-readout";
import { DbScale } from "@/components/ui/db-scale";
import {
  Fader,
  FaderThumb,
  FaderTrack,
  FaderValue,
} from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import { LiveWaveform } from "@/components/ui/live-waveform";
import {
  SoundPad,
  SoundPadGrid,
  SoundPadLabel,
  SoundPadProgress,
  SoundPadShortcut,
} from "@/components/ui/sound-pad";
import type { MeterFrame } from "@/lib/audio/types";
import { createStillSource, socialVisualSource } from "@/lib/social-signal";

import styles from "./social-card.module.css";

const FADERS = [
  { gainDb: -3, label: "Drums", peakDb: -7 },
  { gainDb: -8, label: "Bass", peakDb: -19 },
  { gainDb: -14, label: "Keys", peakDb: -12 },
];

export const FadersPreview = () => (
  <div className="flex items-center justify-center gap-16">
    {FADERS.map((channel) => (
      <div className="flex flex-col items-center gap-5" key={channel.label}>
        <Fader
          aria-label={`${channel.label} gain`}
          className="h-60 flex-col items-center"
          orientation="vertical"
          size="lg"
          value={channel.gainDb}
          variant="console"
        >
          <FaderValue />
          <FaderTrack className="w-6 overflow-visible bg-transparent">
            <LevelMeter
              aria-label={`${channel.label} level`}
              className="absolute inset-0 h-full min-h-0"
              orientation="vertical"
              peakDb={channel.peakDb}
              size="sm"
            />
            <FaderThumb />
          </FaderTrack>
        </Fader>
        <span className="text-muted-foreground font-mono text-sm">
          {channel.label}
        </span>
      </div>
    ))}
  </div>
);

export const PanPreview = () => <PanControlDemo />;
export const RotarySelectorPreview = () => (
  <div className="scale-150">
    <RotarySelectorDemo />
  </div>
);
export const VolumePreview = () => <VolumeControlDemo />;
export const ParametersPreview = () => (
  <div className="w-96">
    <EqTile />
  </div>
);
export const TracksPreview = () => (
  <div className="w-full">
    <TrackListDemo />
  </div>
);

export const TogglesPreview = () => (
  <div className="flex items-center gap-6">
    <MuteToggle aria-label="Mute" pressed>
      M
    </MuteToggle>
    <SoloToggle aria-label="Solo" pressed>
      S
    </SoloToggle>
    <MonitorToggle aria-label="Monitor" pressed size="icon">
      <HeadphonesIcon />
    </MonitorToggle>
  </div>
);

const READOUTS = [
  { label: "Quiet", value: -42 },
  { label: "Program", value: -16 },
  { label: "Hot", value: -7 },
];

export const ReadoutPreview = () => (
  <div className="flex flex-col gap-7">
    {READOUTS.map((reading) => (
      <div
        className="flex items-center justify-between gap-16"
        key={reading.label}
      >
        <span className="text-muted-foreground font-mono text-sm">
          {reading.label}
        </span>
        <DbReadout
          className="data-[zone=warn]:text-meter-warn-foreground data-[zone=clip]:text-meter-clip-foreground text-3xl"
          value={reading.value}
        />
      </div>
    ))}
  </div>
);

export const ScalePreview = () => (
  <div className="flex h-64 items-center gap-10">
    <DbScale maxDb={6} minDb={-60} orientation="vertical" />
    <Fader
      aria-label="Program gain"
      className="h-full"
      orientation="vertical"
      size="lg"
      value={-8}
      variant="console"
    />
  </div>
);

const clipSource = createStillSource<MeterFrame>({ channels: [{ peakDb: 0 }] });

export const ClipPreview = () => (
  <div className="flex w-96 flex-col gap-8">
    <LevelMeter aria-label="Clipping program level" peakDb={0} />
    <div className="flex items-center justify-between">
      <DbReadout className="text-meter-clip-foreground text-2xl" value={0} />
      <ClipIndicator
        holdMs={Number.POSITIVE_INFINITY}
        showCount
        source={clipSource}
      />
    </div>
  </div>
);

const PADS = [
  { hotkey: "1", label: "Applause" },
  { hotkey: "2", label: "Air horn" },
  { hotkey: "3", label: "Drumroll" },
  { hotkey: "4", label: "Whoosh" },
  { hotkey: "5", label: "Chime" },
  { hotkey: "6", label: "Bleep" },
];
const progressSource = createStillSource(0.62);

export const PadsPreview = () => (
  <SoundPadGrid className="w-full" columns={3}>
    {PADS.map((pad) => (
      <SoundPad
        accent="var(--channel-monitor)"
        hotkey={pad.hotkey}
        key={pad.label}
        playing={pad.hotkey === "2"}
      >
        <SoundPadLabel>{pad.label}</SoundPadLabel>
        <SoundPadShortcut />
        {pad.hotkey === "2" ? (
          <SoundPadProgress source={progressSource} />
        ) : null}
      </SoundPad>
    ))}
  </SoundPadGrid>
);

const DEVICES = [
  { description: "USB microphone", id: "usb", label: "Studio Microphone" },
  { id: "default", isDefault: true, label: "Built-in Microphone" },
];

export const DevicesPreview = () => (
  <div className="flex w-96 flex-col gap-6">
    <span className="text-muted-foreground font-mono text-xs">MICROPHONE</span>
    <AudioDeviceSelect defaultValue="usb" devices={DEVICES} />
    <AudioDeviceSelectPreview>
      <LiveWaveform
        aria-label="Microphone signal"
        className="h-16"
        source={socialVisualSource}
      />
    </AudioDeviceSelectPreview>
    <AudioDeviceSelect defaultValue="default" devices={DEVICES} />
  </div>
);

export const ThemingPreview = () => (
  <div className={styles.themes}>
    <div className={styles.lightTheme}>
      <span className="font-mono text-xs">LIGHT / STONE</span>
      <LevelMeter aria-label="Light theme level" peakDb={-12} />
      <Fader aria-label="Light theme gain" value={-8} />
      <div className="flex gap-2">
        <MuteToggle pressed>M</MuteToggle>
        <SoloToggle pressed>S</SoloToggle>
      </div>
    </div>
    <div className={styles.darkTheme}>
      <span className="font-mono text-xs">DARK / STONE</span>
      <LevelMeter aria-label="Dark theme level" peakDb={-12} />
      <Fader aria-label="Dark theme gain" value={-8} />
      <div className="flex gap-2">
        <MuteToggle pressed>M</MuteToggle>
        <SoloToggle pressed>S</SoloToggle>
      </div>
    </div>
  </div>
);
