"use client";

import { HeadphonesIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import {
  ChannelStrip,
  ChannelStripActions,
  ChannelStripControls,
  ChannelStripDescription,
  ChannelStripFader,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripStatus,
  ChannelStripText,
  ChannelStripTitle,
  ChannelStripValue,
} from "@/components/ui/channel-strip";
import type { ChannelStripStatusProps } from "@/components/ui/channel-strip";
import {
  MonitorToggle,
  MuteToggle,
  SoloToggle,
} from "@/components/ui/channel-toggle";
import {
  Fader,
  FaderRange,
  FaderThumb,
  FaderTrack,
} from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import type { Mixer } from "@/hooks/use-mixer";
import { formatDb } from "@/lib/audio/decibels";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

export interface MixerSourceStripProps {
  id: string;
  title: string;
  description?: ReactNode;
  icon: ReactNode;
  accent?: string;
  mixer: Mixer;
  meter: FrameSource<MeterFrame>;
  status?: { label: string; tone: ChannelStripStatusProps["tone"] };
  /** Buttons in the header. */
  actions?: ReactNode;
  /** A ChannelStripNotice, shown under the strip. */
  notice?: ReactNode;
  /** Show the monitor toggle. Default true. */
  monitorable?: boolean;
  disabled?: boolean;
}

export const MixerSourceStrip = ({
  id,
  title,
  description,
  icon,
  accent,
  mixer,
  meter,
  status,
  actions,
  notice,
  monitorable = true,
  disabled = false,
}: MixerSourceStripProps) => {
  const channel = mixer.channel(id);
  if (!channel) {
    return null;
  }

  return (
    <ChannelStrip
      accent={accent}
      dimmed={mixer.isDimmed(id)}
      disabled={disabled}
      muted={channel.muted}
      solo={channel.solo}
    >
      <ChannelStripHeader>
        <ChannelStripIcon>{icon}</ChannelStripIcon>
        <ChannelStripText>
          <ChannelStripTitle>{title}</ChannelStripTitle>
          {description ? (
            <ChannelStripDescription>{description}</ChannelStripDescription>
          ) : null}
        </ChannelStripText>
        {status ? (
          <ChannelStripStatus tone={status.tone}>
            {status.label}
          </ChannelStripStatus>
        ) : null}
        {actions ? <ChannelStripActions>{actions}</ChannelStripActions> : null}
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter
          aria-label={`${title} level`}
          channelCount={2}
          className="h-full"
          size="sm"
          source={meter}
        />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader
          aria-label={`${title} volume`}
          max={12}
          min={-60}
          onValueChange={(gainDb) => mixer.setGain(id, gainDb)}
          silenceAtMin
          size="sm"
          taper="audio"
          value={channel.gainDb}
        >
          <FaderTrack>
            <FaderRange />
            <FaderThumb />
          </FaderTrack>
        </Fader>
      </ChannelStripFader>
      <ChannelStripValue>
        {formatDb(channel.gainDb, { floorDb: -60 })}
      </ChannelStripValue>
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
        {monitorable ? (
          <MonitorToggle
            aria-label={`Monitor ${title}`}
            onPressedChange={(monitor) => mixer.setMonitor(id, monitor)}
            pressed={channel.monitor}
            size="sm"
          >
            <HeadphonesIcon />
          </MonitorToggle>
        ) : null}
      </ChannelStripControls>
      {notice}
    </ChannelStrip>
  );
};
