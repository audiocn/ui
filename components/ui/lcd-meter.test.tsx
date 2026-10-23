import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterChannel,
  LcdMeterChannels,
  LcdMeterClip,
  LcdMeterScale,
  LcdMeterValue,
} from "@/components/ui/lcd-meter";
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
        <LcdMeter peakDb={-12} />
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
        <LcdMeter aria-label="Microphone" peakDb={-12} />
      </LcdDisplayRow>
    </LcdDisplay>
  );
  const meter = screen.getByRole("meter", { name: "Microphone" });
  expect(meter).not.toHaveAttribute("aria-labelledby");
});

it("draws a segment track per channel with the segment count and mode", () => {
  render(
    <LcdMeter
      aria-label="Program"
      channels={[{ peakDb: -12 }, { peakDb: -24 }]}
      mode="dot"
      segments={20}
    />
  );
  const meter = screen.getByRole("meter", { name: "Program" });
  expect(meter).toHaveAttribute("data-lcd-mode", "dot");
  expect(meter).toHaveAttribute("data-segments", "20");
  expect(meter.style.getPropertyValue("--lcd-meter-segments")).toBe("20");
  expect(meter.style.getPropertyValue("--lcd-meter-mask")).toContain(
    "repeating-linear-gradient(to right"
  );
  expect(channels()).toHaveLength(2);
  for (const slot of [
    "lcd-meter-track",
    "lcd-meter-unlit",
    "lcd-meter-lit",
    "lcd-meter-hold",
  ]) {
    expect(document.querySelectorAll(`[data-slot='${slot}']`)).toHaveLength(2);
  }
  const lit = document.querySelector("[data-slot='lcd-meter-lit']");
  // Dot mode lights only the top segment, so the span starts one step below it.
  expect(lit?.className).toContain(
    "--lcd-meter-from:max(0,var(--lcd-meter-lit)"
  );
});

it("lays the segments upward when vertical", () => {
  render(<LcdMeter aria-label="Out" orientation="vertical" peakDb={-6} />);
  const meter = screen.getByRole("meter", { name: "Out" });
  expect(meter).toHaveAttribute("data-orientation", "vertical");
  expect(meter.style.getPropertyValue("--lcd-meter-mask")).toContain("to top");
});

it("never draws fewer than two segments", () => {
  render(<LcdMeter aria-label="Out" segments={0} />);
  expect(screen.getByRole("meter")).toHaveAttribute("data-segments", "2");
});

it("paints declarative levels through the level meter", () => {
  render(<LcdMeter aria-label="Out" peakDb={-30} />);
  advance(400);
  expect(levelOf()).toBeCloseTo(0.5, 2);
  expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "-30.0");
});

it("adds a track for each channel a source sends", () => {
  const emitter = createFrameEmitter<MeterFrame>();
  render(<LcdMeter aria-label="Program" source={emitter} />);
  expect(channels()).toHaveLength(1);
  act(() => {
    emitter.emit({ channels: [{ peakDb: -12 }, { peakDb: -24 }] });
  });
  advance(400);
  expect(channels()).toHaveLength(2);
  expect(levelOf(1)).toBeCloseTo(0.6, 2);
});

it("composes a scale, a readout and a clip legend", () => {
  render(
    <LcdMeter aria-label="Out" peakDb={0}>
      <LcdMeterChannels>
        <LcdMeterChannel />
        <LcdMeterScale />
      </LcdMeterChannels>
      <LcdMeterValue />
      <LcdMeterClip />
    </LcdMeter>
  );
  advance(400);
  expect(document.querySelector("[data-slot='db-scale']")).not.toBeNull();
  expect(document.querySelector("[data-slot='db-readout']")).toHaveTextContent(
    "0.0 dB"
  );
  const clip = screen.getByRole("button", { name: /Clipping/u });
  expect(clip).toHaveTextContent("Clip");
  expect(clip).toHaveAttribute("data-clipping");
  fireEvent.click(clip);
  expect(
    screen.getByRole("button", { name: "Clip indicator" })
  ).not.toHaveAttribute("data-clipping");
});
