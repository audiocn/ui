"use client";

import { HeadphonesIcon } from "@phosphor-icons/react";
import { useState } from "react";

import {
  MonitorToggle,
  MuteToggle,
  SoloToggle,
} from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import { formatPan, PanControl } from "@/components/ui/pan-control";
import { formatDb } from "@/lib/audio/decibels";

const ChannelTile = () => {
  const [muted, setMuted] = useState(false);
  const [solo, setSolo] = useState(true);
  const [monitor, setMonitor] = useState(false);
  const [pan, setPan] = useState(-0.3);
  const [sendDb, setSendDb] = useState(-9);

  return (
    <div className="flex w-full flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-medium">Vocals</span>
        <div className="flex items-center gap-1.5">
          <MuteToggle
            aria-label="Mute vocals"
            onPressedChange={setMuted}
            pressed={muted}
          >
            M
          </MuteToggle>
          <SoloToggle
            aria-label="Solo vocals"
            onPressedChange={setSolo}
            pressed={solo}
          >
            S
          </SoloToggle>
          <MonitorToggle
            aria-label="Monitor vocals"
            onPressedChange={setMonitor}
            pressed={monitor}
            size="icon"
          >
            <HeadphonesIcon />
          </MonitorToggle>
        </div>
      </div>
      <div className="grid gap-2">
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Pan</span>
          <span className="font-mono">{formatPan(pan)}</span>
        </div>
        <PanControl
          aria-label="Vocals pan"
          onValueChange={setPan}
          value={pan}
        />
      </div>
      <div className="grid gap-2">
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Reverb send</span>
          <span className="font-mono">{formatDb(sendDb)}</span>
        </div>
        <Fader
          aria-label="Reverb send"
          onValueChange={setSendDb}
          size="sm"
          value={sendDb}
        />
      </div>
    </div>
  );
};

export default ChannelTile;
