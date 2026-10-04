"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
  SpeakerHighIcon,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";

import {
  ChannelStrip,
  ChannelStripControls,
  ChannelStripFader,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripText,
  ChannelStripTitle,
  ChannelStripValue,
} from "@/components/ui/channel-strip";
import { MuteToggle, SoloToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import {
  Mixer,
  MixerChannels,
  MixerHeader,
  MixerMaster,
  MixerSeparator,
  MixerTitle,
} from "@/components/ui/mixer";
import { useMixer } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";
import { useDemoMixer } from "@/lib/docs/use-demo-mixer";

const channels = [
  { icon: <MicrophoneIcon />, id: "mic", kind: "speech", title: "Microphone" },
  {
    channels: 2,
    icon: <DesktopIcon />,
    id: "system",
    kind: "noise",
    title: "System audio",
  },
  {
    channels: 2,
    icon: <MusicNotesIcon />,
    id: "music",
    kind: "music",
    title: "Music",
  },
] as const;

const Strip = ({
  id,
  title,
  icon,
  source,
  mixer,
}: {
  id: string;
  title: string;
  icon: ReactNode;
  source: FrameSource<MeterFrame>;
  mixer: ReturnType<typeof useMixer>;
}) => {
  const channel = mixer.channel(id);
  if (!channel) {
    return null;
  }
  return (
    <ChannelStrip
      dimmed={mixer.isDimmed(id)}
      muted={channel.muted}
      solo={channel.solo}
    >
      <ChannelStripHeader>
        <ChannelStripIcon>{icon}</ChannelStripIcon>
        <ChannelStripText>
          <ChannelStripTitle>{title}</ChannelStripTitle>
        </ChannelStripText>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter
          aria-label={`${title} level`}
          ballistics={mixer.isAudible(id) ? undefined : "instant"}
          size="sm"
          source={source}
        />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader
          aria-label={`${title} volume`}
          onValueChange={(gainDb) => mixer.setGain(id, gainDb)}
          size="sm"
          value={channel.gainDb}
        />
      </ChannelStripFader>
      <ChannelStripValue>{formatDb(channel.gainDb)}</ChannelStripValue>
      <ChannelStripControls>
        <MuteToggle
          onPressedChange={(muted) => mixer.setMuted(id, muted)}
          pressed={channel.muted}
          size="sm"
        >
          M
        </MuteToggle>
        <SoloToggle
          onPressedChange={(solo) => mixer.setSolo(id, solo)}
          pressed={channel.solo}
          size="sm"
        >
          S
        </SoloToggle>
      </ChannelStripControls>
    </ChannelStrip>
  );
};

const MixerDemo = () => {
  const mixer = useMixer({ channels: channels.map(({ id }) => ({ id })) });
  const demo = useDemoMixer(channels, mixer.state);
  const audible = channels.some(({ id }) => mixer.isAudible(id));

  return (
    <Mixer className="w-full max-w-2xl">
      <MixerHeader>
        <MixerTitle>Audio mixer</MixerTitle>
      </MixerHeader>
      <MixerChannels>
        {channels.map((channel) => (
          <Strip
            icon={channel.icon}
            id={channel.id}
            key={channel.id}
            mixer={mixer}
            source={demo.sources[channel.id]}
            title={channel.title}
          />
        ))}
      </MixerChannels>
      <MixerSeparator />
      <MixerMaster>
        <ChannelStrip variant="master">
          <ChannelStripHeader>
            <ChannelStripIcon>
              <SpeakerHighIcon />
            </ChannelStripIcon>
            <ChannelStripTitle>Master</ChannelStripTitle>
          </ChannelStripHeader>
          <ChannelStripMeter>
            <LevelMeter
              aria-label="Master level"
              ballistics={audible ? undefined : "instant"}
              channelCount={2}
              size="sm"
              source={demo.master}
            />
          </ChannelStripMeter>
          <ChannelStripFader>
            <Fader
              aria-label="Master volume"
              onValueChange={(value) => mixer.setMasterGain(value)}
              size="sm"
              value={mixer.master.gainDb}
            />
          </ChannelStripFader>
        </ChannelStrip>
      </MixerMaster>
    </Mixer>
  );
};

export default MixerDemo;
