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
  advance(4000);
  expect(level("Mic level")).toBeGreaterThan(-60);
  expect(level("Music level")).toBe(-60);
  expect(level("Game level")).toBe(-60);
  fireEvent.click(screen.getByRole("button", { name: "Solo Music" }));
  advance(800);
  expect(level("Mic level")).toBeGreaterThan(-60);
  expect(level("Music level")).toBeGreaterThan(-60);
  expect(level("Game level")).toBe(-60);
  fireEvent.click(screen.getByRole("button", { name: "Mute Mic" }));
  advance(4000);
  expect(level("Mic level")).toBe(-60);
  expect(level("Music level")).toBeGreaterThan(-60);
  fireEvent.click(screen.getByRole("button", { name: "Solo Mic" }));
  fireEvent.click(screen.getByRole("button", { name: "Solo Music" }));
  advance(800);
  expect(level("Mic level")).toBe(-60);
  expect(level("Music level")).toBeGreaterThan(-60);
  expect(level("Game level")).toBeGreaterThan(-60);
  unmount();
});
