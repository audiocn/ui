import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  AudioPlayer,
  AudioPlayerPlay,
  AudioPlayerTime,
} from "@/components/ui/audio-player";
import {
  ChannelStrip,
  ChannelStripTitle,
  useChannelStrip,
} from "@/components/ui/channel-strip";
import { Fader } from "@/components/ui/fader";
import { LevelMeter } from "@/components/ui/level-meter";
import { Mixer, MixerChannels, MixerEmpty } from "@/components/ui/mixer";
import { SoundPad, SoundPadGrid } from "@/components/ui/sound-pad";
import { TrackList, TrackListItem } from "@/components/ui/track-list";
import { Waveform } from "@/components/ui/waveform";
import type { AudioPlayerController } from "@/hooks/use-audio-player";

const SoloProbe = () => {
  const { solo } = useChannelStrip();
  return <span>{solo ? "soloed" : "not soloed"}</span>;
};

const controller = (
  overrides: Partial<AudioPlayerController> = {}
): AudioPlayerController => ({
  buffered: 0,
  currentTime: 84,
  duration: 220,
  element: null,
  error: null,
  loop: false,
  muted: false,
  pause: vi.fn(),
  play: vi.fn(async () => {}),
  playbackRate: 1,
  playing: false,
  seek: vi.fn(),
  setLoop: vi.fn(),
  setMuted: vi.fn(),
  setPlaybackRate: vi.fn(),
  setVolume: vi.fn(),
  status: "paused",
  time: { subscribe: () => () => {} },
  toggle: vi.fn(async () => {}),
  volume: 1,
  ...overrides,
});

describe("ChannelStrip", () => {
  it("is a named group that passes its orientation down", () => {
    render(
      <ChannelStrip muted orientation="vertical">
        <ChannelStripTitle>Mic</ChannelStripTitle>
        <LevelMeter aria-label="Mic level" />
      </ChannelStrip>
    );
    const strip = screen.getByRole("group", { name: "Mic" });
    expect(strip).toHaveAttribute("data-muted");
    const meter = screen.getByRole("meter");
    expect(meter).toHaveAttribute("data-orientation", "vertical");
    expect(meter).toHaveAttribute("data-dimmed");
  });

  it("exposes its state to custom parts", () => {
    render(
      <ChannelStrip solo>
        <SoloProbe />
      </ChannelStrip>
    );
    expect(screen.getByText("soloed")).toBeInTheDocument();
  });
});

describe("Mixer", () => {
  it("shows the empty state without channels", () => {
    render(
      <Mixer aria-label="Mixer">
        <MixerChannels />
        <MixerEmpty>Nothing here</MixerEmpty>
      </Mixer>
    );
    expect(screen.getByText("Nothing here")).toBeInTheDocument();
  });

  it("moves focus to the same control on the next strip with Ctrl+arrow", () => {
    render(
      <Mixer orientation="vertical">
        <MixerChannels>
          <ChannelStrip>
            <ChannelStripTitle>A</ChannelStripTitle>
            <Fader aria-label="A volume" />
          </ChannelStrip>
          <ChannelStrip>
            <ChannelStripTitle>B</ChannelStripTitle>
            <Fader aria-label="B volume" />
          </ChannelStrip>
        </MixerChannels>
      </Mixer>
    );
    const [first, second] = screen.getAllByRole("slider");
    first?.focus();
    fireEvent.keyDown(first as HTMLElement, {
      ctrlKey: true,
      key: "ArrowRight",
    });
    expect(document.activeElement).toBe(second);
  });
});

describe("Waveform", () => {
  it("seeks with the keyboard", () => {
    const onSeekCommitted = vi.fn();
    render(
      <Waveform
        aria-label="Clip"
        defaultCurrentTime={10}
        duration={60}
        onSeekCommitted={onSeekCommitted}
        peaks={[0.2, 0.5, 1]}
      />
    );
    const slider = screen.getByRole("slider", { name: "Clip" });
    expect(slider).toHaveAttribute("aria-valuetext", "0:10 of 1:00");
    fireEvent.keyDown(slider, { key: "ArrowRight", shiftKey: true });
    expect(onSeekCommitted).toHaveBeenCalledWith(25);
    fireEvent.keyDown(slider, { key: "End" });
    expect(onSeekCommitted).toHaveBeenLastCalledWith(60);
  });

  it("is not a slider when display only", () => {
    render(<Waveform duration={10} interactive={false} peaks={[1]} />);
    expect(screen.queryByRole("slider")).toBeNull();
  });
});

describe("TrackList", () => {
  it("selects with Enter and moves with the arrow keys", () => {
    const onSelect = vi.fn();
    render(
      <TrackList>
        <TrackListItem active onSelect={onSelect}>
          One
        </TrackListItem>
        <TrackListItem>Two</TrackListItem>
      </TrackList>
    );
    const [first, second] = screen.getAllByRole("listitem");
    expect(first).toHaveAttribute("aria-current", "true");
    first?.focus();
    fireEvent.keyDown(first as HTMLElement, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(first as HTMLElement, { key: "ArrowDown" });
    expect(document.activeElement).toBe(second);
  });
});

describe("SoundPad", () => {
  it("toggles in toggle mode", () => {
    const onTrigger = vi.fn();
    const onStop = vi.fn();
    const { rerender } = render(
      <SoundPad mode="toggle" onStop={onStop} onTrigger={onTrigger}>
        Pad
      </SoundPad>
    );
    const pad = screen.getByRole("button", { name: "Pad" });
    fireEvent.pointerDown(pad, { button: 0 });
    expect(onTrigger).toHaveBeenCalledTimes(1);
    rerender(
      <SoundPad mode="toggle" onStop={onStop} onTrigger={onTrigger} playing>
        Pad
      </SoundPad>
    );
    expect(pad).toHaveAttribute("aria-pressed", "true");
    fireEvent.pointerDown(pad, { button: 0 });
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("stops a hold pad on release", () => {
    const onStop = vi.fn();
    render(
      <SoundPad mode="hold" onStop={onStop} onTrigger={vi.fn()}>
        Hold
      </SoundPad>
    );
    const pad = screen.getByRole("button", { name: "Hold" });
    fireEvent.pointerDown(pad, { button: 0 });
    fireEvent.pointerUp(pad);
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("triggers from a grid hotkey but not while typing", () => {
    const onTrigger = vi.fn();
    render(
      <>
        <input aria-label="Notes" />
        <SoundPadGrid hotkeyScope="global" hotkeys>
          <SoundPad hotkey="q" onTrigger={onTrigger}>
            Q
          </SoundPad>
        </SoundPadGrid>
      </>
    );
    fireEvent.keyDown(document.body, { key: "q" });
    fireEvent.keyUp(document.body, { key: "q" });
    expect(onTrigger).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "q" });
    expect(onTrigger).toHaveBeenCalledTimes(1);
  });
});

describe("AudioPlayer", () => {
  it("renders parts bound to an external player", () => {
    const player = controller();
    render(
      <AudioPlayer player={player}>
        <AudioPlayerPlay />
        <AudioPlayerTime />
        <AudioPlayerTime type="remaining" />
      </AudioPlayer>
    );
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(player.toggle).toHaveBeenCalled();
    expect(screen.getByText("1:24")).toBeInTheDocument();
    expect(screen.getByText("−2:16")).toBeInTheDocument();
  });

  it("handles keyboard shortcuts", () => {
    const player = controller();
    render(
      <AudioPlayer player={player}>
        <span tabIndex={-1}>focus target</span>
      </AudioPlayer>
    );
    const target = screen.getByText("focus target");
    fireEvent.keyDown(target, { key: "ArrowRight" });
    expect(player.seek).toHaveBeenCalledWith(89);
    fireEvent.keyDown(target, { key: "m" });
    expect(player.setMuted).toHaveBeenCalledWith(true);
    fireEvent.keyDown(target, { key: "k" });
    expect(player.toggle).toHaveBeenCalled();
  });
});
