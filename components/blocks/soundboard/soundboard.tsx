"use client";

import {
  PlusIcon,
  SpeakerHighIcon,
  StopIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import { useId, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { DragEvent, Ref } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
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
import { useGainNode } from "@/hooks/use-gain-node";
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

interface PadHandle {
  stop: () => void;
}

interface PadProps {
  ref?: Ref<PadHandle>;
  sound: SoundboardSound;
  bus: AudioNode | null;
  onChange: (sound: SoundboardSound) => void;
  onRemove: () => void;
}

const Pad = ({ ref, sound, bus, onChange, onRemove }: PadProps) => {
  const mode = sound.mode ?? "one-shot";
  const player = useSound(sound.src, {
    destination: bus,
    interrupt: mode !== "one-shot",
    loop: mode === "loop",
    volume: sound.volume ?? 1,
  });
  const { stop } = player;
  useImperativeHandle(ref, () => ({ stop }), [stop]);

  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <SoundPad
            accent={sound.accent}
            hotkey={sound.hotkey}
            loading={!player.isLoaded}
            mode={mode}
            onStop={() => player.stop()}
            onTrigger={() => player.play()}
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

const AUDIO_FILE = /^audio\//u;
const FILE_EXTENSION = /\.[^.]+$/u;
const NO_SOUNDS: SoundboardSound[] = [];

const fileToSound = (
  file: File,
  index: number
): SoundboardSound & { src: string } => ({
  hotkey: index < 9 ? String(index + 1) : undefined,
  id: `${file.name}-${file.lastModified}`,
  label: file.name.replace(FILE_EXTENSION, ""),
  src: URL.createObjectURL(file),
});

export const Soundboard = ({
  sounds: soundsProp,
  defaultSounds = NO_SOUNDS,
  onSoundsChange,
  output,
  columns = 4,
  className,
}: SoundboardProps) => {
  const [soundsState, setSoundsState] = useState(defaultSounds);
  const sounds = soundsProp ?? soundsState;
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const bus = useGainNode({
    destination: output,
    gain: muted ? 0 : volume ** 2,
  });
  const [hotkeys, setHotkeys] = useState(true);
  const [dragging, setDragging] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [removedSound, setRemovedSound] = useState<{
    sound: SoundboardSound;
    index: number;
  } | null>(null);
  const hotkeysId = useId();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const padsRef = useRef(new Map<string, PadHandle>());
  // Object URLs this board created for dropped files.
  const objectUrlsRef = useRef(new Set<string>());

  const update = (next: SoundboardSound[]) => {
    setSoundsState(next);
    onSoundsChange?.(next);
  };

  const addFiles = (files: FileList | null) => {
    if (!files || files.length === 0) {
      return;
    }
    const existingIds = new Set(sounds.map((sound) => sound.id));
    const added = [...files]
      .filter((file) => AUDIO_FILE.test(file.type))
      .filter((file) => {
        const id = `${file.name}-${file.lastModified}`;
        if (existingIds.has(id)) {
          return false;
        }
        existingIds.add(id);
        return true;
      })
      .map((file, index) => fileToSound(file, sounds.length + index));
    if (added.length === 0) {
      setFeedback(
        "No new sounds added. Choose audio files that aren't already on the board."
      );
      return;
    }
    for (const sound of added) {
      objectUrlsRef.current.add(sound.src);
    }
    update([...sounds, ...added]);
    setFeedback(
      `Added ${added.length} ${added.length === 1 ? "sound" : "sounds"}.`
    );
  };

  // Frees a dropped file once no pad uses it and its removal can't be undone.
  const releaseObjectUrl = ({ src }: SoundboardSound) => {
    const inUse = sounds.some((item) => item.src === src);
    if (
      typeof src === "string" &&
      !inUse &&
      objectUrlsRef.current.delete(src)
    ) {
      URL.revokeObjectURL(src);
    }
  };

  const removeSound = (sound: SoundboardSound) => {
    if (removedSound) {
      releaseObjectUrl(removedSound.sound);
    }
    setRemovedSound({
      index: sounds.findIndex((item) => item.id === sound.id),
      sound,
    });
    update(sounds.filter((item) => item.id !== sound.id));
    setFeedback(`Removed ${sound.label}. You can undo this removal.`);
  };

  const undoRemoval = () => {
    if (!removedSound) {
      return;
    }
    const { sound, index } = removedSound;
    if (!sounds.some((item) => item.id === sound.id)) {
      update([...sounds.slice(0, index), sound, ...sounds.slice(index)]);
    }
    setRemovedSound(null);
    setFeedback(`Restored ${sound.label}.`);
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
            id={hotkeysId}
            onCheckedChange={setHotkeys}
            size="sm"
          />
          <Label htmlFor={hotkeysId}>Hotkeys</Label>
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
          onClick={() => {
            for (const pad of padsRef.current.values()) {
              pad.stop();
            }
            setFeedback("Stopped all sounds.");
          }}
          size="sm"
          variant="outline"
        >
          <StopIcon data-icon="inline-start" />
          Stop all
        </Button>
      </div>
      {feedback && (
        <output aria-live="polite" className="mb-3 block">
          <Alert role="none">
            <AlertDescription>{feedback}</AlertDescription>
            {removedSound && (
              <div className="mt-2">
                <Button onClick={undoRemoval} size="sm" variant="outline">
                  Undo removal
                </Button>
              </div>
            )}
          </Alert>
        </output>
      )}
      {sounds.length === 0 ? (
        <div className="rounded-xl border border-dashed">
          <Empty>
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
        </div>
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
              onRemove={() => removeSound(sound)}
              ref={(pad) => {
                if (!pad) {
                  return;
                }
                padsRef.current.set(sound.id, pad);
                return () => {
                  padsRef.current.delete(sound.id);
                };
              }}
              sound={sound}
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
        aria-label="Add audio files"
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
