"use client";

import { MicrophoneSlashIcon } from "@phosphor-icons/react";

import { ChannelToggle, MuteToggle } from "@/components/ui/channel-toggle";

const variants = ["default", "outline", "ghost"] as const;

const ChannelToggleVariants = () => (
  <div className="grid gap-4">
    {variants.map((variant) => (
      <div className="flex items-center gap-2" key={variant}>
        <MuteToggle defaultPressed variant={variant}>
          <MicrophoneSlashIcon />
          Muted
        </MuteToggle>
        <MuteToggle variant={variant}>Mute</MuteToggle>
        <ChannelToggle aria-label="Record arm" defaultPressed variant={variant}>
          R
        </ChannelToggle>
        <span className="font-mono text-muted-foreground text-xs">{variant}</span>
      </div>
    ))}
  </div>
);

export default ChannelToggleVariants;
