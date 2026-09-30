"use client";

import { mergeProps } from "@base-ui/react/merge-props";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { useRender } from "@base-ui/react/use-render";
import { createContext, useContext, useEffect, useMemo, useState } from "react";
import type {
  ComponentProps,
  CSSProperties,
  KeyboardEvent,
  ReactNode,
} from "react";

import {
  VolumeControl,
  VolumeControlMute,
  VolumeControlSlider,
} from "@/components/ui/volume-control";
import type { VolumeControlProps } from "@/components/ui/volume-control";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import type {
  AudioPlayerController,
  UseAudioPlayerOptions,
} from "@/hooks/use-audio-player";
import { useFrameSource } from "@/hooks/use-frame-source";
import { clamp } from "@/lib/audio/decibels";
import { formatTime } from "@/lib/audio/time";
import { cn } from "@/lib/utils";

const DEFAULT_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];
const SEEK_STEP = 5;
const SEEK_LARGE_STEP = 15;
const VOLUME_STEP = 0.05;

interface AudioPlayerContextValue {
  player: AudioPlayerController;
  onPrevious?: () => void;
  onNext?: () => void;
}

const AudioPlayerContext = createContext<AudioPlayerContextValue | null>(null);

const useAudioPlayerPart = (part: string) => {
  const context = useContext(AudioPlayerContext);
  if (!context) {
    throw new Error(`${part} must be used inside AudioPlayer.`);
  }
  return context;
};

/** The player controller of the surrounding `AudioPlayer`. */
export const useAudioPlayerContext = (): AudioPlayerController =>
  useAudioPlayerPart("useAudioPlayerContext").player;

export interface AudioPlayerProps
  extends
    Omit<
      ComponentProps<"div">,
      "onPlay" | "onPause" | "onEnded" | "onError" | "onTimeUpdate"
    >,
    Omit<UseAudioPlayerOptions, "onPlay" | "onPause" | "onEnded" | "onError"> {
  /** Use an existing player from `useAudioPlayer` instead of an internal one. */
  player?: AudioPlayerController;
  onPlay?: () => void;
  onPause?: () => void;
  onEnded?: () => void;
  onError?: (error: MediaError | null) => void;
  onTimeUpdate?: (time: number) => void;
  /** Enables the previous button. */
  onPrevious?: () => void;
  /** Enables the next button. */
  onNext?: () => void;
  /** Keyboard shortcuts while focus is inside. Default true. */
  shortcuts?: boolean;
}

const InternalPlayer = ({
  options,
  children,
}: {
  options: UseAudioPlayerOptions;
  children: (player: AudioPlayerController) => ReactNode;
}) => {
  const player = useAudioPlayer(options);
  return <>{children(player)}</>;
};

const isEditable = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable ||
    ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName));

type ShortcutEvent = Pick<
  KeyboardEvent<HTMLElement>,
  "key" | "shiftKey" | "target"
>;

const inSliderTarget = (target: HTMLElement) =>
  target.getAttribute("type") === "range" ||
  target.getAttribute("role") === "slider";

/** The player action for a key, or null to let the key through. */
const shortcutFor = (
  event: ShortcutEvent,
  player: AudioPlayerController
): (() => void) | null => {
  const target = event.target as HTMLElement;
  const seekAmount = event.shiftKey ? SEEK_LARGE_STEP : SEEK_STEP;
  const togglesPlayback = !(target.tagName === "BUTTON" || isEditable(target));
  const sliderKeys = !inSliderTarget(target);
  const actions: Record<string, (() => void) | null> = {
    " ": togglesPlayback ? () => player.toggle() : null,
    ArrowDown: sliderKeys
      ? () => player.setVolume(clamp(player.volume - VOLUME_STEP, 0, 1))
      : null,
    ArrowLeft: sliderKeys
      ? () => player.seek(player.currentTime - seekAmount)
      : null,
    ArrowRight: sliderKeys
      ? () => player.seek(player.currentTime + seekAmount)
      : null,
    ArrowUp: sliderKeys
      ? () => player.setVolume(clamp(player.volume + VOLUME_STEP, 0, 1))
      : null,
    End: () => player.seek(player.duration),
    Home: () => player.seek(0),
    k: togglesPlayback ? () => player.toggle() : null,
    m: () => player.setMuted(!player.muted),
  };
  return actions[event.key] ?? null;
};

const PlayerRoot = ({
  player,
  onPrevious,
  onNext,
  onTimeUpdate,
  shortcuts,
  className,
  children,
  onKeyDown,
  ...props
}: Omit<AudioPlayerProps, keyof UseAudioPlayerOptions | "player"> & {
  player: AudioPlayerController;
}) => {
  const { currentTime } = player;
  const contextValue = useMemo<AudioPlayerContextValue>(
    () => ({ onNext, onPrevious, player }),
    [onNext, onPrevious, player]
  );

  useEffect(() => {
    onTimeUpdate?.(currentTime);
  }, [currentTime, onTimeUpdate]);

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(event);
    if (
      !shortcuts ||
      event.defaultPrevented ||
      event.metaKey ||
      event.ctrlKey
    ) {
      return;
    }
    const action = shortcutFor(event, player);
    if (action) {
      event.preventDefault();
      action();
    }
  };

  return (
    <AudioPlayerContext.Provider value={contextValue}>
      <div
        className={cn(
          "group/audio-player flex flex-wrap items-center gap-x-3 gap-y-2 outline-none",
          className
        )}
        data-ended={player.status === "ended" ? "" : undefined}
        data-error={player.status === "error" ? "" : undefined}
        data-loading={player.status === "loading" ? "" : undefined}
        data-muted={player.muted ? "" : undefined}
        data-paused={player.playing ? undefined : ""}
        data-playing={player.playing ? "" : undefined}
        data-slot="audio-player"
        onKeyDown={handleKeyDown}
        role="group"
        {...props}
      >
        {children}
      </div>
    </AudioPlayerContext.Provider>
  );
};

export const AudioPlayer = ({
  player,
  src,
  autoPlay,
  loop,
  volume,
  muted,
  playbackRate,
  preload,
  crossOrigin,
  onPlay,
  onPause,
  onEnded,
  onError,
  shortcuts = true,
  ...props
}: AudioPlayerProps) => {
  if (player) {
    return <PlayerRoot player={player} shortcuts={shortcuts} {...props} />;
  }
  return (
    <InternalPlayer
      options={{
        autoPlay,
        crossOrigin,
        loop,
        muted,
        onEnded,
        onError,
        onPause,
        onPlay,
        playbackRate,
        preload,
        src,
        volume,
      }}
    >
      {(internal) => (
        <PlayerRoot player={internal} shortcuts={shortcuts} {...props} />
      )}
    </InternalPlayer>
  );
};

export const AudioPlayerArtwork = ({
  className,
  alt = "",
  ...props
}: ComponentProps<"img">) => (
  <img
    alt={alt}
    className={cn(
      "bg-muted size-12 shrink-0 rounded-lg object-cover",
      className
    )}
    data-slot="audio-player-artwork"
    {...props}
  />
);

export const AudioPlayerTitle = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn("truncate text-sm font-medium", className)}
    data-slot="audio-player-title"
    {...props}
  />
);

export const AudioPlayerDescription = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn("text-muted-foreground truncate text-xs", className)}
    data-slot="audio-player-description"
    {...props}
  />
);

export const AudioPlayerControls = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn("flex items-center gap-1", className)}
    data-slot="audio-player-controls"
    {...props}
  />
);

const buttonClass =
  "inline-flex size-8 shrink-0 items-center justify-center rounded-full text-foreground outline-none transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 disabled:pointer-events-none disabled:opacity-40 [&_svg:not([class*='size-'])]:size-4";

type PlayerButtonProps = Omit<
  useRender.ComponentProps<"button">,
  "children"
> & {
  children?: ReactNode;
};

const usePlayerButton = (
  slot: string,
  label: string,
  onClick: () => void,
  { render, className, children, ...props }: PlayerButtonProps,
  options: { disabled?: boolean; state?: Record<string, boolean> } = {}
) =>
  useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      {
        "aria-label": label,
        children: children ?? <span className="text-xs">{label}</span>,
        className: cn(buttonClass, !children && "w-auto px-2.5", className),
        disabled: options.disabled,
        onClick,
        type: "button",
      },
      props
    ),
    render,
    state: { slot, ...options.state },
  });

export interface AudioPlayerPlayProps extends Omit<
  useRender.ComponentProps<"button">,
  "children"
> {
  children?:
    | ReactNode
    | ((state: { playing: boolean; loading: boolean }) => ReactNode);
}

export const AudioPlayerPlay = ({
  children,
  className,
  ...props
}: AudioPlayerPlayProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerPlay");
  const loading = player.status === "loading";
  const content =
    typeof children === "function"
      ? children({ loading, playing: player.playing })
      : children;
  return usePlayerButton(
    "audio-player-play",
    player.playing ? "Pause" : "Play",
    () => {
      player.toggle();
    },
    {
      children: content,
      className: cn(
        "bg-primary text-primary-foreground hover:bg-primary/85 size-10",
        className
      ),
      ...props,
    },
    {
      disabled: player.status === "idle" || player.status === "error",
      state: { loading, playing: player.playing },
    }
  );
};

export const AudioPlayerPrevious = (props: PlayerButtonProps) => {
  const { onPrevious } = useAudioPlayerPart("AudioPlayerPrevious");
  return usePlayerButton(
    "audio-player-previous",
    "Previous",
    () => onPrevious?.(),
    props,
    {
      disabled: !onPrevious,
    }
  );
};

export const AudioPlayerNext = (props: PlayerButtonProps) => {
  const { onNext } = useAudioPlayerPart("AudioPlayerNext");
  return usePlayerButton("audio-player-next", "Next", () => onNext?.(), props, {
    disabled: !onNext,
  });
};

export interface AudioPlayerSkipProps extends PlayerButtonProps {
  /** Default 10. */
  seconds?: number;
}

export const AudioPlayerSkipBack = ({
  seconds = 10,
  ...props
}: AudioPlayerSkipProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerSkipBack");
  return usePlayerButton(
    "audio-player-skip-back",
    `Back ${seconds} seconds`,
    () => player.seek(player.currentTime - seconds),
    props
  );
};

export const AudioPlayerSkipForward = ({
  seconds = 10,
  ...props
}: AudioPlayerSkipProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerSkipForward");
  return usePlayerButton(
    "audio-player-skip-forward",
    `Forward ${seconds} seconds`,
    () => player.seek(player.currentTime + seconds),
    props
  );
};

export interface AudioPlayerSeekProps extends Omit<
  SliderPrimitive.Root.Props<number>,
  | "value"
  | "defaultValue"
  | "onValueChange"
  | "onValueCommitted"
  | "min"
  | "max"
  | "step"
> {
  /** Seconds per arrow key. Default 5. */
  step?: number;
  /** Seconds per Shift+arrow. Default 15. */
  largeStep?: number;
}

export const AudioPlayerSeek = ({
  step = SEEK_STEP,
  largeStep = SEEK_LARGE_STEP,
  className,
  ...props
}: AudioPlayerSeekProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerSeek");
  const [time, setTime] = useState(player.currentTime);
  const [dragValue, setDragValue] = useState<number | null>(null);
  const duration = player.duration || 0;

  useFrameSource(player.time, (next) => {
    if (dragValue === null) {
      setTime(next);
    }
  });

  const value = dragValue ?? time;
  const bufferedPercent =
    duration > 0 ? clamp(player.buffered / duration, 0, 1) * 100 : 0;

  return (
    <SliderPrimitive.Root
      className={cn(
        "relative flex min-w-24 flex-1 touch-none items-center select-none",
        className
      )}
      data-slot="audio-player-seek"
      disabled={duration === 0}
      largeStep={largeStep}
      max={Math.max(duration, 0.001)}
      min={0}
      onValueChange={(next) => setDragValue(next)}
      onValueCommitted={(next) => {
        player.seek(next);
        setTime(next);
        setDragValue(null);
      }}
      step={0.01}
      value={clamp(value, 0, Math.max(duration, 0.001))}
      {...props}
    >
      <SliderPrimitive.Control className="relative flex h-4 w-full items-center px-1.5 before:absolute before:inset-x-0 before:-inset-y-1.5 pointer-coarse:before:-inset-y-3">
        <SliderPrimitive.Track
          className="bg-input/90 relative h-1 w-full grow rounded-full"
          data-slot="audio-player-seek-track"
        >
          <div
            className="bg-muted-foreground/25 absolute inset-y-0 left-0 w-(--buffered) rounded-full"
            data-slot="audio-player-seek-buffered"
            style={{ "--buffered": `${bufferedPercent}%` } as CSSProperties}
          />
          <SliderPrimitive.Indicator
            className="bg-primary rounded-full"
            data-slot="audio-player-seek-range"
          />
          <SliderPrimitive.Thumb
            aria-label="Seek"
            className="bg-background ring-foreground/15 hover:ring-ring/30 focus-visible:ring-ring/40 block size-3 shrink-0 rounded-full shadow-sm ring-1 outline-hidden transition-[box-shadow] hover:ring-4 focus-visible:ring-4"
            data-slot="audio-player-seek-thumb"
            getAriaValueText={() =>
              `${formatTime(value)} of ${formatTime(duration)}`
            }
            onKeyDown={(event) => {
              const amount = event.shiftKey ? largeStep : step;
              if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                event.preventDefault();
                player.seek(
                  value + (event.key === "ArrowRight" ? amount : -amount)
                );
              }
            }}
          />
        </SliderPrimitive.Track>
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
};

export interface AudioPlayerTimeProps extends ComponentProps<"span"> {
  /** Default `current`. */
  type?: "current" | "remaining" | "duration";
  format?: (
    seconds: number,
    type: "current" | "remaining" | "duration"
  ) => string;
}

const defaultTimeFormat = (
  seconds: number,
  type: "current" | "remaining" | "duration"
) => formatTime(seconds, { remaining: type === "remaining" });

export const AudioPlayerTime = ({
  type = "current",
  format = defaultTimeFormat,
  className,
  ...props
}: AudioPlayerTimeProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerTime");
  let seconds = player.currentTime;
  if (type === "duration") {
    seconds = player.duration;
  } else if (type === "remaining") {
    seconds = Math.max(0, player.duration - player.currentTime);
  }
  return (
    <span
      className={cn(
        "text-muted-foreground font-mono text-xs tabular-nums",
        className
      )}
      data-slot="audio-player-time"
      data-type={type}
      {...props}
    >
      {format(seconds, type)}
    </span>
  );
};

export type AudioPlayerVolumeProps = Omit<
  VolumeControlProps,
  | "value"
  | "defaultValue"
  | "onValueChange"
  | "muted"
  | "defaultMuted"
  | "onMutedChange"
>;

export const AudioPlayerVolume = ({
  children,
  className,
  ...props
}: AudioPlayerVolumeProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerVolume");
  return (
    <VolumeControl
      className={cn("w-32", className)}
      data-slot="audio-player-volume"
      muted={player.muted}
      onMutedChange={(value) => player.setMuted(value)}
      onValueChange={(value) => player.setVolume(value)}
      value={player.volume}
      {...props}
    >
      {children ?? (
        <>
          <VolumeControlMute />
          <VolumeControlSlider />
        </>
      )}
    </VolumeControl>
  );
};

export interface AudioPlayerRateProps extends PlayerButtonProps {
  rates?: number[];
}

export const AudioPlayerRate = ({
  rates = DEFAULT_RATES,
  children,
  ...props
}: AudioPlayerRateProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerRate");
  const index = rates.indexOf(player.playbackRate);
  const next = rates[(index + 1) % rates.length] ?? 1;
  return usePlayerButton(
    "audio-player-rate",
    `Playback speed ${player.playbackRate}×`,
    () => player.setPlaybackRate(next),
    {
      children: children ?? (
        <span className="font-mono text-xs tabular-nums">
          {player.playbackRate}×
        </span>
      ),
      ...props,
      className: cn("w-auto px-2", props.className),
    }
  );
};

export const AudioPlayerLoop = (props: PlayerButtonProps) => {
  const { player } = useAudioPlayerPart("AudioPlayerLoop");
  return usePlayerButton(
    "audio-player-loop",
    player.loop ? "Loop on" : "Loop off",
    () => player.setLoop(!player.loop),
    {
      "aria-pressed": player.loop,
      ...props,
      className: cn(
        "aria-pressed:bg-muted aria-pressed:text-primary",
        props.className
      ),
    },
    { state: { looping: player.loop } }
  );
};
