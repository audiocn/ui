"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
  SpeakerHighIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";

import {
  ChannelStrip,
  ChannelStripControls,
  ChannelStripFader,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripTitle,
  ChannelStripValue,
} from "@/components/ui/channel-strip";
import { MuteToggle, SoloToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import {
  Mixer,
  MixerChannels,
  MixerMaster,
  MixerSeparator,
} from "@/components/ui/mixer";
import { useDemoSignal } from "@/hooks/use-demo-signal";
import type { DemoSignalKind } from "@/hooks/use-demo-signal";
import { useMixer } from "@/hooks/use-mixer";
import type { Mixer as MixerController } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";

interface Channel {
  id: string;
  /** The full name, for labels read aloud. */
  name: string;
  /** The short name on the strip. */
  title: string;
  icon: ReactNode;
  kind: DemoSignalKind;
  channels: number;
  gainDb: number;
  seed?: number;
}

const CHANNELS: Channel[] = [
  {
    channels: 1,
    gainDb: 0,
    icon: <MicrophoneIcon />,
    id: "mic",
    kind: "speech",
    name: "Microphone",
    title: "Mic",
  },
  {
    channels: 2,
    gainDb: -8,
    icon: <DesktopIcon />,
    id: "system",
    kind: "noise",
    name: "System audio",
    title: "System",
  },
  {
    channels: 2,
    gainDb: -14,
    icon: <MusicNotesIcon />,
    id: "music",
    kind: "music",
    name: "Music",
    title: "Music",
  },
  {
    channels: 1,
    gainDb: -4,
    icon: <WaveformIcon />,
    id: "sounds",
    kind: "speech",
    name: "Sounds",
    seed: 11,
    title: "Sounds",
  },
];

const INITIAL_CHANNELS = CHANNELS.map(({ id, gainDb }) => ({ gainDb, id }));

const Strip = ({
  channel,
  mixer,
}: {
  channel: Channel;
  mixer: MixerController;
}) => {
  const signal = useDemoSignal({
    channels: channel.channels,
    kind: channel.kind,
    seed: channel.seed,
  });
  const state = mixer.channel(channel.id);
  if (!state) {
    return null;
  }
  return (
    <ChannelStrip
      dimmed={mixer.isDimmed(channel.id)}
      muted={state.muted}
      solo={state.solo}
    >
      <ChannelStripHeader>
        <ChannelStripIcon>{channel.icon}</ChannelStripIcon>
        <ChannelStripTitle>{channel.title}</ChannelStripTitle>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter
          aria-label={`${channel.name} level`}
          channelCount={channel.channels}
          size="sm"
          source={signal.meter}
        />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader
          aria-label={`${channel.name} volume`}
          onValueChange={(gainDb) => mixer.setGain(channel.id, gainDb)}
          size="sm"
          value={state.gainDb}
        />
      </ChannelStripFader>
      <ChannelStripValue>{formatDb(state.gainDb)}</ChannelStripValue>
      <ChannelStripControls>
        <MuteToggle
          aria-label={`Mute ${channel.name}`}
          onPressedChange={(muted) => mixer.setMuted(channel.id, muted)}
          pressed={state.muted}
          size="sm"
        >
          M
        </MuteToggle>
        <SoloToggle
          aria-label={`Solo ${channel.name}`}
          onPressedChange={(solo) => mixer.setSolo(channel.id, solo)}
          pressed={state.solo}
          size="sm"
        >
          S
        </SoloToggle>
      </ChannelStripControls>
    </ChannelStrip>
  );
};

const MixerTile = () => {
  const mixer = useMixer({ channels: INITIAL_CHANNELS });
  const program = useDemoSignal({ channels: 2, kind: "music", seed: 9 });

  return (
    // The card label titles the tile, so the mixer is named here instead of
    // pointing at a MixerTitle it does not render.
    <Mixer
      aria-label="Mixer"
      aria-labelledby={undefined}
      className="w-full"
      orientation="vertical"
    >
      <MixerChannels>
        {CHANNELS.map((channel) => (
          <Strip channel={channel} key={channel.id} mixer={mixer} />
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
              channelCount={2}
              size="sm"
              source={program.meter}
            />
          </ChannelStripMeter>
          <ChannelStripFader>
            <Fader
              aria-label="Master volume"
              onValueChange={(gainDb) => mixer.setMasterGain(gainDb)}
              size="sm"
              value={mixer.master.gainDb}
            />
          </ChannelStripFader>
          <ChannelStripValue>{formatDb(mixer.master.gainDb)}</ChannelStripValue>
        </ChannelStrip>
      </MixerMaster>
    </Mixer>
  );
};

export default MixerTile;
