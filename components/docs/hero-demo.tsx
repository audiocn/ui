"use client";

import { MicrophoneIcon, MusicNotesIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import {
  ChannelStrip,
  ChannelStripControls,
  ChannelStripFader,
  ChannelStripHeader,
  ChannelStripIcon,
  ChannelStripMeter,
  ChannelStripStatus,
  ChannelStripText,
  ChannelStripTitle,
} from "@/components/ui/channel-strip";
import { MuteToggle, SoloToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import { LiveWaveform } from "@/components/ui/live-waveform";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const HeroDemo = () => {
  const voice = useDemoSignal({
    channels: 2,
    historySize: 120,
    kind: "speech",
  });
  const music = useDemoSignal({ channels: 2, kind: "music", seed: 4 });
  const [voiceDb, setVoiceDb] = useState(0);
  const [musicDb, setMusicDb] = useState(-12);
  const [voiceMuted, setVoiceMuted] = useState(false);

  return (
    <div className="bg-card grid w-full gap-3 rounded-2xl border p-3 shadow-sm sm:p-4">
      <div className="bg-muted/40 flex items-center gap-3 rounded-xl px-3 py-2">
        <BarVisualizer
          aria-hidden
          barCount={9}
          className="text-primary h-8 w-16"
          source={voice.visual}
        />
        <LiveWaveform
          aria-hidden
          barWidth={2}
          className="h-8 flex-1"
          mode="scrolling"
          source={voice.visual}
        />
      </div>
      <ChannelStrip muted={voiceMuted} size="sm">
        <ChannelStripHeader>
          <ChannelStripIcon>
            <MicrophoneIcon />
          </ChannelStripIcon>
          <ChannelStripText>
            <ChannelStripTitle>Microphone</ChannelStripTitle>
          </ChannelStripText>
          <ChannelStripStatus tone={voiceMuted ? "muted" : "live"}>
            {voiceMuted ? "Muted" : "Live"}
          </ChannelStripStatus>
        </ChannelStripHeader>
        <ChannelStripMeter>
          <LevelMeter
            aria-label="Microphone level"
            channelCount={2}
            size="sm"
            source={voice.meter}
          />
        </ChannelStripMeter>
        <ChannelStripFader>
          <Fader
            aria-label="Microphone volume"
            onValueChange={setVoiceDb}
            size="sm"
            value={voiceDb}
          />
        </ChannelStripFader>
        <ChannelStripControls>
          <MuteToggle
            onPressedChange={setVoiceMuted}
            pressed={voiceMuted}
            size="sm"
          >
            M
          </MuteToggle>
          <SoloToggle size="sm">S</SoloToggle>
        </ChannelStripControls>
      </ChannelStrip>
      <ChannelStrip size="sm">
        <ChannelStripHeader>
          <ChannelStripIcon>
            <MusicNotesIcon />
          </ChannelStripIcon>
          <ChannelStripText>
            <ChannelStripTitle>Music</ChannelStripTitle>
          </ChannelStripText>
        </ChannelStripHeader>
        <ChannelStripMeter>
          <LevelMeter
            aria-label="Music level"
            channelCount={2}
            size="sm"
            source={music.meter}
          />
        </ChannelStripMeter>
        <ChannelStripFader>
          <Fader
            aria-label="Music volume"
            onValueChange={setMusicDb}
            size="sm"
            value={musicDb}
          />
        </ChannelStripFader>
        <ChannelStripControls>
          <MuteToggle size="sm">M</MuteToggle>
          <SoloToggle size="sm">S</SoloToggle>
        </ChannelStripControls>
      </ChannelStrip>
    </div>
  );
};

export default HeroDemo;
