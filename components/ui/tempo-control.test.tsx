import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import TempoControlPadDemo from "@/components/examples/tempo-control-pad";
import {
  TempoControl,
  TempoControlInput,
  TempoControlTap,
} from "@/components/ui/tempo-control";
import { AudioConfigProvider } from "@/hooks/use-audio-config";

const input = () =>
  screen.getByRole("textbox", { name: "Tempo in beats per minute" });
const tapAt = (time: number) => {
  vi.spyOn(performance, "now").mockReturnValue(time);
  fireEvent.click(screen.getByRole("button", { name: "Tap tempo" }));
};

afterEach(() => vi.restoreAllMocks());

describe("TempoControl", () => {
  it("rounds fractional defaults, typed values and commits to whole BPM", () => {
    const onValueChange = vi.fn();
    const onValueCommitted = vi.fn();
    render(
      <TempoControl
        defaultValue={123.6}
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
      />
    );
    expect(input()).toHaveValue("124");
    fireEvent.change(input(), { target: { value: "96.4" } });
    fireEvent.blur(input());
    expect(input()).toHaveValue("96");
    expect(onValueChange).toHaveBeenLastCalledWith(
      96,
      expect.objectContaining({ reason: "input" })
    );
    expect(onValueCommitted).toHaveBeenLastCalledWith(96);
    fireEvent.keyDown(input(), { altKey: true, key: "ArrowUp" });
    expect(input()).toHaveValue("97");
  });

  it("rounds controlled BPM and fractional large steps", () => {
    const onValueChange = vi.fn();
    render(
      <TempoControl
        value={90.6}
        largeStep={2.5}
        onValueChange={onValueChange}
      />
    );
    expect(input()).toHaveValue("91");
    fireEvent.keyDown(input(), { key: "ArrowUp", shiftKey: true });
    expect(onValueChange).toHaveBeenLastCalledWith(
      94,
      expect.objectContaining({ reason: "keyboard" })
    );
    expect(input()).toHaveValue("91");
  });

  it("uses the visible label when the default aria-label is removed", () => {
    render(
      <>
        <label htmlFor="delay-tempo">Delay tempo</label>
        <TempoControl>
          <TempoControlInput id="delay-tempo" aria-label={undefined} />
        </TempoControl>
      </>
    );
    expect(screen.getByRole("textbox")).toHaveAccessibleName("Delay tempo");
  });

  it("labels its input and waits for two taps", () => {
    const onValueChange = vi.fn();
    render(<TempoControl onValueChange={onValueChange} />);
    expect(input()).toHaveValue("120");
    tapAt(0);
    expect(onValueChange).not.toHaveBeenCalled();
    tapAt(600);
    expect(input()).toHaveValue("100");
    expect(onValueChange).toHaveBeenLastCalledWith(
      100,
      expect.objectContaining({ reason: "tap" })
    );
  });

  it("averages intervals instead of averaging BPM estimates", () => {
    render(<TempoControl />);
    tapAt(0);
    tapAt(400);
    tapAt(1000);
    expect(input()).toHaveValue("120");
  });

  it("keeps only four recent intervals", () => {
    render(<TempoControl />);
    tapAt(0);
    tapAt(1000);
    for (let index = 1; index <= 4; index += 1) {
      tapAt(1000 + index * 500);
    }
    expect(input()).toHaveValue("120");
  });

  it("starts again after an idle pause without changing BPM on the first tap", () => {
    render(<TempoControl />);
    tapAt(0);
    tapAt(600);
    tapAt(4000);
    expect(input()).toHaveValue("100");
    tapAt(4500);
    expect(input()).toHaveValue("120");
  });

  it("honours a custom timeout", () => {
    render(<TempoControl tapTimeout={3000} />);
    tapAt(0);
    tapAt(600);
    tapAt(3700);
    expect(input()).toHaveValue("100");
    tapAt(4200);
    expect(input()).toHaveValue("120");
  });

  it("accepts gaps up to two seconds and resets above two seconds", () => {
    render(<TempoControl min={1} />);
    tapAt(0);
    tapAt(1999);
    expect(input()).toHaveValue("30");
    tapAt(4000);
    expect(input()).toHaveValue("30");
    tapAt(4500);
    expect(input()).toHaveValue("120");
  });

  it("reacquires a slower tempo after a short pause", () => {
    render(<TempoControl />);
    for (const time of [0, 250, 500, 750, 1000]) {
      tapAt(time);
    }
    expect(input()).toHaveValue("240");
    tapAt(3200);
    expect(input()).toHaveValue("240");
    for (const time of [4200, 5200]) {
      tapAt(time);
    }
    expect(input()).toHaveValue("60");
  });

  it("supports tapping the default minimum without a timeout override", () => {
    render(<TempoControl />);
    tapAt(0);
    tapAt(2000);
    expect(input()).toHaveValue("30");
  });

  it("keeps smoothing normal timing variation", () => {
    render(<TempoControl />);
    for (const time of [0, 500, 1050, 1500, 2020]) {
      tapAt(time);
    }
    expect(input()).toHaveValue("119");
  });

  it("does not treat a single missed beat as a new tempo", () => {
    render(<TempoControl />);
    for (const time of [0, 500, 1000, 2000, 2500]) {
      tapAt(time);
    }
    expect(input()).toHaveValue("96");
  });

  it("ignores duplicate and accidental rapid taps without moving the last timestamp", () => {
    const onValueCommitted = vi.fn();
    render(<TempoControl onValueCommitted={onValueCommitted} />);
    tapAt(0);
    tapAt(0);
    tapAt(40);
    expect(onValueCommitted).not.toHaveBeenCalled();
    tapAt(600);
    expect(onValueCommitted).toHaveBeenLastCalledWith(100);
  });

  it("clamps tapped BPM to the configured bounds", () => {
    render(<TempoControl min={60} max={180} tapTimeout={3000} />);
    tapAt(0);
    tapAt(100);
    expect(input()).toHaveValue("180");
    tapAt(4000);
    tapAt(6000);
    expect(input()).toHaveValue("60");
  });

  it("rounds tapped BPM to the nearest integer", () => {
    const onValueCommitted = vi.fn();
    render(<TempoControl onValueCommitted={onValueCommitted} />);
    tapAt(0);
    tapAt(487);
    expect(onValueCommitted).toHaveBeenLastCalledWith(123);
  });

  it("reports a tap without changing a parent-controlled value", () => {
    const onValueChange = vi.fn();
    const { rerender } = render(
      <TempoControl value={90} onValueChange={onValueChange} />
    );
    tapAt(0);
    tapAt(500);
    expect(onValueChange).toHaveBeenLastCalledWith(120, expect.anything());
    expect(input()).toHaveValue("90");
    rerender(<TempoControl value={120} onValueChange={onValueChange} />);
    expect(input()).toHaveValue("120");
  });

  it("commits a measured tempo even when its value is unchanged", () => {
    const onValueChange = vi.fn();
    const onValueCommitted = vi.fn();
    render(
      <TempoControl
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
      />
    );
    tapAt(0);
    tapAt(500);
    expect(onValueChange).not.toHaveBeenCalled();
    expect(onValueCommitted).toHaveBeenCalledWith(120);
  });

  it("steps, clamps and commits using Base UI keyboard controls", () => {
    const onValueCommitted = vi.fn();
    render(<TempoControl onValueCommitted={onValueCommitted} />);
    fireEvent.keyDown(input(), { key: "ArrowUp" });
    expect(input()).toHaveValue("121");
    fireEvent.keyDown(input(), { key: "ArrowDown", shiftKey: true });
    expect(input()).toHaveValue("111");
    fireEvent.keyDown(input(), { key: "Home" });
    expect(input()).toHaveValue("30");
    fireEvent.keyDown(input(), { key: "End" });
    expect(input()).toHaveValue("600");
    expect(onValueCommitted).toHaveBeenLastCalledWith(600);
  });

  it("accepts typed BPM and resets tap history after an edit", async () => {
    const user = userEvent.setup();
    render(<TempoControl />);
    tapAt(0);
    await user.clear(input());
    await user.type(input(), "95");
    await user.tab();
    expect(input()).toHaveValue("95");
    tapAt(500);
    expect(input()).toHaveValue("95");
    tapAt(1000);
    expect(input()).toHaveValue("120");
  });

  it("resets tap history on partial input while preserving consumer handlers", () => {
    const onValueChange = vi.fn();
    const onValueCommitted = vi.fn();
    const onChange = vi.fn();
    render(
      <TempoControl
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
      >
        <TempoControlInput onChange={onChange} />
        <TempoControlTap />
      </TempoControl>
    );
    tapAt(0);
    fireEvent.change(input(), { target: { value: "." } });
    expect(input()).toHaveValue(".");
    expect(onChange).toHaveBeenCalledTimes(1);
    fireEvent.blur(input());
    tapAt(600);
    expect(onValueChange).not.toHaveBeenCalled();
    expect(onValueCommitted).not.toHaveBeenCalled();
    tapAt(1200);
    expect(input()).toHaveValue("100");
    expect(onValueChange).toHaveBeenLastCalledWith(
      100,
      expect.objectContaining({ reason: "tap" })
    );
    expect(onValueCommitted).toHaveBeenCalledExactlyOnceWith(100);
  });

  it("restores the last numeric value when an empty edit is blurred", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    render(<TempoControl defaultValue={96} onValueChange={onValueChange} />);
    await user.clear(input());
    await user.tab();
    expect(input()).toHaveValue("96");
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it("inherits disabled state and resets taps when re-enabled", () => {
    const { rerender } = render(
      <AudioConfigProvider value={{ disabled: false }}>
        <TempoControl />
      </AudioConfigProvider>
    );
    const originalInput = input();
    tapAt(0);
    rerender(
      <AudioConfigProvider value={{ disabled: true }}>
        <TempoControl />
      </AudioConfigProvider>
    );
    expect(input()).toBeDisabled();
    expect(input()).toBe(originalInput);
    expect(screen.getByRole("button")).toBeDisabled();
    rerender(
      <AudioConfigProvider value={{ disabled: false }}>
        <TempoControl />
      </AudioConfigProvider>
    );
    tapAt(600);
    expect(input()).toBe(originalInput);
    expect(input()).toHaveValue("120");
    tapAt(1200);
    expect(input()).toHaveValue("100");
  });

  it("keeps read-only input focusable and disables tap", () => {
    render(<TempoControl readOnly />);
    expect(input()).toHaveAttribute("readonly");
    expect(input()).not.toBeDisabled();
    expect(screen.getByRole("button")).toBeDisabled();
    fireEvent.keyDown(input(), { key: "ArrowUp" });
    expect(input()).toHaveValue("120");
  });

  it("allows a consumer to cancel a tap", () => {
    const onValueCommitted = vi.fn();
    render(
      <TempoControl onValueCommitted={onValueCommitted}>
        <TempoControlInput />
        <TempoControlTap onClick={(event) => event.preventDefault()} />
      </TempoControl>
    );
    tapAt(0);
    tapAt(500);
    expect(onValueCommitted).not.toHaveBeenCalled();
  });

  it("prevents held activation keys from generating repeated taps", () => {
    render(<TempoControl />);
    expect(
      fireEvent.keyDown(screen.getByRole("button"), {
        key: "Enter",
        repeat: true,
      })
    ).toBe(false);
    expect(
      fireEvent.keyDown(screen.getByRole("button"), { key: " ", repeat: true })
    ).toBe(false);
  });

  it("supports a sound pad as the only tap trigger", () => {
    render(<TempoControlPadDemo />);
    const pad = screen.getByRole("button", { name: "Tap tempo" });
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(pad).toHaveAttribute("data-sound-pad");
    vi.spyOn(performance, "now").mockReturnValue(0);
    fireEvent.pointerDown(pad, { button: 0 });
    fireEvent.pointerUp(pad);
    fireEvent.click(pad);
    vi.spyOn(performance, "now").mockReturnValue(600);
    fireEvent.pointerDown(pad, { button: 0 });
    fireEvent.pointerUp(pad);
    fireEvent.click(pad);
    expect(screen.getByRole("status", { name: "Tempo" })).toHaveTextContent(
      "100 BPM"
    );
  });

  it("does not register page-wide tapping shortcuts", () => {
    const onValueCommitted = vi.fn();
    render(<TempoControl onValueCommitted={onValueCommitted} />);
    fireEvent.keyDown(document.body, { key: " " });
    fireEvent.keyDown(document.body, { key: " " });
    expect(onValueCommitted).not.toHaveBeenCalled();
  });

  it("does not double count Space on the focused default trigger", async () => {
    const user = userEvent.setup();
    const onValueCommitted = vi.fn();
    render(<TempoControl onValueCommitted={onValueCommitted} />);
    screen.getByRole("button").focus();
    vi.spyOn(performance, "now").mockReturnValue(0);
    await user.keyboard(" ");
    vi.spyOn(performance, "now").mockReturnValue(600);
    await user.keyboard(" ");
    expect(input()).toHaveValue("100");
    expect(onValueCommitted).toHaveBeenCalledTimes(1);
  });

  it("forwards refs, labels, classes and form names", () => {
    const ref = vi.fn();
    const { container } = render(
      <TempoControl name="tempo" className="gap-4">
        <TempoControlInput aria-label="Delay BPM" ref={ref} />
        <TempoControlTap>Measure</TempoControlTap>
      </TempoControl>
    );
    expect(
      screen.getByRole("textbox", { name: "Delay BPM" })
    ).toBeInTheDocument();
    expect(ref).toHaveBeenCalledWith(expect.any(HTMLInputElement));
    expect(container.querySelector('[data-slot="tempo-control"]')).toHaveClass(
      "gap-4"
    );
    expect(container.querySelector('input[name="tempo"]')).toHaveValue(120);
  });
});
