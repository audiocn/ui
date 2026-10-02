import { act, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { ClipIndicator } from "@/components/ui/clip-indicator";
import type { ClipIndicatorActions } from "@/components/ui/clip-indicator";
import { DbReadout } from "@/components/ui/db-readout";
import { DbScale, thinDbScaleLabels } from "@/components/ui/db-scale";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

beforeEach(() => {
  useFakeFrames();
});

afterEach(() => {
  vi.useRealTimers();
});

/** Places each label along the scale, as layout would. */
const placeLabels = (
  scale: HTMLElement,
  spans: Record<string, [number, number]>
) => {
  for (const tick of scale.querySelectorAll<HTMLElement>(
    '[data-slot="db-scale-tick"]'
  )) {
    const [left, right] = spans[tick.dataset.value ?? ""] ?? [0, 0];
    const label = tick.querySelector<HTMLElement>(
      '[data-slot="db-scale-label"]'
    );
    if (label) {
      label.getBoundingClientRect = () =>
        DOMRect.fromRect({ height: 10, width: right - left, x: left, y: 0 });
    }
  }
};

describe("DbScale", () => {
  it("renders common ticks inside the range", () => {
    const { container } = render(<DbScale maxDb={0} minDb={-24} />);
    const labels = [
      ...container.querySelectorAll('[data-slot="db-scale-label"]'),
    ].map((label) => label.textContent);
    expect(labels).toEqual(["0", "−6", "−12", "−18", "−24"]);
  });

  it("hides labels that would touch 0 dB or the ends", () => {
    const { container } = render(<DbScale ticks={[0, -6, -12, -54, -60]} />);
    const scale = container.querySelector<HTMLElement>(
      '[data-slot="db-scale"]'
    );
    if (!scale) {
      throw new Error("No scale rendered.");
    }
    placeLabels(scale, {
      "-12": [70, 80],
      "-54": [8, 18],
      "-6": [92, 102],
      "-60": [0, 10],
      "0": [100, 110],
    });
    thinDbScaleLabels(scale);
    const hidden = [
      ...scale.querySelectorAll('[data-slot="db-scale-label"][data-hidden]'),
    ].map((label) => label.textContent);
    expect(hidden).toEqual(["−6", "−54"]);
  });

  it("renders custom ticks", () => {
    const { container } = render(<DbScale ticks={[-3, -9]} />);
    expect(
      container.querySelectorAll('[data-slot="db-scale-tick"]')
    ).toHaveLength(2);
  });
});

describe("DbReadout", () => {
  it("formats a declarative value", () => {
    render(<DbReadout value={-6} />);
    expect(screen.getByText("−6.0 dB")).toBeInTheDocument();
  });

  it("updates from a source at its interval", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    const { container } = render(<DbReadout source={emitter} />);
    const readout = container.querySelector('[data-slot="db-readout"]');
    expect(readout).toHaveTextContent("−∞ dB");
    emitter.emit({ channels: [{ peakDb: -12.34 }] });
    advance(260);
    expect(readout).toHaveTextContent("−12.3 dB");
    expect(readout).toHaveAttribute("data-zone", "warn");
  });

  it("shows the value again when its source goes away", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    const { container, rerender } = render(<DbReadout source={emitter} />);
    emitter.emit({ channels: [{ peakDb: -12.34 }] });
    advance(260);
    // The value renders the same text as the live readout's first paint, so
    // React alone would leave the last live level on screen.
    rerender(<DbReadout value={Number.NEGATIVE_INFINITY} />);
    const readout = container.querySelector('[data-slot="db-readout"]');
    expect(readout).toHaveTextContent("−∞ dB");
    expect(readout).toHaveAttribute("data-zone", "ok");
    expect(readout).toHaveAttribute("data-silent");
  });

  it("shows the live level again when a source comes back", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    const { container, rerender } = render(<DbReadout source={emitter} />);
    emitter.emit({ channels: [{ peakDb: -12.34 }] });
    advance(260);
    rerender(<DbReadout value={Number.NEGATIVE_INFINITY} />);
    rerender(<DbReadout source={emitter} />);
    emitter.emit({ channels: [{ peakDb: -12.34 }] });
    advance(260);
    const readout = container.querySelector('[data-slot="db-readout"]');
    expect(readout).toHaveTextContent("−12.3 dB");
    expect(readout).toHaveAttribute("data-zone", "warn");
  });
});

describe("ClipIndicator", () => {
  it("lights up on a clip, counts it and resets on click", () => {
    const actions = createRef<ClipIndicatorActions>();
    render(<ClipIndicator actionsRef={actions} showCount />);
    const button = screen.getByRole("button");
    expect(button).not.toHaveAttribute("data-clipping");

    act(() => {
      actions.current?.report(0);
    });
    advance(10);
    expect(button).toHaveAttribute("data-clipping");
    expect(button).toHaveTextContent("1");

    fireEvent.click(button);
    expect(button).not.toHaveAttribute("data-clipping");
    expect(button).toHaveTextContent("0");
  });

  it("turns off after the hold time", () => {
    const actions = createRef<ClipIndicatorActions>();
    render(<ClipIndicator actionsRef={actions} holdMs={500} />);
    act(() => {
      actions.current?.report(-0.5);
    });
    advance(10);
    expect(screen.getByRole("button")).toHaveAttribute("data-clipping");
    advance(600);
    expect(screen.getByRole("button")).not.toHaveAttribute("data-clipping");
  });

  it("follows the controlled prop", () => {
    render(<ClipIndicator clipping />);
    expect(screen.getByRole("button")).toHaveAttribute("data-clipping");
  });
});

describe("BarVisualizer", () => {
  it("renders the requested number of bars", () => {
    const { container } = render(<BarVisualizer barCount={7} />);
    expect(
      container.querySelectorAll('[data-slot="bar-visualizer-bar"]')
    ).toHaveLength(7);
  });

  it("paints declarative levels", () => {
    const { container } = render(
      <BarVisualizer barCount={2} levels={[0.5, 1]} minLevel={0} />
    );
    advance(100);
    const bars = container.querySelectorAll<HTMLElement>(
      '[data-slot="bar-visualizer-bar"]'
    );
    expect(bars[0]?.style.getPropertyValue("--bar-level")).toBe("0.5000");
    expect(bars[1]?.style.getPropertyValue("--bar-level")).toBe("1.0000");
  });
});
