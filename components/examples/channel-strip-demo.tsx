"use client";

import { MicrophoneIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { MuteToggle, SoloToggle } from "@/components/ui/channel-toggle";
import {
  ChannelStrip,
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
import { DbReadout } from "@/components/ui/db-readout";
import { Fader } from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const ChannelStripDemo = () => {
  const signal = useDemoSignal({ kind: "speech" });
  const [gainDb, setGainDb] = useState(0);
  const [muted, setMuted] = useState(false);
  const [solo, setSolo] = useState(false);

  return (
    <ChannelStrip className="max-w-2xl" muted={muted} solo={solo}>
      <ChannelStripHeader>
        <ChannelStripIcon>
          <MicrophoneIcon />
        </ChannelStripIcon>
        <ChannelStripText>
          <ChannelStripTitle>Microphone</ChannelStripTitle>
          <ChannelStripDescription>Shure MV7+</ChannelStripDescription>
        </ChannelStripText>
        <ChannelStripStatus tone={muted ? "muted" : "live"}>{muted ? "Muted" : "Live"}</ChannelStripStatus>
      </ChannelStripHeader>
      <ChannelStripMeter>
        <LevelMeter aria-label="Microphone level" size="sm" source={signal.meter} />
      </ChannelStripMeter>
      <ChannelStripFader>
        <Fader aria-label="Microphone volume" onValueChange={setGainDb} size="sm" value={gainDb} />
      </ChannelStripFader>
      <ChannelStripValue>
        <DbReadout source={signal.meter} />
      </ChannelStripValue>
      <ChannelStripControls>
        <MuteToggle onPressedChange={setMuted} pressed={muted} size="sm">
          M
        </MuteToggle>
        <SoloToggle onPressedChange={setSolo} pressed={solo} size="sm">
          S
        </SoloToggle>
      </ChannelStripControls>
    </ChannelStrip>
  );
};

export default ChannelStripDemo;
