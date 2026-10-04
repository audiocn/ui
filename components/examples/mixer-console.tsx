"use client";

import {
  ChannelStrip,
  ChannelStripControls,
  ChannelStripFader,
  ChannelStripHeader,
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
  MixerHeader,
  MixerTitle,
} from "@/components/ui/mixer";
import { useDemoSignal } from "@/hooks/use-demo-signal";
import type { DemoSignalKind } from "@/hooks/use-demo-signal";
import { useMixer } from "@/hooks/use-mixer";
import type { Mixer as MixerController } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";

const CHANNEL_COUNT = 16;
const KINDS: DemoSignalKind[] = ["speech", "music", "noise", "music"];

const CHANNELS = Array.from({ length: CHANNEL_COUNT }, (_, index) => ({
  id: `input-${index + 1}`,
  kind: KINDS[index % KINDS.length] ?? "music",
  seed: index + 1,
  title: `In ${index + 1}`,
}));

const INITIAL_CHANNELS = CHANNELS.map(({ id }) => ({ id }));

const ConsoleStrip = ({
  id,
  title,
  kind,
  seed,
  mixer,
}: {
  id: string;
  title: string;
  kind: DemoSignalKind;
  seed: number;
  mixer: MixerController;
}) => {
  const channel = mixer.channel(id);
  const signal = useDemoSignal({
    gainDb: channel?.gainDb,
    kind,
    playing: mixer.isAudible(id),
    seed,
  });
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
        <ChannelStripTitle>{title}</ChannelStripTitle>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter
          aria-label={`${title} level`}
          ballistics={mixer.isAudible(id) ? undefined : "instant"}
          size="sm"
          source={signal.meter}
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

const MixerConsole = () => {
  const mixer = useMixer({ channels: INITIAL_CHANNELS });

  return (
    <Mixer className="w-full" orientation="vertical">
      <MixerHeader>
        <MixerTitle>Console</MixerTitle>
      </MixerHeader>
      <MixerChannels>
        {CHANNELS.map((channel) => (
          <ConsoleStrip
            id={channel.id}
            key={channel.id}
            kind={channel.kind}
            mixer={mixer}
            seed={channel.seed}
            title={channel.title}
          />
        ))}
      </MixerChannels>
    </Mixer>
  );
};

export default MixerConsole;
