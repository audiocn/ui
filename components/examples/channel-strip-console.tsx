"use client";

import {
  DesktopIcon,
  MicrophoneIcon,
  MusicNotesIcon,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { useState } from "react";

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
import { formatDb } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

const Strip = ({
  title,
  icon,
  accent,
  source,
}: {
  title: string;
  icon: ReactNode;
  accent: string;
  source: FrameSource<MeterFrame>;
}) => {
  const [gainDb, setGainDb] = useState(0);
  const [muted, setMuted] = useState(false);

  return (
    <ChannelStrip
      accent={accent}
      muted={muted}
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
          channelCount={2}
          className="h-full"
          size="sm"
          source={source}
        />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader
          aria-label={`${title} volume`}
          onValueChange={setGainDb}
          size="sm"
          taper="audio"
          value={gainDb}
        />
      </ChannelStripFader>
      <ChannelStripValue>{formatDb(gainDb)}</ChannelStripValue>
      <ChannelStripControls>
        <MuteToggle onPressedChange={setMuted} pressed={muted} size="sm">
          M
        </MuteToggle>
        <SoloToggle size="sm">S</SoloToggle>
      </ChannelStripControls>
    </ChannelStrip>
  );
};

const ChannelStripConsole = () => {
  const voice = useDemoSignal({ channels: 2, kind: "speech" });
  const music = useDemoSignal({ channels: 2, kind: "music" });
  const game = useDemoSignal({ channels: 2, kind: "noise", seed: 3 });

  return (
    <div className="flex h-96 max-w-full gap-3 overflow-x-auto">
      <Strip
        accent="var(--chart-2)"
        icon={<MicrophoneIcon />}
        source={voice.meter}
        title="Mic"
      />
      <Strip
        accent="var(--chart-1)"
        icon={<MusicNotesIcon />}
        source={music.meter}
        title="Music"
      />
      <Strip
        accent="var(--chart-4)"
        icon={<DesktopIcon />}
        source={game.meter}
        title="Game"
      />
    </div>
  );
};

export default ChannelStripConsole;
