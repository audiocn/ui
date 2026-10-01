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
import { useEffect, useEffectEvent, useMemo, useState } from "react";

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
import type { AudioPlayerController } from "@/hooks/use-audio-player";
import { useMicrophone } from "@/hooks/use-microphone";
import type { UseMicrophoneResult } from "@/hooks/use-microphone";
import { useMixer } from "@/hooks/use-mixer";
import type { Mixer as MixerController } from "@/hooks/use-mixer";
import { useSound } from "@/hooks/use-sound";
import { useSystemAudio } from "@/hooks/use-system-audio";
import type { UseSystemAudioResult } from "@/hooks/use-system-audio";
import { useWebAudioMixer } from "@/hooks/use-web-audio-mixer";
import type { FrameSource, MeterFrame, Orientation } from "@/lib/audio/types";
import { cn } from "@/lib/utils";

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

const NO_TRACKS: MixerTrack[] = [];
const NO_SOUNDS: MixerSound[] = [];

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
      onTrigger={() => player.play()}
      playing={player.isPlaying}
      size="sm"
    >
      <SoundPadLabel>{sound.label}</SoundPadLabel>
      <SoundPadShortcut />
      <SoundPadProgress source={player.progress} />
    </SoundPad>
  );
};

interface ChannelProps {
  mixer: MixerController;
  meter: FrameSource<MeterFrame>;
}

const microphoneStatus = (
  microphone: UseMicrophoneResult,
  audible: boolean
): Status => {
  if (microphone.status === "active") {
    return audible
      ? { label: "Live", tone: "live" }
      : { label: "Muted", tone: "muted" };
  }
  if (microphone.status === "denied") {
    return { label: "Blocked", tone: "error" };
  }
  return { label: "Off", tone: "default" };
};

const systemStatus = (system: UseSystemAudioResult): Status => {
  if (system.status === "active") {
    return { label: "On", tone: "live" };
  }
  if (system.status === "unsupported") {
    return { label: "Unsupported", tone: "warning" };
  }
  return { label: "Off", tone: "default" };
};

const MicrophoneChannel = ({
  microphone,
  deviceId,
  onDeviceChange,
  mixer,
  meter,
}: ChannelProps & {
  microphone: UseMicrophoneResult;
  deviceId: string | null;
  onDeviceChange: (deviceId: string | null) => void;
}) => {
  const devices = useAudioDevices();
  const active = microphone.status === "active";
  const deviceLabel =
    devices.devices.find((device) => device.id === deviceId)?.label ??
    "Default microphone";

  return (
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
                onRequestPermission={() => devices.requestPermission()}
                onValueChange={onDeviceChange}
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
            onClick={active ? microphone.stop : microphone.start}
            size="xs"
            variant={active ? "secondary" : "outline"}
          >
            {active ? "Stop" : "Start"}
          </Button>
        </>
      }
      description={active ? deviceLabel : "Not listening"}
      icon={<MicrophoneIcon />}
      id="microphone"
      meter={meter}
      mixer={mixer}
      notice={
        microphone.status === "denied" ? (
          <ChannelStripNotice variant="destructive">
            Microphone access is blocked. Allow it in your browser&apos;s site
            settings.
          </ChannelStripNotice>
        ) : null
      }
      status={microphoneStatus(microphone, mixer.isAudible("microphone"))}
      title="Microphone"
    />
  );
};

const SystemChannel = ({
  system,
  mixer,
  meter,
}: ChannelProps & { system: UseSystemAudioResult }) => {
  const active = system.status === "active";
  return (
    <MixerSourceStrip
      accent="var(--chart-4)"
      actions={
        <Switch
          aria-label="Capture system audio"
          checked={active}
          disabled={!system.isSupported}
          onCheckedChange={(checked) =>
            checked ? system.start() : system.stop()
          }
          size="sm"
        />
      }
      description={active ? "Capturing" : "Share a screen or tab with audio"}
      icon={<DesktopIcon />}
      id="system"
      meter={meter}
      mixer={mixer}
      monitorable={false}
      notice={
        system.status === "no-audio" ? (
          <ChannelStripNotice variant="warning">
            Nothing to hear: tick &quot;Share audio&quot; in the browser&apos;s
            picker.
          </ChannelStripNotice>
        ) : null
      }
      status={systemStatus(system)}
      title="System audio"
    />
  );
};

const MusicChannel = ({
  player,
  track,
  trackCount,
  onNext,
  mixer,
  meter,
}: ChannelProps & {
  player: AudioPlayerController;
  track: MixerTrack | undefined;
  trackCount: number;
  onNext: () => void;
}) => (
  <MixerSourceStrip
    accent="var(--chart-1)"
    actions={
      <>
        <Button
          aria-label={player.playing ? "Pause music" : "Play music"}
          disabled={!track}
          onClick={() => player.toggle()}
          size="icon-xs"
          variant="ghost"
        >
          {player.playing ? <PauseIcon /> : <PlayIcon />}
        </Button>
        <Button
          aria-label="Next track"
          disabled={trackCount < 2}
          onClick={onNext}
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
    meter={meter}
    mixer={mixer}
    status={player.playing ? { label: "Playing", tone: "live" } : undefined}
    title="Music"
  />
);

const SoundsChannel = ({
  sounds,
  bus,
  mixer,
  meter,
}: ChannelProps & { sounds: MixerSound[]; bus: AudioNode | null }) => (
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
          <SoundPadGrid className="[--pad-min-width:4rem]" columns={4} hotkeys>
            {sounds.map((sound) => (
              <MixerPad bus={bus} key={sound.id} sound={sound} />
            ))}
          </SoundPadGrid>
        </PopoverContent>
      </Popover>
    }
    description={`${sounds.length} pads`}
    icon={<WaveformIcon />}
    id="sounds"
    meter={meter}
    mixer={mixer}
    title="Sounds"
  />
);

export const SystemAudioMixer = ({
  defaultOrientation = "horizontal",
  sources = ALL_SOURCES,
  persistKey,
  onOutputChange,
  tracks = NO_TRACKS,
  sounds = NO_SOUNDS,
  className,
}: SystemAudioMixerProps) => {
  const [orientation, setOrientation] =
    useState<Orientation>(defaultOrientation);
  const mixer = useMixer({ channels: CHANNELS, persistKey });
  const { context } = useAudioContext();

  const [deviceId, setDeviceId] = useState<string | null>(null);
  const microphone = useMicrophone({ deviceId });
  const system = useSystemAudio();

  const [trackIndex, setTrackIndex] = useState(0);
  const track = tracks[trackIndex];
  const nextTrack = () =>
    setTrackIndex((index) =>
      tracks.length > 0 ? (index + 1) % tracks.length : 0
    );
  const player = useAudioPlayer({ onEnded: nextTrack, src: track?.src });

  // Sound pads play into one bus so the mixer sees them as a single source.
  const soundBus = useMemo(() => context?.createGain() ?? null, [context]);

  const graph = useWebAudioMixer(mixer, {
    ducking: { targets: ["music"], trigger: "microphone" },
    inputs: {
      microphone: microphone.stream,
      music: player.element,
      sounds: soundBus,
      system: system.stream,
    },
  });

  // Fires when the stream changes, not on every render of the parent.
  const emitOutput = useEffectEvent((stream: MediaStream | null) => {
    onOutputChange?.(stream);
  });

  useEffect(() => {
    emitOutput(graph.output);
  }, [graph.output]);

  const show = (id: MixerSourceId) => sources.includes(id);
  const meterFor = (id: MixerSourceId) =>
    graph.meters[id] ?? graph.master.meter;

  return (
    <Mixer
      className={cn("[--channel-strip-header-width:16rem]", className)}
      orientation={orientation}
    >
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
            onClick={() => mixer.reset()}
            size="icon-sm"
            variant="ghost"
          >
            <ArrowCounterClockwiseIcon />
          </Button>
        </MixerActions>
      </MixerHeader>
      <MixerChannels>
        {show("microphone") ? (
          <MicrophoneChannel
            deviceId={deviceId}
            meter={meterFor("microphone")}
            microphone={microphone}
            mixer={mixer}
            onDeviceChange={setDeviceId}
          />
        ) : null}
        {show("system") ? (
          <SystemChannel
            meter={meterFor("system")}
            mixer={mixer}
            system={system}
          />
        ) : null}
        {show("music") ? (
          <MusicChannel
            meter={meterFor("music")}
            mixer={mixer}
            onNext={nextTrack}
            player={player}
            track={track}
            trackCount={tracks.length}
          />
        ) : null}
        {show("sounds") ? (
          <SoundsChannel
            bus={soundBus}
            meter={meterFor("sounds")}
            mixer={mixer}
            sounds={sounds}
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
