"use client";

import { SpeakerHighIcon, SpeakerXIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  VolumeControl,
  VolumeControlMute,
  VolumeControlSlider,
} from "@/components/ui/volume-control";

const VolumeControlPopover = () => {
  const [volume, setVolume] = useState(0.8);
  const [muted, setMuted] = useState(false);

  return (
    <Popover>
      <PopoverTrigger
        render={<Button aria-label="Volume" size="icon" variant="outline" />}
      >
        {muted || volume === 0 ? <SpeakerXIcon /> : <SpeakerHighIcon />}
      </PopoverTrigger>
      <PopoverContent className="w-auto p-3" side="top">
        <VolumeControl
          muted={muted}
          onMutedChange={setMuted}
          onValueChange={setVolume}
          orientation="vertical"
          value={volume}
        >
          <VolumeControlMute>
            {muted ? <SpeakerXIcon /> : <SpeakerHighIcon />}
          </VolumeControlMute>
          <VolumeControlSlider />
        </VolumeControl>
      </PopoverContent>
    </Popover>
  );
};

export default VolumeControlPopover;
