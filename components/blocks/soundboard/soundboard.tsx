"use client";

import {
  PlusIcon,
  SpeakerHighIcon,
  StopIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent } from "react";

import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuRadioGroup,
  ContextMenuRadioItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Label } from "@/components/ui/label";
import {
  SoundPad,
  SoundPadGrid,
  SoundPadLabel,
  SoundPadProgress,
  SoundPadShortcut,
} from "@/components/ui/sound-pad";
import type { SoundPadMode } from "@/components/ui/sound-pad";
import { Switch } from "@/components/ui/switch";
import {
  VolumeControl,
  VolumeControlMute,
  VolumeControlSlider,
} from "@/components/ui/volume-control";
import { useAudioContext } from "@/hooks/use-audio-context";
import { useSound } from "@/hooks/use-sound";

export interface SoundboardSound {
  id: string;
  label: string;
  src: string | AudioBuffer;
  hotkey?: string;
  mode?: SoundPadMode;
  /** 0..1. Default 1. */
  volume?: number;
  accent?: string;
}

export interface SoundboardProps {
  sounds?: SoundboardSound[];
  defaultSounds?: SoundboardSound[];
  onSoundsChange?: (sounds: SoundboardSound[]) => void;
  /** Route pads into a mixer instead of the speakers. */
  output?: AudioNode | null;
  /** Default 4. */
  columns?: number;
  className?: string;
}

const MODES: { value: SoundPadMode; label: string }[] = [
  { label: "One shot", value: "one-shot" },
  { label: "Toggle", value: "toggle" },
  { label: "Hold", value: "hold" },
  { label: "Loop", value: "loop" },
];

const VOLUMES = [1, 0.75, 0.5, 0.25];

interface PadProps {
  sound: SoundboardSound;
  bus: AudioNode | null;
  stopSignal: number;
  onChange: (sound: SoundboardSound) => void;
  onRemove: () => void;
}

const Pad = ({ sound, bus, stopSignal, onChange, onRemove }: PadProps) => {
  const mode = sound.mode ?? "one-shot";
  const player = useSound(sound.src, {
    destination: bus,
    interrupt: mode !== "one-shot",
    loop: mode === "loop",
    volume: sound.volume ?? 1,
  });
  const { stop } = player;

  useEffect(() => {
    if (stopSignal > 0) {
      stop();
    }
  }, [stop, stopSignal]);

  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <SoundPad
            accent={sound.accent}
            hotkey={sound.hotkey}
            loading={!player.isLoaded}
            mode={mode}
            onStop={player.stop}
            onTrigger={player.play}
            playing={player.isPlaying}
          />
        }
      >
        <SoundPadLabel>{sound.label}</SoundPadLabel>
        <SoundPadShortcut />
        <SoundPadProgress source={player.progress} />
      </ContextMenuTrigger>
      <ContextMenuContent className="w-48">
        <ContextMenuGroup>
          <ContextMenuLabel>Mode</ContextMenuLabel>
          <ContextMenuRadioGroup
            onValueChange={(value) =>
              onChange({ ...sound, mode: value as SoundPadMode })
            }
            value={mode}
          >
            {MODES.map((option) => (
              <ContextMenuRadioItem key={option.value} value={option.value}>
                {option.label}
              </ContextMenuRadioItem>
            ))}
          </ContextMenuRadioGroup>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuLabel>Volume</ContextMenuLabel>
          <ContextMenuRadioGroup
            onValueChange={(value) =>
              onChange({ ...sound, volume: Number(value) })
            }
            value={String(sound.volume ?? 1)}
          >
            {VOLUMES.map((volume) => (
              <ContextMenuRadioItem key={volume} value={String(volume)}>
                {Math.round(volume * 100)}%
              </ContextMenuRadioItem>
            ))}
          </ContextMenuRadioGroup>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem onClick={onRemove} variant="destructive">
            Remove
          </ContextMenuItem>
        </ContextMenuGroup>
      </ContextMenuContent>
    </ContextMenu>
  );
};

const AUDIO_FILE = /^audio\//;

const fileToSound = (file: File, index: number): SoundboardSound => ({
  hotkey: index < 9 ? String(index + 1) : undefined,
  id: `${file.name}-${file.lastModified}`,
  label: file.name.replace(/\.[^.]+$/, ""),
  src: URL.createObjectURL(file),
});

export const Soundboard = ({
  sounds: soundsProp,
  defaultSounds = [],
  onSoundsChange,
  output,
  columns = 4,
  className,
}: SoundboardProps) => {
  const [soundsState, setSoundsState] = useState(defaultSounds);
  const sounds = soundsProp ?? soundsState;
  const { context } = useAudioContext();
  const [bus, setBus] = useState<GainNode | null>(null);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const [hotkeys, setHotkeys] = useState(true);
  const [stopSignal, setStopSignal] = useState(0);
  const [dragging, setDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!context) {
      return;
    }
    const gain = context.createGain();
    setBus(gain);
    return () => {
      gain.disconnect();
      setBus(null);
    };
  }, [context]);

  useEffect(() => {
    if (!(bus && context)) {
      return;
    }
    const target = output === undefined ? context.destination : output;
    if (!target) {
      return;
    }
    bus.connect(target);
    return () => {
      bus.disconnect(target);
    };
  }, [bus, context, output]);

  useEffect(() => {
    if (bus && context) {
      bus.gain.setTargetAtTime(
        muted ? 0 : volume ** 2,
        context.currentTime,
        0.01
      );
    }
  }, [bus, context, muted, volume]);

  const update = (next: SoundboardSound[]) => {
    setSoundsState(next);
    onSoundsChange?.(next);
  };

  const addFiles = (files: FileList | null) => {
    if (!files) {
      return;
    }
    const added = [...files]
      .filter((file) => AUDIO_FILE.test(file.type))
      .map((file, index) => fileToSound(file, sounds.length + index));
    if (added.length > 0) {
      update([...sounds, ...added]);
    }
  };

  const dropProps = useMemo(
    () => ({
      onDragLeave: () => setDragging(false),
      onDragOver: (event: DragEvent<HTMLDivElement>) => {
        event.preventDefault();
        setDragging(true);
      },
    }),
    []
  );

  return (
    <div
      className={className}
      data-dragging={dragging ? "" : undefined}
      data-slot="soundboard"
      {...dropProps}
      onDrop={(event) => {
        event.preventDefault();
        setDragging(false);
        addFiles(event.dataTransfer.files);
      }}
    >
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <h3 className="font-heading mr-auto font-medium">Soundboard</h3>
        <div className="flex items-center gap-2">
          <Switch
            checked={hotkeys}
            id="soundboard-hotkeys"
            onCheckedChange={setHotkeys}
            size="sm"
          />
          <Label
            className="text-muted-foreground text-xs"
            htmlFor="soundboard-hotkeys"
          >
            Hotkeys
          </Label>
        </div>
        <VolumeControl
          className="w-36"
          muted={muted}
          onMutedChange={setMuted}
          onValueChange={setVolume}
          value={volume}
        >
          <VolumeControlMute>
            <SpeakerHighIcon />
          </VolumeControlMute>
          <VolumeControlSlider />
        </VolumeControl>
        <Button
          onClick={() => setStopSignal((signal) => signal + 1)}
          size="sm"
          variant="outline"
        >
          <StopIcon data-icon="inline-start" />
          Stop all
        </Button>
      </div>
      {sounds.length === 0 ? (
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <WaveformIcon />
            </EmptyMedia>
            <EmptyTitle>No sounds yet</EmptyTitle>
            <EmptyDescription>
              Drop audio files here, or add them from your computer.
            </EmptyDescription>
          </EmptyHeader>
          <Button
            onClick={() => fileInputRef.current?.click()}
            size="sm"
            variant="outline"
          >
            <PlusIcon data-icon="inline-start" />
            Add sounds
          </Button>
        </Empty>
      ) : (
        <SoundPadGrid
          className="data-dragging:ring-2"
          columns={columns}
          hotkeyScope="global"
          hotkeys={hotkeys}
        >
          {sounds.map((sound) => (
            <Pad
              bus={bus}
              key={sound.id}
              onChange={(next) =>
                update(
                  sounds.map((item) => (item.id === sound.id ? next : item))
                )
              }
              onRemove={() =>
                update(sounds.filter((item) => item.id !== sound.id))
              }
              sound={sound}
              stopSignal={stopSignal}
            />
          ))}
          <SoundPad
            aria-label="Add sounds"
            className="text-muted-foreground items-center justify-center border-dashed"
            onTrigger={() => fileInputRef.current?.click()}
            variant="outline"
          >
            <PlusIcon className="size-5" />
            <SoundPadLabel>Add</SoundPadLabel>
          </SoundPad>
        </SoundPadGrid>
      )}
      <input
        accept="audio/*"
        className="hidden"
        multiple
        onChange={(event) => {
          addFiles(event.target.files);
          event.target.value = "";
        }}
        ref={fileInputRef}
        type="file"
      />
    </div>
  );
};
