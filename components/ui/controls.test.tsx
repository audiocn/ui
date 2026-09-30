import { fireEvent, render, screen } from "@testing-library/react";
import type { CSSProperties } from "react";
import { describe, expect, it, vi } from "vitest";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { MuteToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import {
  Knob,
  KnobDial,
  KnobLabel,
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
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
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
