"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
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
import { useDemoSignal } from "@/hooks/use-demo-signal";
import type { DemoSignalKind } from "@/hooks/use-demo-signal";
import { useMixer } from "@/hooks/use-mixer";
import type { Mixer } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";

const INITIAL_CHANNELS = [{ id: "mic" }, { id: "music" }, { id: "game" }];

const Strip = ({
  id,
  mixer,
  title,
  icon,
  accent,
  kind,
  seed,
}: {
  id: string;
  mixer: Mixer;
  title: string;
  icon: ReactNode;
  accent: string;
  kind: DemoSignalKind;
  seed?: number;
}) => {
  const channel = mixer.channel(id);
  const signal = useDemoSignal({
    channels: 2,
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
      accent={accent}
      dimmed={mixer.isDimmed(id)}
      muted={channel.muted}
      solo={channel.solo}
      orientation="vertical"
      variant="card"
    >
      <ChannelStripHeader>
        <ChannelStripIcon>{icon}</ChannelStripIcon>
        <ChannelStripTitle>{title}</ChannelStripTitle>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter
          aria-label={`${title} level`}
          ballistics={mixer.isAudible(id) ? undefined : "instant"}
          channelCount={2}
          className="h-full"
          size="sm"
          source={signal.meter}
        />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader
          aria-label={`${title} volume`}
          onValueChange={(gainDb) => mixer.setGain(id, gainDb)}
          size="sm"
          taper="audio"
          value={channel.gainDb}
        />
      </ChannelStripFader>
      <ChannelStripValue>{formatDb(channel.gainDb)}</ChannelStripValue>
      <ChannelStripControls>
        <MuteToggle
          aria-label={`Mute ${title}`}
          onPressedChange={(muted) => mixer.setMuted(id, muted)}
          pressed={channel.muted}
          size="sm"
        >
          M
        </MuteToggle>
        <SoloToggle
          aria-label={`Solo ${title}`}
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

const ChannelStripConsole = () => {
  const mixer = useMixer({ channels: INITIAL_CHANNELS });
  return (
    <div className="flex h-96 max-w-full gap-3 overflow-x-auto">
      <Strip
        accent="var(--chart-2)"
        icon={<MicrophoneIcon />}
        id="mic"
        mixer={mixer}
        kind="speech"
        title="Mic"
      />
      <Strip
        accent="var(--chart-1)"
        icon={<MusicNotesIcon />}
        id="music"
        mixer={mixer}
        kind="music"
        title="Music"
      />
      <Strip
        accent="var(--chart-4)"
        icon={<DesktopIcon />}
        id="game"
        mixer={mixer}
        kind="noise"
        seed={3}
        title="Game"
      />
    </div>
  );
};

export default ChannelStripConsole;
