import { fireEvent, render, screen } from "@testing-library/react";
import type { CSSProperties } from "react";
import { describe, expect, it, vi } from "vitest";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { MuteToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import {
  Knob,
  KnobCap,
  KnobDial,
  KnobLabel,
  KnobScale,
  KnobValue,
  parseKnobValue,
} from "@/components/ui/knob";
import { formatPan, PanControl, parsePan } from "@/components/ui/pan-control";
import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderLabel,
} from "@/components/ui/parameter-slider";
import { VolumeControl } from "@/components/ui/volume-control";

const faderInput = () => screen.getByRole("slider");

describe("Fader", () => {
  it("reports its value in dB to assistive technology", () => {
    render(<Fader aria-label="Mic" defaultValue={-6} />);
    expect(faderInput()).toHaveAttribute("aria-valuetext", "−6.0 dB");
    expect(faderInput()).toHaveAccessibleName("Mic");
  });

  it("steps by step, largeStep and fineStep", () => {
    const onValueChange = vi.fn();
    render(<Fader defaultValue={0} onValueChange={onValueChange} />);
    fireEvent.keyDown(faderInput(), { key: "ArrowDown" });
    expect(onValueChange).toHaveBeenLastCalledWith(-0.5, expect.anything());
    fireEvent.keyDown(faderInput(), { key: "ArrowDown", shiftKey: true });
    expect(onValueChange).toHaveBeenLastCalledWith(-6.5, expect.anything());
    fireEvent.keyDown(faderInput(), { altKey: true, key: "ArrowUp" });
    expect(onValueChange).toHaveBeenLastCalledWith(-6.4, expect.anything());
  });

  it("jumps to the ends with Home and End, and to silence when allowed", () => {
    const onValueChange = vi.fn();
    render(
      <Fader defaultValue={0} onValueChange={onValueChange} silenceAtMin />
    );
    fireEvent.keyDown(faderInput(), { key: "End" });
    expect(onValueChange).toHaveBeenLastCalledWith(6, expect.anything());
    fireEvent.keyDown(faderInput(), { key: "Home" });
    expect(onValueChange).toHaveBeenLastCalledWith(
      Number.NEGATIVE_INFINITY,
      expect.anything()
    );
    expect(faderInput()).toHaveAttribute("aria-valuetext", "Silent");
  });

  it("stays put when controlled without an update", () => {
    render(<Fader onValueChange={vi.fn()} value={-12} />);
    fireEvent.keyDown(faderInput(), { key: "ArrowUp" });
    expect(faderInput()).toHaveAttribute("aria-valuetext", "−12.0 dB");
  });

  it("commits keyboard changes", () => {
    const onValueCommitted = vi.fn();
    render(<Fader defaultValue={-3} onValueCommitted={onValueCommitted} />);
    fireEvent.keyDown(faderInput(), { key: "PageUp" });
    expect(onValueCommitted).toHaveBeenCalledWith(3);
  });
});

describe("ParameterSlider", () => {
  it("steps within its range and labels the thumb", () => {
    const onValueChange = vi.fn();
    render(
      <ParameterSlider
        max={1000}
        min={-1000}
        onValueChange={onValueChange}
        step={5}
        unit="ms"
      >
        <ParameterSliderLabel>Sync</ParameterSliderLabel>
        <ParameterSliderControl />
      </ParameterSlider>
    );
    const thumb = screen.getByRole("slider", { name: "Sync" });
    fireEvent.keyDown(thumb, { key: "ArrowUp" });
    expect(onValueChange).toHaveBeenLastCalledWith(-995, expect.anything());
    fireEvent.keyDown(thumb, { key: "End" });
    expect(onValueChange).toHaveBeenLastCalledWith(1000, expect.anything());
  });
});

/** The knob's dial, laid out as a 100px square at the page origin. */
const circularDial = () => {
  const dial = screen.getByRole("slider");
  dial.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  dial.setPointerCapture = vi.fn();
  dial.hasPointerCapture = vi.fn(() => true);
  dial.releasePointerCapture = vi.fn();
  return dial;
};

describe("Knob", () => {
  it("is a slider that responds to the keyboard and double-click", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = screen.getByRole("slider");
    expect(dial).toHaveAttribute("aria-valuenow", "50");
    fireEvent.keyDown(dial, { key: "ArrowUp", shiftKey: true });
    expect(onValueChange).toHaveBeenLastCalledWith(60, expect.anything());
    fireEvent.doubleClick(dial);
    expect(onValueChange).toHaveBeenLastCalledWith(50, expect.anything());
  });

  it("keeps its angle when given a style", () => {
    render(
      <Knob aria-label="Gain" defaultValue={50}>
        <KnobDial style={{ "--from-test": "1" } as CSSProperties} />
      </Knob>
    );
    const dial = screen.getByRole("slider");
    expect(dial.style.getPropertyValue("--knob-angle")).not.toBe("");
    expect(dial.style.getPropertyValue("--from-test")).toBe("1");
  });

  it("resets on Alt+click", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = screen.getByRole("slider");
    fireEvent.keyDown(dial, { key: "ArrowUp" });
    fireEvent.pointerDown(dial, { altKey: true, button: 0 });
    expect(onValueChange).toHaveBeenLastCalledWith(50, { reason: "reset" });
  });

  it("drags ten times finer with Shift, without jumping when Shift changes", () => {
    const onValueChange = vi.fn();
    render(
      <Knob
        aria-label="Gain"
        defaultValue={50}
        dragDirection="vertical"
        onValueChange={onValueChange}
      />
    );
    const dial = screen.getByRole("slider");
    dial.setPointerCapture = vi.fn();
    dial.hasPointerCapture = vi.fn(() => true);
    dial.releasePointerCapture = vi.fn();
    fireEvent.pointerDown(dial, { button: 0, clientY: 100 });
    // 20px up is a tenth of the 200px sensitivity: 10 units, or 1 with Shift.
    fireEvent.pointerMove(dial, { clientY: 80, shiftKey: true });
    expect(onValueChange).toHaveBeenLastCalledWith(51, expect.anything());
    fireEvent.pointerMove(dial, { clientY: 60 });
    expect(onValueChange).toHaveBeenLastCalledWith(61, expect.anything());
  });

  it("turns from where it is grabbed when circled, and stops at its ends", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = circularDial();
    // Grabbed at 3 o'clock, a quarter turn is a third of the 270° arc.
    fireEvent.pointerDown(dial, { button: 0, clientX: 90, clientY: 50 });
    fireEvent.pointerMove(dial, { clientX: 50, clientY: 90 });
    expect(onValueChange).toHaveBeenLastCalledWith(83, expect.anything());
    // Past the end, across the gap at the bottom, it stays at the end.
    fireEvent.pointerMove(dial, { clientX: 10, clientY: 50 });
    expect(onValueChange).toHaveBeenLastCalledWith(100, expect.anything());
    // Turning back moves it straight away.
    fireEvent.pointerMove(dial, { clientX: 50, clientY: 90 });
    expect(onValueChange).toHaveBeenLastCalledWith(67, expect.anything());
  });

  it("ignores the pointer near the centre of the dial while circling", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = circularDial();
    fireEvent.pointerDown(dial, { button: 0, clientX: 90, clientY: 50 });
    fireEvent.pointerMove(dial, { clientX: 48, clientY: 52 });
    // Leaving the centre picks up from there, without a jump.
    fireEvent.pointerMove(dial, { clientX: 50, clientY: 10 });
    expect(onValueChange).not.toHaveBeenCalled();
    fireEvent.pointerMove(dial, { clientX: 90, clientY: 50 });
    expect(onValueChange).toHaveBeenLastCalledWith(83, expect.anything());
  });

  it("edits the value on double-click and returns focus to the dial", () => {
    const onValueCommitted = vi.fn();
    render(
      <Knob defaultValue={50} onValueCommitted={onValueCommitted}>
        <KnobDial />
        <KnobValue />
        <KnobLabel>Gain</KnobLabel>
      </Knob>
    );
    fireEvent.doubleClick(screen.getByText("50"));
    const input = screen.getByRole("textbox", { name: "Value" });
    fireEvent.change(input, { target: { value: "72" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onValueCommitted).toHaveBeenLastCalledWith(72);
    expect(screen.getByText("72")).toBeInTheDocument();
    expect(screen.getByRole("slider")).toHaveFocus();
  });

  it("opens the editor from the label or Enter, and Escape cancels", () => {
    const onValueCommitted = vi.fn();
    render(
      <Knob defaultValue={50} onValueCommitted={onValueCommitted}>
        <KnobDial />
        <KnobValue />
        <KnobLabel>Gain</KnobLabel>
      </Knob>
    );
    fireEvent.doubleClick(screen.getByText("Gain"));
    const input = screen.getByRole("textbox", { name: "Value" });
    fireEvent.change(input, { target: { value: "10" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onValueCommitted).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("slider"), { key: "Enter" });
    expect(screen.getByRole("textbox", { name: "Value" })).toHaveValue("50");
  });

  it("draws a numbered scale, lit from the origin to the value", () => {
    const { container } = render(
      <Knob aria-label="Volume" defaultValue={33}>
        <KnobDial>
          <KnobScale labelEvery={10} majorEvery={5} ticks={100} />
        </KnobDial>
      </Knob>
    );
    const ticks = container.querySelectorAll("[data-slot='knob-tick']");
    const labels = container.querySelectorAll("[data-slot='knob-scale-label']");
    expect(ticks).toHaveLength(101);
    expect(container.querySelectorAll("[data-major]")).toHaveLength(21);
    expect([...labels].map((label) => label.textContent)).toEqual([
      "0",
      "10",
      "20",
      "30",
      "40",
      "50",
      "60",
      "70",
      "80",
      "90",
      "100",
    ]);
    expect(container.querySelectorAll("[data-active]")).toHaveLength(34);
    fireEvent.keyDown(screen.getByRole("slider"), { key: "End" });
    expect(container.querySelectorAll("[data-active]")).toHaveLength(101);
  });

  it("lights a bipolar scale from its centre", () => {
    const { container } = render(
      <Knob aria-label="Gain" defaultValue={-12} max={24} min={-24} origin={0}>
        <KnobDial>
          <KnobScale labelEvery={0} ticks={48} />
        </KnobDial>
      </Knob>
    );
    const lit = [
      ...container.querySelectorAll<SVGLineElement>("[data-slot='knob-tick']"),
    ]
      .map((tick, index) =>
        Object.hasOwn(tick.dataset, "active") ? index : null
      )
      .filter((index) => index !== null);
    // -12 dB to 0 dB is ticks 12 to 24 of 48.
    expect(lit).toEqual(Array.from({ length: 13 }, (_, index) => index + 12));
    expect(
      container.querySelectorAll("[data-slot='knob-scale-label']")
    ).toHaveLength(0);
  });

  it("turns the cap's grain with the dot while keeping the lighting fixed", () => {
    const { container } = render(
      <Knob aria-label="Volume" defaultValue={50}>
        <KnobDial>
          <KnobCap />
        </KnobDial>
      </Knob>
    );
    const dot = () => container.querySelector("[data-slot='knob-cap-dot']");
    const grain = container.querySelector("[data-slot='knob-cap-grain']");
    const face = container.querySelector("[data-slot='knob-cap-face']");
    const faceMarkup = face?.outerHTML;
    expect(grain).toHaveAttribute("transform", "rotate(0 50 50)");
    // At the middle of the arc the dot is straight up.
    expect(Number(dot()?.getAttribute("cx"))).toBeCloseTo(50);
    expect(Number(dot()?.getAttribute("cy"))).toBeLessThan(50);
    fireEvent.keyDown(screen.getByRole("slider"), { key: "End" });
    expect(Number(dot()?.getAttribute("cx"))).toBeGreaterThan(50);
    expect(Number(dot()?.getAttribute("cy"))).toBeGreaterThan(50);
    expect(grain).toHaveAttribute("transform", "rotate(135 50 50)");
    fireEvent.keyDown(screen.getByRole("slider"), { key: "Home" });
    expect(grain).toHaveAttribute("transform", "rotate(-135 50 50)");
    expect(face?.outerHTML).toBe(faceMarkup);
  });

  it("reads typed values", () => {
    expect(parseKnobValue("\u221212 dB")).toBe(-12);
    expect(parseKnobValue("1.2k")).toBe(1200);
    expect(parseKnobValue("gain")).toBeNull();
  });
});

describe("PanControl", () => {
  it("formats pan positions", () => {
    expect(formatPan(0)).toBe("C");
    expect(formatPan(-0.3)).toBe("L30");
    expect(formatPan(1)).toBe("R100");
    expect(parsePan("L30")).toBe(-0.3);
    expect(parsePan("r15")).toBe(0.15);
    expect(parsePan("C")).toBe(0);
    expect(parsePan("-50")).toBe(-0.5);
    expect(parsePan("left")).toBeNull();
  });

  it("describes its value", () => {
    render(<PanControl defaultValue={0.25} />);
    expect(screen.getByRole("slider", { name: "Pan" })).toHaveAttribute(
      "aria-valuetext",
      "25% right"
    );
  });
});

describe("MuteToggle", () => {
  it("toggles its pressed state", () => {
    const onPressedChange = vi.fn();
    render(<MuteToggle onPressedChange={onPressedChange}>M</MuteToggle>);
    const toggle = screen.getByRole("button", { name: "Mute" });
    fireEvent.click(toggle);
    expect(onPressedChange).toHaveBeenCalledWith(true, expect.anything());
    expect(toggle).toHaveAttribute("aria-pressed", "true");
  });
});

describe("VolumeControl", () => {
  it("mutes and restores the last audible volume", () => {
    const onValueChange = vi.fn();
    render(<VolumeControl defaultValue={0} onValueChange={onValueChange} />);
    const mute = screen.getByRole("button", { name: "Mute" });
    expect(mute).toHaveAttribute("data-level", "muted");
    fireEvent.click(mute);
    fireEvent.click(screen.getByRole("button", { name: "Unmute" }));
    expect(onValueChange).toHaveBeenCalledWith(1);
  });
});

describe("AudioDeviceSelect", () => {
  it("shows the selected device", () => {
    render(
      <AudioDeviceSelect
        defaultValue="usb"
        devices={[
          { id: "default", label: "Built-in" },
          { id: "usb", label: "USB mic" },
        ]}
      />
    );
    expect(screen.getByRole("combobox")).toHaveTextContent("USB mic");
  });

  it("keeps a disconnected device visible", () => {
    render(
      <AudioDeviceSelect
        defaultValue="gone"
        devices={[{ id: "a", label: "A" }]}
      />
    );
    expect(screen.getByRole("combobox")).toHaveTextContent(
      "Unknown device (disconnected)"
    );
    expect(screen.getByRole("combobox")).toHaveAttribute("data-missing");
  });

  it("shows None when allowed", () => {
    render(
      <AudioDeviceSelect allowNone devices={[]} noneLabel="No microphone" />
    );
    expect(screen.getByRole("combobox")).toHaveTextContent("No microphone");
  });
});
