import { fireEvent, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { AudioPlayer } from "@/components/ui/audio-player";
import { Knob } from "@/components/ui/knob";
import {
  ParameterSlider,
  ParameterSliderControl,
} from "@/components/ui/parameter-slider";
import {
  SoundPad,
  SoundPadGrid,
  SoundPadProgress,
} from "@/components/ui/sound-pad";
import type { AudioPlayerController } from "@/hooks/use-audio-player";

const controller = (): AudioPlayerController => ({
  buffered: 0,
  currentTime: 12,
  duration: 220,
  element: null,
  error: null,
  loop: false,
  muted: false,
  pause: vi.fn(),
  play: vi.fn(() => Promise.resolve()),
  playbackRate: 1,
  playing: false,
  seek: vi.fn(),
  setLoop: vi.fn(),
  setMuted: vi.fn(),
  setPlaybackRate: vi.fn(),
  setVolume: vi.fn(),
  status: "paused",
  time: {
    subscribe: () => () => {
      // Nothing to release.
    },
  },
  toggle: vi.fn(() => Promise.resolve()),
  volume: 1,
});

const holdPad = (props: { loading?: boolean; onStop: () => void }) => (
  <SoundPadGrid hotkeyScope="global" hotkeys>
    <SoundPad
      hotkey="a"
      loading={props.loading}
      mode="hold"
      onStop={props.onStop}
      onTrigger={vi.fn()}
    >
      A
    </SoundPad>
  </SoundPadGrid>
);

describe("controls", () => {
  it("knob: the wheel doesn't change a disabled dial", () => {
    render(<Knob aria-label="Gain" defaultValue={50} disabled />);
    const dial = screen.getByRole("slider");
    dial.focus();
    fireEvent.wheel(dial, { deltaY: -100 });
    expect(dial).toHaveAttribute("aria-valuenow", "50");
  });

  it("knob: the wheel still turns an enabled dial", () => {
    render(<Knob aria-label="Gain" defaultValue={50} />);
    const dial = screen.getByRole("slider");
    dial.focus();
    fireEvent.wheel(dial, { deltaY: -100 });
    expect(dial).not.toHaveAttribute("aria-valuenow", "50");
  });

  it("parameter slider: steps from the parent's value after it rejected a change", () => {
    const onValueChange = vi.fn();
    const slider = (className: string) => (
      <ParameterSlider
        className={className}
        max={10}
        min={0}
        onValueChange={onValueChange}
        step={1}
        value={0}
      >
        <ParameterSliderControl />
      </ParameterSlider>
    );
    const { rerender } = render(slider("a"));
    fireEvent.keyDown(screen.getByRole("slider"), { key: "ArrowUp" });
    // An unrelated re-render; the parent still says 0.
    rerender(slider("b"));
    fireEvent.keyDown(screen.getByRole("slider"), { key: "ArrowUp" });
    const reported = onValueChange.mock.calls.map(([value]) => value);
    expect(reported).toContain(1);
    expect(reported).not.toContain(2);
  });

  it("sound pad: a held hotkey is released when its pad goes away", () => {
    const onStop = vi.fn();
    const { rerender } = render(holdPad({ onStop }));
    fireEvent.keyDown(document, { key: "a" });
    expect(onStop).not.toHaveBeenCalled();
    rerender(holdPad({ loading: true, onStop }));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("sound pad: a held hotkey is released when the window loses focus", () => {
    const onStop = vi.fn();
    render(holdPad({ onStop }));
    fireEvent.keyDown(document, { key: "a" });
    fireEvent.blur(window);
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("sound pad progress: renders a declarative value on the server", () => {
    const html = renderToString(<SoundPadProgress value={0.5} />);
    expect(html).toContain("--pad-progress:0.5000");
  });

  it("audio player: reports time changes, not parent re-renders", () => {
    const player = controller();
    const reported: number[] = [];
    const view = (label: string) => (
      <AudioPlayer
        aria-label={label}
        onTimeUpdate={(time) => reported.push(time)}
        player={player}
      />
    );
    const { rerender } = render(view("Player"));
    const calls = reported.length;
    rerender(view("Player, again"));
    expect(reported).toHaveLength(calls);
  });
});
