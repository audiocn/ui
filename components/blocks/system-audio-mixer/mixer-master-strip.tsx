"use client";

import { SpeakerHighIcon } from "@phosphor-icons/react";

import {
  ChannelStrip,
  ChannelStripControls,
  ChannelStripDescription,
  ChannelStripFader,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripText,
  ChannelStripTitle,
  ChannelStripValue,
} from "@/components/ui/channel-strip";
import { MuteToggle } from "@/components/ui/channel-toggle";
import {
  Fader,
  FaderRange,
  FaderThumb,
  FaderTrack,
} from "@/components/ui/fader";
import {
  LevelMeter,
  LevelMeterBar,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterClip,
  LevelMeterHold,
  LevelMeterTrack,
} from "@/components/ui/level-meter";
import type { Mixer } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

const CHANNELS = [0, 1];

export interface MixerMasterStripProps {
  mixer: Mixer;
  meter: FrameSource<MeterFrame>;
}

export const MixerMasterStrip = ({ mixer, meter }: MixerMasterStripProps) => (
  <ChannelStrip muted={mixer.master.muted} variant="master">
    <ChannelStripHeader>
      <ChannelStripIcon>
        <SpeakerHighIcon />
      </ChannelStripIcon>
      <ChannelStripText>
        <ChannelStripTitle>Master</ChannelStripTitle>
        <ChannelStripDescription>Mix output</ChannelStripDescription>
      </ChannelStripText>
    </ChannelStripHeader>
    <ChannelStripMeter>
      <LevelMeter aria-label="Master level" className="h-full" source={meter}>
        <LevelMeterChannels>
          {CHANNELS.map((index) => (
            <LevelMeterChannel index={index} key={index}>
              <LevelMeterTrack>
                <LevelMeterBar />
                <LevelMeterHold />
              </LevelMeterTrack>
            </LevelMeterChannel>
          ))}
        </LevelMeterChannels>
        <LevelMeterClip showCount />
      </LevelMeter>
    </ChannelStripMeter>
    <ChannelStripFader>
      <Fader
        aria-label="Master volume"
        max={6}
        min={-60}
        onValueChange={mixer.setMasterGain}
        silenceAtMin
        size="sm"
        taper="audio"
        value={mixer.master.gainDb}
      >
        <FaderTrack>
          <FaderRange />
          <FaderThumb />
        </FaderTrack>
      </Fader>
    </ChannelStripFader>
    <ChannelStripValue>
      {formatDb(mixer.master.gainDb, { floorDb: -60 })}
    </ChannelStripValue>
    <ChannelStripControls>
      <MuteToggle
        aria-label="Mute master"
        onPressedChange={mixer.setMasterMuted}
        pressed={mixer.master.muted}
        size="sm"
      >
        M
      </MuteToggle>
    </ChannelStripControls>
  </ChannelStrip>
);
