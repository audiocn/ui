"use client";

import { HeadphonesIcon } from "@phosphor-icons/react";
import { useState } from "react";

import {
  MonitorToggle,
  MuteToggle,
  SoloToggle,
} from "@/components/ui/channel-toggle";

const ChannelToggleDemo = () => {
  const [muted, setMuted] = useState(true);
  const [solo, setSolo] = useState(false);
  const [monitor, setMonitor] = useState(false);

  return (
    <div className="flex items-center gap-1.5">
      <MuteToggle onPressedChange={setMuted} pressed={muted}>
        M
      </MuteToggle>
      <SoloToggle onPressedChange={setSolo} pressed={solo}>
        S
      </SoloToggle>
      <MonitorToggle onPressedChange={setMonitor} pressed={monitor} size="icon">
        <HeadphonesIcon />
      </MonitorToggle>
    </div>
  );
};

export default ChannelToggleDemo;
