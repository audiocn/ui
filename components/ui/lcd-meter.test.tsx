import { act, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LcdDisplay } from "@/components/ui/lcd-display";
import {
  LcdMeter,
  LcdMeterBar,
  LcdMeterChannel,
  LcdMeterLabel,
  LcdMeterTrack,
  LcdMeterValue,
} from "@/components/ui/lcd-meter";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const MINUS = "−";

const slot = (name: string) =>
  document.querySelectorAll<HTMLElement>(`[data-slot="${name}"]`);

const textsOf = (name: string) =>
  Array.from(slot(name), (element) => element.textContent);

describe("LcdMeter", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("must sit on an LcdDisplay", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {
      // React reports the thrown error; the assertion below is the test.
    });
    expect(() => render(<LcdMeter aria-label="Mic level" peakDb={-12} />)).toThrow(
      "LcdMeter must be used inside LcdDisplay."
    );
    error.mockRestore();
  });

  it("renders a named meter with every part and merges className last", () => {
    const ref = createRef<HTMLDivElement>();
    render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" className="py-1" peakDb={-12} ref={ref} />
      </LcdDisplay>
    );
    const meter = screen.getByRole("meter", { name: "Mic level" });
    expect(meter).toHaveAttribute("data-slot", "lcd-meter");
    expect(meter).toHaveClass("py-1");
    expect(ref.current).toBe(meter);
    for (const name of [
      "lcd-meter-channel",
      "lcd-meter-track",
      "lcd-meter-segments",
      "lcd-meter-bar",
      "lcd-meter-hold",
      "lcd-meter-value",
      "lcd-meter-scale",
      "lcd-meter-bracket",
      "lcd-meter-clip",
    ]) {
      expect(slot(name)).toHaveLength(1);
    }
  });

  it("labels a pair L and R, more channels by number, and one not at all", () => {
    const { rerender } = render(
      <LcdDisplay>
        <LcdMeter
          aria-label="Program"
          channels={[{ peakDb: -12 }, { peakDb: -24 }]}
        />
      </LcdDisplay>
    );
    expect(textsOf("lcd-meter-label")).toEqual(["L", "R"]);

    rerender(
      <LcdDisplay>
        <LcdMeter
          aria-label="Program"
          channels={[-6, -12, -18, -24].map((peakDb) => ({ peakDb }))}
        />
      </LcdDisplay>
    );
    expect(textsOf("lcd-meter-label")).toEqual(["1", "2", "3", "4"]);

    rerender(
      <LcdDisplay>
        <LcdMeter aria-label="Program" peakDb={-12} />
      </LcdDisplay>
    );
    expect(slot("lcd-meter-label")).toHaveLength(0);
    expect(slot("lcd-meter-track")).toHaveLength(1);
  });

  it("shows each channel's level in its own readout, without a unit", () => {
    render(
      <LcdDisplay>
        <LcdMeter
          aria-label="Program"
          channels={[{ peakDb: -12 }, { peakDb: -24 }]}
        />
      </LcdDisplay>
    );
    expect(textsOf("lcd-meter-value")).toEqual([
      `${MINUS}12.0`,
      `${MINUS}24.0`,
    ]);
  });

  it("paints its channels from a source on the audio taper", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" source={emitter} />
      </LcdDisplay>
    );
    act(() => {
      emitter.emit({ channels: [{ peakDb: -15 }] });
    });
    advance(400);
    const [channel] = slot("lcd-meter-channel");
    // 1 − √(15 / 60): a quarter of the range in dB is half the track.
    expect(
      Number(channel?.style.getPropertyValue("--meter-level"))
    ).toBeCloseTo(0.5, 2);
  });

  it("prints magnitudes on the scale, with a plus above 0", () => {
    const { rerender } = render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" peakDb={-12} />
      </LcdDisplay>
    );
    expect(textsOf("db-scale-label")).toEqual([
      "0",
      "3",
      "6",
      "12",
      "24",
      "48",
    ]);

    rerender(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" maxDb={6} peakDb={-12} />
      </LcdDisplay>
    );
    expect(textsOf("db-scale-label").slice(0, 3)).toEqual(["+6", "+3", "0"]);
  });

  it("starts the bracket where the clip zone does", () => {
    const { rerender } = render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" peakDb={-12} taper="linear" />
      </LcdDisplay>
    );
    // −9 dB on a linear −60..0 scale.
    expect(
      slot("lcd-meter-bracket")[0]?.style.getPropertyValue(
        "--lcd-meter-bracket-start"
      )
    ).toBe("0.8500");

    rerender(
      <LcdDisplay>
        <LcdMeter
          aria-label="Mic level"
          peakDb={-12}
          zones={[{ fromDb: Number.NEGATIVE_INFINITY, zone: "ok" }]}
        />
      </LcdDisplay>
    );
    expect(slot("lcd-meter-bracket")).toHaveLength(0);
  });

  it("inverts the CLIP flag while the signal clips", () => {
    render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" peakDb={0} />
      </LcdDisplay>
    );
    advance(100);
    expect(screen.getByRole("meter")).toHaveAttribute("data-clipping");
    const [flag] = slot("lcd-meter-clip");
    expect(flag).toHaveTextContent("Clip");
    expect(flag).toHaveAttribute("data-clipping");
  });

  it("masks its own dots when the screen's mask is off", () => {
    const { rerender } = render(
      <LcdDisplay>
        <LcdMeter aria-label="Mic level" peakDb={-12} />
      </LcdDisplay>
    );
    expect(slot("lcd-meter-track")[0]?.className).not.toContain(
      "mask-[radial-gradient"
    );

    rerender(
      <LcdDisplay matrix={false}>
        <LcdMeter aria-label="Mic level" peakDb={-12} />
      </LcdDisplay>
    );
    expect(slot("lcd-meter-track")[0]?.className).toContain(
      "mask-[radial-gradient"
    );
  });

  it("takes its size from the display", () => {
    render(
      <LcdDisplay size="lg">
        <LcdMeter aria-label="Mic level" peakDb={-12} />
      </LcdDisplay>
    );
    const meter = screen.getByRole("meter");
    expect(meter).toHaveAttribute("data-size", "lg");
    expect(meter.className).toContain(
      "[--lcd-meter-thickness:calc(var(--lcd-display-pitch)*10)]"
    );
  });

  it("composes custom rows, with the readout following its channel", () => {
    render(
      <LcdDisplay>
        <LcdMeter
          aria-label="Program"
          channels={[
            { peakDb: -6, rmsDb: -12 },
            { peakDb: -18, rmsDb: -24 },
          ]}
        >
          <LcdMeterChannel index={1}>
            <LcdMeterLabel>Out</LcdMeterLabel>
            <LcdMeterTrack>
              <LcdMeterBar measure="rms" />
            </LcdMeterTrack>
            <LcdMeterValue measure="rms" />
          </LcdMeterChannel>
        </LcdMeter>
      </LcdDisplay>
    );
    expect(textsOf("lcd-meter-label")).toEqual(["Out"]);
    expect(slot("lcd-meter-bar")[0]).toHaveAttribute("data-measure", "rms");
    expect(textsOf("lcd-meter-value")).toEqual([`${MINUS}24.0`]);
    expect(slot("lcd-meter-scale")).toHaveLength(0);
  });
});
