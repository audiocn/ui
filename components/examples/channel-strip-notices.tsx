"use client";

import {
  DesktopIcon,
  MicrophoneSlashIcon,
  WarningIcon,
} from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  ChannelStrip,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripNotice,
  ChannelStripStatus,
  ChannelStripText,
  ChannelStripTitle,
} from "@/components/ui/channel-strip";
import { LevelMeter } from "@/components/ui/level-meter";

const ChannelStripNotices = () => (
  <div className="grid w-full max-w-2xl gap-3">
    <ChannelStrip disabled>
      <ChannelStripHeader>
        <ChannelStripIcon>
          <MicrophoneSlashIcon />
        </ChannelStripIcon>
        <ChannelStripText>
          <ChannelStripTitle>Microphone</ChannelStripTitle>
        </ChannelStripText>
        <ChannelStripStatus tone="error">Blocked</ChannelStripStatus>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter aria-label="Microphone level" size="sm" />
      </ChannelStripMeter>
      <ChannelStripNotice variant="destructive">
        Microphone access is blocked. Allow it in your browser&apos;s site
        settings.
      </ChannelStripNotice>
    </ChannelStrip>
    <ChannelStrip>
      <ChannelStripHeader>
        <ChannelStripIcon>
          <DesktopIcon />
        </ChannelStripIcon>
        <ChannelStripText>
          <ChannelStripTitle>System audio</ChannelStripTitle>
        </ChannelStripText>
        <ChannelStripStatus tone="warning">Paused</ChannelStripStatus>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter aria-label="System audio level" size="sm" />
      </ChannelStripMeter>
      <ChannelStripNotice variant="warning">
        <WarningIcon />
        <span className="flex-1">
          Paused to prevent an echo from your own stream.
        </span>
        <Button size="xs" variant="outline">
          Resume
        </Button>
      </ChannelStripNotice>
    </ChannelStrip>
  </div>
);

export default ChannelStripNotices;
