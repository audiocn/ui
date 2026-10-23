import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdVuMeter,
  LcdVuMeterBar,
  LcdVuMeterChannel,
  LcdVuMeterChannels,
  LcdVuMeterClip,
  LcdVuMeterHold,
  LcdVuMeterScale,
  LcdVuMeterTrack,
  LcdVuMeterValue,
} from "@/components/ui/lcd-vumeter";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

beforeEach(useFakeFrames);
afterEach(() => vi.useRealTimers());

const channels = () =>
  document.querySelectorAll<HTMLElement>("[data-slot='level-meter-channel']");

const levelOf = (index = 0) =>
  Number(channels()[index]?.style.getPropertyValue("--meter-level"));

it("is a meter named by its row's label", () => {
  render(
    <LcdDisplay>
      <LcdDisplayRow>
        <LcdDisplayLabel>Input</LcdDisplayLabel>
        <LcdVuMeter peakDb={-12} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
  const meter = screen.getByRole("meter", { name: "Input" });
  expect(meter).toHaveAttribute("data-slot", "level-meter");
  expect(meter).toHaveAttribute("aria-valuemin", "-60");
});

it("keeps a caller's own name over the row's label", () => {
  render(
    <LcdDisplay>
      <LcdDisplayRow>
        <LcdDisplayLabel>Input</LcdDisplayLabel>
        <LcdVuMeter aria-label="Microphone" peakDb={-12} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
  const meter = screen.getByRole("meter", { name: "Microphone" });
  expect(meter).not.toHaveAttribute("aria-labelledby");
});

it("draws a segment track per channel with the segment count and mode", () => {
  render(
    <LcdVuMeter
      aria-label="Program"
      channels={[{ peakDb: -12 }, { peakDb: -24 }]}
      mode="dot"
      segments={20}
    />
  );
  const meter = screen.getByRole("meter", { name: "Program" });
  expect(meter).toHaveAttribute("data-lcd-mode", "dot");
  expect(meter).toHaveAttribute("data-segments", "20");
  expect(meter.style.getPropertyValue("--lcd-vumeter-segments")).toBe("20");
  expect(meter.style.getPropertyValue("--lcd-vumeter-mask")).toContain(
    "repeating-linear-gradient(to right"
  );
  expect(channels()).toHaveLength(2);
  for (const slot of [
    "lcd-vumeter-track",
    "lcd-vumeter-unlit",
    "lcd-vumeter-bar",
    "lcd-vumeter-hold",
  ]) {
    expect(document.querySelectorAll(`[data-slot='${slot}']`)).toHaveLength(2);
  }
  const bar = document.querySelector("[data-slot='lcd-vumeter-bar']");
  expect(bar).toHaveAttribute("data-measure", "peak");
  // Dot mode lights only the top segment, so the span starts one step below it.
  expect(bar?.className).toContain(
    "--lcd-vumeter-from:max(0,var(--lcd-vumeter-bar)"
  );
});

it("defaults to the Level Meter's 24 segments", () => {
  render(<LcdVuMeter aria-label="Out" peakDb={-6} />);
  expect(screen.getByRole("meter")).toHaveAttribute("data-segments", "24");
});

it("layers an RMS bar over a peak bar, like the Level Meter's dual meter", () => {
  render(
    <LcdVuMeter aria-label="Voice" peakDb={-6} rmsDb={-18}>
      <LcdVuMeterChannels>
        <LcdVuMeterChannel>
          <LcdVuMeterTrack>
            <LcdVuMeterBar className="opacity-40" measure="peak" />
            <LcdVuMeterBar measure="rms" />
            <LcdVuMeterHold />
          </LcdVuMeterTrack>
        </LcdVuMeterChannel>
      </LcdVuMeterChannels>
    </LcdVuMeter>
  );
  const channel = document.querySelector("[data-slot='level-meter-channel']");
  // The RMS level is snapped to whole segments like the peak level.
  expect(channel?.className).toContain(
    "--lcd-vumeter-rms:calc(round(down,var(--meter-rms)"
  );
  const bars = document.querySelectorAll<HTMLElement>(
    "[data-slot='lcd-vumeter-bar']"
  );
  expect([...bars].map((bar) => bar.dataset.measure)).toEqual(["peak", "rms"]);
  expect(bars[0]).toHaveClass("opacity-40");
  expect(bars[1]?.className).toContain(
    "--lcd-vumeter-bar:var(--lcd-vumeter-rms)"
  );
  expect(
    document.querySelectorAll("[data-slot='lcd-vumeter-hold']")
  ).toHaveLength(1);
});

it("lays the segments upward when vertical", () => {
  render(<LcdVuMeter aria-label="Out" orientation="vertical" peakDb={-6} />);
  const meter = screen.getByRole("meter", { name: "Out" });
  expect(meter).toHaveAttribute("data-orientation", "vertical");
  expect(meter.style.getPropertyValue("--lcd-vumeter-mask")).toContain(
    "to top"
  );
});

it("never draws fewer than two segments", () => {
  render(<LcdVuMeter aria-label="Out" segments={0} />);
  expect(screen.getByRole("meter")).toHaveAttribute("data-segments", "2");
});

it("paints declarative levels through the level meter", () => {
  render(<LcdVuMeter aria-label="Out" peakDb={-30} />);
  advance(400);
  expect(levelOf()).toBeCloseTo(0.5, 2);
  expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "-30.0");
});

it("adds a track for each channel a source sends", () => {
  const emitter = createFrameEmitter<MeterFrame>();
  render(<LcdVuMeter aria-label="Program" source={emitter} />);
  expect(channels()).toHaveLength(1);
  act(() => {
    emitter.emit({ channels: [{ peakDb: -12 }, { peakDb: -24 }] });
  });
  advance(400);
  expect(channels()).toHaveLength(2);
  expect(levelOf(1)).toBeCloseTo(0.6, 2);
});

it("composes a scale, a readout and a clip light", () => {
  render(
    <LcdVuMeter aria-label="Out" peakDb={0}>
      <LcdVuMeterChannels>
        <LcdVuMeterChannel />
        <LcdVuMeterScale />
      </LcdVuMeterChannels>
      <LcdVuMeterValue />
      <LcdVuMeterClip />
    </LcdVuMeter>
  );
  advance(400);
  expect(document.querySelector("[data-slot='db-scale']")).not.toBeNull();
  expect(document.querySelector("[data-slot='db-readout']")).toHaveTextContent(
    "0.0 dB"
  );
  const clip = screen.getByRole("button", { name: /Clipping/u });
  // A dot, like the Level Meter's clip light, not a word.
  expect(
    clip.querySelector("[data-slot='lcd-vumeter-clip-light']")
  ).not.toBeNull();
  // The only text is the screen reader announcement.
  expect(clip).toHaveTextContent(/^Clipping$/u);
  expect(clip).toHaveAttribute("data-clipping");
  fireEvent.click(clip);
  expect(
    screen.getByRole("button", { name: "Clip indicator" })
  ).not.toHaveAttribute("data-clipping");
});
