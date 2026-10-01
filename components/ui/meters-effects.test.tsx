import { act, render, screen, waitFor } from "@testing-library/react";
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ChannelStrip } from "@/components/ui/channel-strip";
import { DbReadout } from "@/components/ui/db-readout";
import { LevelMeter, LevelMeterValue } from "@/components/ui/level-meter";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const CLIP_HOLD_MS = 1500;

/** Re-renders every 100 ms and passes a new inline `format` each time. */
const BusyParent = ({ source }: { source: FrameSource<MeterFrame> }) => {
  const [ticks, setTicks] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => setTicks((value) => value + 1), 100);
    return () => clearInterval(timer);
  }, []);
  return (
    <DbReadout
      data-ticks={ticks}
      format={(db) => `${db.toFixed(1)} dB`}
      source={source}
    />
  );
};

describe("meters across re-renders", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("clears data-clipping when the painter is rebuilt mid-clip", () => {
    const { rerender } = render(<LevelMeter aria-label="Mic" peakDb={0} />);
    advance(50);
    const meter = screen.getByRole("meter");
    expect(meter).toHaveAttribute("data-clipping");

    // A ballistics change rebuilds the painter while the clip is held.
    rerender(<LevelMeter aria-label="Mic" ballistics="vu" peakDb={-20} />);
    advance(CLIP_HOLD_MS + 500);
    expect(meter).not.toHaveAttribute("data-clipping");
  });

  it("renders its zone fill on the server, under the user's style", () => {
    const html = renderToString(<LevelMeter aria-label="Mic" peakDb={-12} />);
    expect(html).toContain("--meter-fill:linear-gradient");
    render(
      <LevelMeter
        aria-label="Styled"
        peakDb={-12}
        style={{ "--meter-fill": "var(--meter-warn)" } as CSSProperties}
      />
    );
    expect(
      screen.getByRole("meter").style.getPropertyValue("--meter-fill")
    ).toBe("var(--meter-warn)");
  });

  it("keeps showing a declarative level in LevelMeterValue", () => {
    render(
      <LevelMeter aria-label="Mic" peakDb={-12}>
        <LevelMeterValue />
      </LevelMeter>
    );
    advance(1000);
    expect(
      document.querySelector("[data-slot='db-readout']")
    ).toHaveTextContent("−12.0 dB");
  });

  it("keeps a readout updating while its parent re-renders faster than it ticks", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(<BusyParent source={emitter} />);
    act(() => {
      emitter.emit({ channels: [{ peakDb: -6 }] });
    });
    // Small steps, so each parent re-render commits between timer ticks.
    for (let step = 0; step < 6; step += 1) {
      advance(50);
    }
    expect(
      document.querySelector("[data-slot='db-readout']")
    ).toHaveTextContent("-6.0 dB");
  });
});

const strip = (withMeter: boolean) => (
  <ChannelStrip>
    {withMeter ? <div data-clipping="" data-slot="level-meter" /> : null}
  </ChannelStrip>
);

describe("ChannelStrip", () => {
  it("clears data-clipping when a clipping meter is removed", async () => {
    const { container, rerender } = render(strip(true));
    const root = container.querySelector("[data-slot='channel-strip']");
    await waitFor(() => {
      expect(root).toHaveAttribute("data-clipping");
    });
    rerender(strip(false));
    await waitFor(() => {
      expect(root).not.toHaveAttribute("data-clipping");
    });
  });
});
