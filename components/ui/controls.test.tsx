import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { MuteToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import { Knob } from "@/components/ui/knob";
import { formatPan, PanControl } from "@/components/ui/pan-control";
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
});

describe("PanControl", () => {
  it("formats pan positions", () => {
    expect(formatPan(0)).toBe("C");
    expect(formatPan(-0.3)).toBe("L30");
    expect(formatPan(1)).toBe("R100");
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
