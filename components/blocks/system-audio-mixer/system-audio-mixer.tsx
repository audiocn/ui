"use client";

import {
  ArrowCounterClockwiseIcon,
  DesktopIcon,
  GearSixIcon,
  MicrophoneIcon,
  MusicNotesIcon,
  PauseIcon,
  PlayIcon,
  SkipForwardIcon,
  SquaresFourIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { MixerMasterStrip } from "@/components/blocks/system-audio-mixer/mixer-master-strip";
import { MixerSourceStrip } from "@/components/blocks/system-audio-mixer/mixer-source-strip";
import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { Button } from "@/components/ui/button";
import { ChannelStripNotice } from "@/components/ui/channel-strip";
import type { ChannelStripStatusProps } from "@/components/ui/channel-strip";
import {
  Mixer,
  MixerActions,
  MixerChannels,
  MixerEmpty,
  MixerHeader,
  MixerMaster,
  MixerSeparator,
  MixerTitle,
} from "@/components/ui/mixer";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  SoundPad,
  SoundPadGrid,
  SoundPadLabel,
  SoundPadProgress,
  SoundPadShortcut,
} from "@/components/ui/sound-pad";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAudioContext } from "@/hooks/use-audio-context";
import { useAudioDevices } from "@/hooks/use-audio-devices";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import { useMicrophone } from "@/hooks/use-microphone";
import { useMixer } from "@/hooks/use-mixer";
import { useSound } from "@/hooks/use-sound";
import { useSystemAudio } from "@/hooks/use-system-audio";
import { useWebAudioMixer } from "@/hooks/use-web-audio-mixer";
import type { Orientation } from "@/lib/audio/types";

export type MixerSourceId = "microphone" | "system" | "music" | "sounds";

export interface MixerTrack {
  id: string;
  title: string;
  artist?: string;
  src: string;
}

export interface MixerSound {
  id: string;
  label: string;
  src: string | AudioBuffer;
  hotkey?: string;
  accent?: string;
}

export interface SystemAudioMixerProps {
  /** Rows or console strips. Default `horizontal`. */
  defaultOrientation?: Orientation;
  /** Which sources to show. Default all four. */
  sources?: MixerSourceId[];
  /** Save mixer settings to localStorage under this key. */
  persistKey?: string;
  /** The mixed output, for recording or streaming. */
  onOutputChange?: (stream: MediaStream | null) => void;
  /** Tracks for the music channel. */
  tracks?: MixerTrack[];
  /** Sounds for the sounds channel. */
  sounds?: MixerSound[];
  className?: string;
}

const ALL_SOURCES: MixerSourceId[] = [
  "microphone",
  "system",
  "music",
  "sounds",
];

const CHANNELS = [
  { id: "microphone" },
  { gainDb: -6, id: "system" },
  { gainDb: -12, id: "music", monitor: true },
  { gainDb: -6, id: "sounds", monitor: true },
];

interface Status {
  label: string;
  tone: ChannelStripStatusProps["tone"];
}

const MixerPad = ({
  sound,
  bus,
}: {
  sound: MixerSound;
  bus: AudioNode | null;
}) => {
  const player = useSound(sound.src, { destination: bus });
  return (
    <SoundPad
      accent={sound.accent}
      hotkey={sound.hotkey}
      loading={!player.isLoaded}
      onTrigger={player.play}
      playing={player.isPlaying}
      size="sm"
    >
      <SoundPadLabel>{sound.label}</SoundPadLabel>
      <SoundPadShortcut />
      <SoundPadProgress source={player.progress} />
    </SoundPad>
  );
};

export const SystemAudioMixer = ({
  defaultOrientation = "horizontal",
  sources = ALL_SOURCES,
  persistKey,
  onOutputChange,
  tracks = [],
  sounds = [],
  className,
}: SystemAudioMixerProps) => {
  const [orientation, setOrientation] =
    useState<Orientation>(defaultOrientation);
  const mixer = useMixer({ channels: CHANNELS, persistKey });
  const { context } = useAudioContext();

  const devices = useAudioDevices();
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const microphone = useMicrophone({ deviceId });
  const system = useSystemAudio();

  const [trackIndex, setTrackIndex] = useState(0);
  const track = tracks[trackIndex];
  const player = useAudioPlayer({
    onEnded: () =>
      setTrackIndex((index) =>
        tracks.length > 0 ? (index + 1) % tracks.length : 0
      ),
    src: track?.src,
  });

  const [soundBus, setSoundBus] = useState<GainNode | null>(null);
  useEffect(() => {
    if (!context) {
      return;
    }
    const bus = context.createGain();
    setSoundBus(bus);
    return () => {
      bus.disconnect();
      setSoundBus(null);
    };
  }, [context]);

  const graph = useWebAudioMixer(mixer, {
    ducking: { targets: ["music"], trigger: "microphone" },
    inputs: {
      microphone: microphone.stream,
      music: player.element,
      sounds: soundBus,
      system: system.stream,
    },
  });

  useEffect(() => {
    onOutputChange?.(graph.output);
  }, [graph.output, onOutputChange]);

  const show = (id: MixerSourceId) => sources.includes(id);
  const micActive = microphone.status === "active";
  const systemActive = system.status === "active";

  let micStatus: Status = { label: "Off", tone: "default" };
  if (micActive) {
    micStatus = mixer.isAudible("microphone")
      ? { label: "Live", tone: "live" }
      : { label: "Muted", tone: "muted" };
  } else if (microphone.status === "denied") {
    micStatus = { label: "Blocked", tone: "error" };
  }

  let systemStatus: Status = { label: "Off", tone: "default" };
  if (systemActive) {
    systemStatus = { label: "On", tone: "live" };
  } else if (system.status === "unsupported") {
    systemStatus = { label: "Unsupported", tone: "warning" };
  }

  const deviceLabel =
    devices.devices.find((device) => device.id === deviceId)?.label ??
    "Default microphone";

  return (
    <Mixer className={className} orientation={orientation}>
      <MixerHeader>
        <MixerTitle>Audio mixer</MixerTitle>
        <MixerActions>
          <Tabs
            onValueChange={(value) => setOrientation(value as Orientation)}
            value={orientation}
          >
            <TabsList>
              <TabsTrigger value="horizontal">Rows</TabsTrigger>
              <TabsTrigger value="vertical">Console</TabsTrigger>
            </TabsList>
          </Tabs>
          <Button
            aria-label="Reset mixer"
            onClick={mixer.reset}
            size="icon-sm"
            variant="ghost"
          >
            <ArrowCounterClockwiseIcon />
          </Button>
        </MixerActions>
      </MixerHeader>
      <MixerChannels>
        {show("microphone") ? (
          <MixerSourceStrip
            accent="var(--chart-2)"
            actions={
              <>
                <Popover>
                  <PopoverTrigger
                    render={
                      <Button
                        aria-label="Microphone settings"
                        size="icon-xs"
                        variant="ghost"
                      />
                    }
                  >
                    <GearSixIcon />
                  </PopoverTrigger>
                  <PopoverContent className="w-72">
                    <AudioDeviceSelect
                      devices={devices.devices}
                      loading={devices.isLoading}
                      onRequestPermission={devices.requestPermission}
                      onValueChange={setDeviceId}
                      permission={
                        devices.permission === "unsupported"
                          ? "denied"
                          : devices.permission
                      }
                      value={deviceId}
                    />
                  </PopoverContent>
                </Popover>
                <Button
                  onClick={micActive ? microphone.stop : microphone.start}
                  size="xs"
                  variant={micActive ? "secondary" : "outline"}
                >
                  {micActive ? "Stop" : "Start"}
                </Button>
              </>
            }
            description={micActive ? deviceLabel : "Not listening"}
            icon={<MicrophoneIcon />}
            id="microphone"
            meter={graph.meters.microphone ?? graph.master.meter}
            mixer={mixer}
            notice={
              microphone.status === "denied" ? (
                <ChannelStripNotice variant="destructive">
                  Microphone access is blocked. Allow it in your browser&apos;s
                  site settings.
                </ChannelStripNotice>
              ) : null
            }
            status={micStatus}
            title="Microphone"
          />
        ) : null}
        {show("system") ? (
          <MixerSourceStrip
            accent="var(--chart-4)"
            actions={
              <Switch
                aria-label="Capture system audio"
                checked={systemActive}
                disabled={!system.isSupported}
                onCheckedChange={(checked) =>
                  checked ? system.start() : system.stop()
                }
                size="sm"
              />
            }
            description={
              systemActive ? "Capturing" : "Share a screen or tab with audio"
            }
            icon={<DesktopIcon />}
            id="system"
            meter={graph.meters.system ?? graph.master.meter}
            mixer={mixer}
            monitorable={false}
            notice={
              system.status === "no-audio" ? (
                <ChannelStripNotice variant="warning">
                  Nothing to hear: tick &quot;Share audio&quot; in the
                  browser&apos;s picker.
                </ChannelStripNotice>
              ) : null
            }
            status={systemStatus}
            title="System audio"
          />
        ) : null}
        {show("music") ? (
          <MixerSourceStrip
            accent="var(--chart-1)"
            actions={
              <>
                <Button
                  aria-label={player.playing ? "Pause music" : "Play music"}
                  disabled={!track}
                  onClick={player.toggle}
                  size="icon-xs"
                  variant="ghost"
                >
                  {player.playing ? <PauseIcon /> : <PlayIcon />}
                </Button>
                <Button
                  aria-label="Next track"
                  disabled={tracks.length < 2}
                  onClick={() =>
                    setTrackIndex((trackIndex + 1) % tracks.length)
                  }
                  size="icon-xs"
                  variant="ghost"
                >
                  <SkipForwardIcon />
                </Button>
              </>
            }
            description={track ? track.title : "No tracks"}
            icon={<MusicNotesIcon />}
            id="music"
            meter={graph.meters.music ?? graph.master.meter}
            mixer={mixer}
            status={
              player.playing ? { label: "Playing", tone: "live" } : undefined
            }
            title="Music"
          />
        ) : null}
        {show("sounds") ? (
          <MixerSourceStrip
            accent="var(--chart-3)"
            actions={
              <Popover>
                <PopoverTrigger
                  render={
                    <Button
                      aria-label="Sound pads"
                      disabled={sounds.length === 0}
                      size="icon-xs"
                      variant="ghost"
                    />
                  }
                >
                  <SquaresFourIcon />
                </PopoverTrigger>
                <PopoverContent className="w-80">
                  <SoundPadGrid columns={4} hotkeys>
                    {sounds.map((sound) => (
                      <MixerPad bus={soundBus} key={sound.id} sound={sound} />
                    ))}
                  </SoundPadGrid>
                </PopoverContent>
              </Popover>
            }
            description={`${sounds.length} pads`}
            icon={<WaveformIcon />}
            id="sounds"
            meter={graph.meters.sounds ?? graph.master.meter}
            mixer={mixer}
            title="Sounds"
          />
        ) : null}
      </MixerChannels>
      <MixerEmpty>No audio sources.</MixerEmpty>
      <MixerSeparator />
      <MixerMaster>
        <MixerMasterStrip meter={graph.master.meter} mixer={mixer} />
      </MixerMaster>
    </Mixer>
  );
};
