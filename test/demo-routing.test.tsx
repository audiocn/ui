import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import ChannelStripConsole from "@/components/examples/channel-strip-console";
import ChannelStripDemo from "@/components/examples/channel-strip-demo";
import FaderWithMeter from "@/components/examples/fader-with-meter";
import MixerConsole from "@/components/examples/mixer-console";
import MixerDemo from "@/components/examples/mixer-demo";
import FadersTile from "@/components/home/tiles/faders-tile";
import MixerTile from "@/components/home/tiles/mixer-tile";
import { advance, useFakeFrames } from "@/test/fake-frames";

beforeEach(useFakeFrames);
afterEach(() => vi.useRealTimers());

const level = (name: string) =>
  Number(screen.getByRole("meter", { name }).getAttribute("aria-valuenow"));

const expectCleared = (name: string) => {
  const meter = screen.getByRole("meter", { name });
  expect(level(name)).toBe(-60);
  const channels = meter.querySelectorAll<HTMLElement>(
    "[data-slot='level-meter-channel']"
  );
  expect(channels.length).toBeGreaterThan(0);
  for (const channel of channels) {
    for (const property of ["--meter-level", "--meter-rms", "--meter-hold"]) {
      const value = channel.style.getPropertyValue(property);
      expect(value).not.toBe("");
      expect(Number(value)).toBe(0);
    }
  }
};

it.each([
  { Component: ChannelStripDemo, meter: "Microphone level" },
  { Component: ChannelStripConsole, meter: "Mic level" },
  { Component: MixerConsole, meter: "In 1 level" },
  { Component: MixerDemo, meter: "Microphone level" },
  { Component: MixerTile, meter: "Microphone level" },
])("$Component.name clears mute on the next frame", ({ Component, meter }) => {
  const { unmount } = render(<Component />);
  advance(800);
  expect(level(meter)).toBeGreaterThan(-60);
  const [mute] = screen.getAllByRole("button", { name: /^Mute/u });
  fireEvent.click(mute);
  advance(16);
  expectCleared(meter);
  fireEvent.click(mute);
  advance(800);
  expect(level(meter)).toBeGreaterThan(-60);
  fireEvent.keyDown(screen.getAllByRole("slider")[0], { key: "Home" });
  advance(16);
  // Ordinary fader changes still release smoothly after unmuting.
  expect(level(meter)).toBeGreaterThan(-60);
  unmount();
});

it.each([{ Component: MixerDemo }, { Component: MixerTile }])(
  "$Component.name clears the master only when every input is muted",
  ({ Component }) => {
    const { unmount } = render(<Component />);
    advance(800);
    const [first, ...others] = screen.getAllByRole("button", {
      name: /^Mute/u,
    });
    fireEvent.click(first);
    advance(16);
    expect(level("Master level")).toBeGreaterThan(-60);
    for (const mute of others) {
      fireEvent.click(mute);
    }
    advance(16);
    expectCleared("Master level");
    fireEvent.click(first);
    advance(800);
    expect(level("Master level")).toBeGreaterThan(-60);
    unmount();
  }
);

it.each([
  { Component: FaderWithMeter, fader: "Program gain", meter: "Program level" },
  { Component: FadersTile, fader: "Drums volume", meter: "Drums level" },
  {
    Component: ChannelStripDemo,
    fader: "Microphone volume",
    meter: "Microphone level",
  },
  { Component: ChannelStripConsole, fader: "Mic volume", meter: "Mic level" },
  { Component: MixerConsole, fader: "In 1 volume", meter: "In 1 level" },
  {
    Component: MixerDemo,
    fader: "Microphone volume",
    meter: "Microphone level",
  },
  {
    Component: MixerTile,
    fader: "Microphone volume",
    meter: "Microphone level",
  },
])(
  "$Component.name routes $fader into $meter",
  ({ Component, fader, meter }) => {
    const { unmount } = render(<Component />);
    advance(800);
    expect(level(meter)).toBeGreaterThan(-60);
    fireEvent.keyDown(screen.getByRole("slider", { name: fader }), {
      key: "Home",
    });
    advance(4000);
    expect(level(meter)).toBeLessThanOrEqual(-60);
    fireEvent.keyDown(screen.getByRole("slider", { name: fader }), {
      key: "End",
    });
    advance(800);
    expect(level(meter)).toBeGreaterThan(-60);
    unmount();
  }
);

it.each([{ Component: MixerDemo }, { Component: MixerTile }])(
  "$Component.name master follows channel faders and its own fader",
  ({ Component }) => {
    const { unmount } = render(<Component />);
    advance(800);
    expect(level("Master level")).toBeGreaterThan(-60);
    for (const slider of screen.getAllByRole("slider")) {
      if (slider.getAttribute("aria-label") !== "Master volume") {
        fireEvent.keyDown(slider, { key: "Home" });
      }
    }
    advance(4000);
    expect(level("Master level")).toBeLessThan(-48);
    fireEvent.keyDown(
      screen.getByRole("slider", { name: "Microphone volume" }),
      { key: "End" }
    );
    advance(800);
    expect(level("Master level")).toBeGreaterThan(-60);
    fireEvent.keyDown(screen.getByRole("slider", { name: "Master volume" }), {
      key: "Home",
    });
    advance(4000);
    expect(level("Master level")).toBeLessThan(-48);
    expect(level("Microphone level")).toBeGreaterThan(-60);
    unmount();
  }
);

it("console solo isolates channels, respects mute and restores the mix", () => {
  const { unmount } = render(<ChannelStripConsole />);
  advance(800);
  fireEvent.click(screen.getByRole("button", { name: "Solo Mic" }));
  advance(16);
  expect(level("Mic level")).toBeGreaterThan(-60);
  expectCleared("Music level");
  expectCleared("Game level");
  fireEvent.click(screen.getByRole("button", { name: "Solo Music" }));
  advance(800);
  expect(level("Mic level")).toBeGreaterThan(-60);
  expect(level("Music level")).toBeGreaterThan(-60);
  expect(level("Game level")).toBe(-60);
  fireEvent.click(screen.getByRole("button", { name: "Mute Mic" }));
  advance(16);
  expectCleared("Mic level");
  expect(level("Music level")).toBeGreaterThan(-60);
  fireEvent.click(screen.getByRole("button", { name: "Solo Mic" }));
  fireEvent.click(screen.getByRole("button", { name: "Solo Music" }));
  advance(800);
  expect(level("Mic level")).toBe(-60);
  expect(level("Music level")).toBeGreaterThan(-60);
  expect(level("Game level")).toBeGreaterThan(-60);
  unmount();
});
