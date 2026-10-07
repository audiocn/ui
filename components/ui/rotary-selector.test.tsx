import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import { createRef, Fragment, useState } from "react";
import type { ComponentProps, ReactNode } from "react";
import { describe, expect, expectTypeOf, it, onTestFinished, vi } from "vitest";

import { KnobCap } from "@/components/ui/knob";
import {
  RotarySelector,
  RotarySelectorDial,
  RotarySelectorLabel,
  RotarySelectorPositionLabel,
  RotarySelectorPositionLeader,
  RotarySelectorPositionMark,
  RotarySelectorValue,
} from "@/components/ui/rotary-selector";
import type {
  RotarySelectorLeader,
  RotarySelectorPosition,
} from "@/components/ui/rotary-selector";
import { AudioConfigProvider } from "@/hooks/use-audio-config";
import { AudioContextProvider } from "@/hooks/use-audio-context";
import { createFakeAudioContext } from "@/test/fake-audio";

const WAVES = ["sine", "triangle", "saw", "square"] as const;
type Wave = (typeof WAVES)[number];

const TWELVE = [
  "AC",
  "BD",
  "SD",
  "LT",
  "MT",
  "HT",
  "RS",
  "CP",
  "CB",
  "CY",
  "OH",
  "CH",
] as const;

type SelectorProps = Omit<
  ComponentProps<typeof RotarySelector<typeof WAVES, number>>,
  "children" | "values"
>;

/** Four waves with a mark and a clickable label each, and a cap. */
const Waves = ({
  dial,
  ...props
}: SelectorProps & { dial?: ComponentProps<typeof RotarySelectorDial> }) => (
  <RotarySelector values={WAVES} {...props}>
    {({ positions }) => (
      <>
        <RotarySelectorDial {...dial}>
          {positions.map((position) => (
            <Fragment key={position.value}>
              <RotarySelectorPositionMark position={position} />
              <RotarySelectorPositionLabel position={position}>
                <text
                  {...position.pointAt(62)}
                >{`${position.value} label`}</text>
              </RotarySelectorPositionLabel>
            </Fragment>
          ))}
          <KnobCap />
        </RotarySelectorDial>
        <RotarySelectorValue />
        <RotarySelectorLabel>Waveform</RotarySelectorLabel>
      </>
    )}
  </RotarySelector>
);

const dial = () => screen.getByRole("slider");

const press = (key: string) => {
  const event = createEvent.keyDown(dial(), { key });
  fireEvent(dial(), event);
  return event;
};

const valueNow = () => Number(dial().getAttribute("aria-valuenow"));

/** A 100 × 100 px dial at the origin that accepts pointer capture. */
const draggable = () => {
  const element = dial();
  element.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  element.setPointerCapture = vi.fn();
  element.hasPointerCapture = vi.fn(() => true);
  element.releasePointerCapture = vi.fn();
  return element;
};

/** Client coordinates on a 100 px dial, at an angle clockwise from 12 o'clock. */
const around = (degrees: number) => {
  const radians = (degrees * Math.PI) / 180;
  return {
    clientX: 50 + 40 * Math.sin(radians),
    clientY: 50 - 40 * Math.cos(radians),
  };
};

/** Starts on square; a button picks triangle. */
const Controlled = () => {
  const [wave, setWave] = useState<Wave>("square");
  return (
    <>
      <button onClick={() => setWave("triangle")} type="button">
        Triangle
      </button>
      <Waves onValueChange={setWave} value={wave} />
    </>
  );
};

const hitArea = (container: HTMLElement) =>
  container.querySelector("[data-slot='rotary-selector-hit-area']");

/** Waves that click through a fake context, with clicks 100 ms apart. */
const clicking = (props: SelectorProps = {}) => {
  const provided = createFakeAudioContext();
  let now = 0;
  vi.spyOn(performance, "now").mockImplementation(() => {
    now += 100;
    return now;
  });
  onTestFinished(() => {
    vi.restoreAllMocks();
  });
  render(
    <AudioContextProvider context={provided.context}>
      <Waves {...props} />
    </AudioContextProvider>
  );
  return provided.sources;
};

const warnings = () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => null);
  onTestFinished(() => {
    warn.mockRestore();
  });
  return warn;
};

const label = (wave: Wave) => screen.getByText(`${wave} label`);

describe("RotarySelector", () => {
  it("is a slider over the position indices, named by its label", () => {
    render(<Waves defaultValue="saw" format={(wave) => wave.toUpperCase()} />);
    expect(dial()).toHaveAccessibleName("Waveform");
    expect(dial()).toHaveAttribute("aria-valuenow", "2");
    expect(dial()).toHaveAttribute("aria-valuemin", "0");
    expect(dial()).toHaveAttribute("aria-valuemax", "3");
    expect(dial()).toHaveAttribute("aria-valuetext", "SAW");
    expect(dial()).toHaveAttribute("tabindex", "0");
    expect(dial().querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("SAW")).toHaveAttribute(
      "data-slot",
      "rotary-selector-value"
    );
  });

  it("starts on the first value, and follows a controlled value", () => {
    const { unmount } = render(<Waves />);
    expect(dial()).toHaveAttribute("aria-valuetext", "sine");
    unmount();
    render(<Controlled />);
    expect(valueNow()).toBe(3);
    fireEvent.click(screen.getByRole("button", { name: "Triangle" }));
    expect(valueNow()).toBe(1);
  });

  it("keeps a controlled value until the owner updates it", () => {
    const onValueChange = vi.fn();
    render(<Waves onValueChange={onValueChange} value="sine" />);
    press("ArrowUp");
    expect(onValueChange).toHaveBeenLastCalledWith("triangle", {
      event: expect.any(KeyboardEvent),
      reason: "keyboard",
    });
    expect(valueNow()).toBe(0);
  });

  it("turns clockwise with Up and Right, anticlockwise with Down and Left", () => {
    render(<Waves defaultValue="triangle" />);
    expect(press("ArrowUp").defaultPrevented).toBe(true);
    expect(valueNow()).toBe(2);
    press("ArrowRight");
    expect(valueNow()).toBe(3);
    press("ArrowDown");
    expect(valueNow()).toBe(2);
    press("ArrowLeft");
    expect(valueNow()).toBe(1);
  });

  it("moves to lower indices clockwise when the positions run anticlockwise", () => {
    render(<Waves defaultValue="triangle" stepAngle={-30} />);
    press("ArrowUp");
    expect(valueNow()).toBe(0);
    press("ArrowDown");
    press("ArrowLeft");
    expect(valueNow()).toBe(2);
  });

  it("goes to the first and last values with Home and End, and leaves Page keys alone", () => {
    render(<Waves defaultValue="triangle" stepAngle={-30} />);
    press("End");
    expect(valueNow()).toBe(3);
    press("Home");
    expect(valueNow()).toBe(0);
    const pageUp = press("PageUp");
    expect(pageUp.defaultPrevented).toBe(false);
    expect(valueNow()).toBe(0);
  });

  it("stops at both ends unless the positions fill the whole turn", () => {
    const { unmount } = render(<Waves defaultValue="square" />);
    expect(dial()).not.toHaveAttribute("data-wraps");
    press("ArrowUp");
    expect(valueNow()).toBe(3);
    press("Home");
    press("ArrowDown");
    expect(valueNow()).toBe(0);
    unmount();

    // 4 × 90° is a full turn: past the last value comes the first.
    render(<Waves defaultValue="square" stepAngle={90} />);
    expect(dial()).toHaveAttribute("data-wraps", "");
    press("ArrowUp");
    expect(valueNow()).toBe(0);
    press("ArrowDown");
    expect(valueNow()).toBe(3);
  });

  it("wraps when the steps add up to a turn within half a degree", () => {
    for (const stepAngle of [89.9, 90.1]) {
      const { unmount } = render(<Waves stepAngle={stepAngle} />);
      expect(dial()).toHaveAttribute("data-wraps", "");
      press("ArrowDown");
      expect(valueNow()).toBe(3);
      unmount();
    }
    render(<Waves stepAngle={89} />);
    expect(dial()).not.toHaveAttribute("data-wraps");
    press("ArrowDown");
    expect(valueNow()).toBe(0);
  });

  it("throws in development when the steps pass a turn by more than half a degree", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      quiet.mockRestore();
    });
    expect(() => render(<Waves stepAngle={90.2} />)).toThrow(
      "RotarySelector: 4 values × 90.2° = 360.8°, max 360°."
    );
  });

  it("throws in development when the positions overlap", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      quiet.mockRestore();
    });
    const values: string[] = [...TWELVE, "XX"];
    expect(() =>
      render(
        <RotarySelector values={values}>
          {() => <RotarySelectorDial aria-label="Instrument" />}
        </RotarySelector>
      )
    ).toThrow("RotarySelector: 13 values × 30° = 390°, max 360°.");
  });

  it("throws in development without values or with an unusable step", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      quiet.mockRestore();
    });
    expect(() =>
      render(
        <RotarySelector values={[]}>
          {() => <RotarySelectorDial aria-label="Empty" />}
        </RotarySelector>
      )
    ).toThrow("RotarySelector: values is empty.");
    for (const stepAngle of [0, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => render(<Waves stepAngle={stepAngle} />)).toThrow(
        `RotarySelector: stepAngle must be a finite number other than 0, not ${stepAngle}.`
      );
    }
  });

  it("logs a selector without values in production and still renders", () => {
    vi.stubEnv("NODE_ENV", "production");
    const logged = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      vi.unstubAllEnvs();
      logged.mockRestore();
    });
    render(
      <RotarySelector values={[]}>
        {() => <RotarySelectorDial aria-label="Empty" />}
      </RotarySelector>
    );
    expect(logged).toHaveBeenCalledWith("RotarySelector: values is empty.");
    press("End");
    expect(dial()).toHaveAttribute("aria-valuemax", "-1");
  });

  it("warns in development about a value or reset value missing from values", () => {
    const warn = warnings();
    const values: string[] = ["a", "b"];
    render(
      <RotarySelector resetValue="z" value="y" values={values}>
        {() => <RotarySelectorDial aria-label="Missing" />}
      </RotarySelector>
    );
    expect(warn.mock.calls.map(([message]) => message)).toEqual([
      'RotarySelector: value "y" is not in values.',
      'RotarySelector: resetValue "z" is not in values.',
    ]);
    expect(valueNow()).toBe(0);
  });

  it("logs overlapping positions in production and still renders", () => {
    vi.stubEnv("NODE_ENV", "production");
    const logged = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      vi.unstubAllEnvs();
      logged.mockRestore();
    });
    const nine: string[] = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
    render(
      <RotarySelector stepAngle={-45} values={nine}>
        {() => <RotarySelectorDial aria-label="Nine" />}
      </RotarySelector>
    );
    expect(logged).toHaveBeenCalledWith(
      "RotarySelector: 9 values × 45° = 405°, max 360°."
    );
    expect(dial()).toHaveAttribute("aria-valuemax", "8");
  });

  it("selects a position when its label is clicked, and focuses the dial", () => {
    const onValueChange = vi.fn();
    const onValueCommitted = vi.fn();
    render(
      <Waves
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
      />
    );
    fireEvent.click(label("saw"));
    expect(onValueChange).toHaveBeenLastCalledWith("saw", {
      event: expect.any(MouseEvent),
      reason: "label",
    });
    expect(onValueCommitted).toHaveBeenLastCalledWith("saw");
    expect(valueNow()).toBe(2);
    expect(dial()).toHaveFocus();
    expect(label("saw").closest("g")).toHaveAttribute("aria-hidden", "true");
  });

  it('marks the selected position\'s parts with data-selected="true"', () => {
    const { container } = render(<Waves defaultValue="saw" />);
    const selected = container.querySelectorAll<HTMLElement>("[data-selected]");
    expect([...selected].map((part) => part.dataset.slot)).toEqual([
      "rotary-selector-position-mark",
      "rotary-selector-position-label",
    ]);
    for (const part of selected) {
      expect(part).toHaveAttribute("data-selected", "true");
    }
    expect(label("saw").closest("g")).toBe(selected[1]);
  });

  it("commits at the end of every interaction, even without a change", () => {
    const onValueChange = vi.fn();
    const onValueCommitted = vi.fn();
    render(
      <Waves
        defaultValue="square"
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
      />
    );
    press("ArrowUp");
    expect(onValueChange).not.toHaveBeenCalled();
    expect(onValueCommitted).toHaveBeenLastCalledWith("square");
    fireEvent.click(label("square"));
    expect(onValueCommitted).toHaveBeenCalledTimes(2);

    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, clientY: 100 });
    fireEvent.pointerMove(element, { clientY: 124 });
    fireEvent.pointerMove(element, { clientY: 148 });
    expect(onValueChange).toHaveBeenLastCalledWith("triangle", {
      event: expect.any(PointerEvent),
      reason: "drag",
    });
    expect(onValueCommitted).toHaveBeenCalledTimes(2);
    fireEvent.pointerUp(element);
    expect(onValueCommitted).toHaveBeenCalledTimes(3);
    expect(onValueCommitted).toHaveBeenLastCalledWith("triangle");

    dial().focus();
    fireEvent.wheel(dial(), { deltaY: 100 });
    expect(onValueCommitted).toHaveBeenCalledTimes(4);
    expect(onValueCommitted).toHaveBeenLastCalledWith("sine");
    // Down from the first value goes nowhere, and still commits.
    fireEvent.wheel(dial(), { deltaY: 100 });
    expect(onValueCommitted).toHaveBeenCalledTimes(5);
    expect(onValueCommitted).toHaveBeenLastCalledWith("sine");
    fireEvent.doubleClick(dial());
    expect(onValueCommitted).toHaveBeenCalledTimes(6);
    expect(onValueCommitted).toHaveBeenLastCalledWith("square");
  });

  it("resets on double-click and Alt+click", () => {
    const onValueChange = vi.fn();
    render(
      <Waves
        defaultValue="saw"
        onValueChange={onValueChange}
        resetValue="sine"
      />
    );
    fireEvent.doubleClick(dial());
    expect(onValueChange).toHaveBeenLastCalledWith("sine", { reason: "reset" });
    press("End");
    fireEvent.pointerDown(dial(), { altKey: true, button: 0 });
    expect(onValueChange).toHaveBeenLastCalledWith("sine", { reason: "reset" });
    // Double-clicking a label selects it, and does not reset.
    fireEvent.click(label("square"));
    fireEvent.doubleClick(label("square"));
    expect(valueNow()).toBe(3);
  });

  it("leaves a press on a label to the label, without a drag", () => {
    const onValueChange = vi.fn();
    render(<Waves onValueChange={onValueChange} />);
    const element = draggable();
    fireEvent.pointerDown(label("saw"), { button: 0, clientY: 100 });
    fireEvent.pointerMove(element, { clientY: 40 });
    expect(element).not.toHaveAttribute("data-dragging");
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it("drags with the main button only", () => {
    render(<Waves />);
    const element = draggable();
    fireEvent.pointerDown(element, { button: 2, clientY: 100 });
    fireEvent.pointerMove(element, { clientY: 40 });
    expect(element).not.toHaveAttribute("data-dragging");
    expect(valueNow()).toBe(0);
  });

  it("resets to the default value without a reset value", () => {
    render(<Waves defaultValue="saw" />);
    press("Home");
    fireEvent.doubleClick(dial());
    expect(valueNow()).toBe(2);
  });

  it("steps one position per 24 px of vertical or horizontal drag", () => {
    const { unmount } = render(<Waves />);
    const vertical = draggable();
    fireEvent.pointerDown(vertical, { button: 0, clientY: 100 });
    fireEvent.pointerMove(vertical, { clientY: 77 });
    expect(valueNow()).toBe(0);
    fireEvent.pointerMove(vertical, { clientY: 76 });
    expect(valueNow()).toBe(1);
    // Past the last value it stays there, and turns back as far down.
    fireEvent.pointerMove(vertical, { clientY: -92 });
    expect(valueNow()).toBe(3);
    fireEvent.pointerMove(vertical, { clientY: -68 });
    expect(valueNow()).toBe(2);
    fireEvent.pointerUp(vertical);
    unmount();

    render(<Waves dragDirection="horizontal" stepAngle={-30} />);
    const horizontal = draggable();
    fireEvent.pointerDown(horizontal, { button: 0, clientX: 0 });
    // Right is clockwise, which is a lower index here: it stays on the first.
    fireEvent.pointerMove(horizontal, { clientX: 48 });
    expect(valueNow()).toBe(0);
    fireEvent.pointerMove(horizontal, { clientX: 0 });
    expect(valueNow()).toBe(2);
  });

  it("snaps to the nearest position when circled", () => {
    render(<Waves dragDirection="circular" startAngle={-45} />);
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, ...around(0) });
    fireEvent.pointerMove(element, around(20));
    expect(valueNow()).toBe(2);
    fireEvent.pointerMove(element, around(-40));
    expect(valueNow()).toBe(0);
    // Near the centre the angle is too jumpy to read.
    fireEvent.pointerMove(element, { clientX: 52, clientY: 48 });
    expect(valueNow()).toBe(0);
  });

  it("stays at the end it reached when circled through the gap", () => {
    // -45°, -15°, 15° and 45°: the gap is the bottom 240°.
    render(<Waves dragDirection="circular" startAngle={-45} />);
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, ...around(40) });
    fireEvent.pointerMove(element, around(90));
    expect(valueNow()).toBe(3);
    // Round the bottom to the left side, nearer the first position.
    fireEvent.pointerMove(element, around(180));
    fireEvent.pointerMove(element, around(270));
    expect(valueNow()).toBe(3);
    // Back over the dial, it follows again once it meets that end.
    fireEvent.pointerMove(element, around(-20));
    expect(valueNow()).toBe(3);
    fireEvent.pointerMove(element, around(45));
    fireEvent.pointerMove(element, around(-10));
    expect(valueNow()).toBe(1);
  });

  it("finds the nearest position whatever turn the angles are given in", () => {
    for (const startAngle of [0, 360, -720]) {
      const { unmount } = render(
        <Waves
          dragDirection="circular"
          startAngle={startAngle}
          stepAngle={90}
        />
      );
      const element = draggable();
      fireEvent.pointerDown(element, { button: 0, ...around(0) });
      fireEvent.pointerMove(element, around(-90));
      expect(valueNow()).toBe(3);
      fireEvent.pointerMove(element, around(100));
      expect(valueNow()).toBe(1);
      unmount();
    }
  });

  it("holds the end when the first move crosses the gap", () => {
    // From -150° to 150°: a 30° gap at the bottom.
    const ELEVEN = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
    render(
      <RotarySelector
        defaultValue="k"
        dragDirection="circular"
        startAngle={-150}
        values={ELEVEN}
      >
        {() => <RotarySelectorDial aria-label="Eleven" />}
      </RotarySelector>
    );
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, ...around(150) });
    fireEvent.pointerMove(element, around(-150));
    expect(valueNow()).toBe(10);
  });

  it("holds the end when a fast flick crosses a narrow gap between two moves", () => {
    // From -120° to 120°: a 90° gap at the bottom.
    const NINE = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
    render(
      <RotarySelector dragDirection="circular" startAngle={-120} values={NINE}>
        {() => <RotarySelectorDial aria-label="Nine" />}
      </RotarySelector>
    );
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, ...around(0) });
    fireEvent.pointerMove(element, around(130));
    expect(valueNow()).toBe(8);
    fireEvent.pointerMove(element, around(-130));
    expect(valueNow()).toBe(8);
  });

  it("circles straight across the gap of a selector that wraps", () => {
    render(<Waves dragDirection="circular" stepAngle={90} />);
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, ...around(270) });
    fireEvent.pointerMove(element, around(275));
    expect(valueNow()).toBe(3);
    fireEvent.pointerMove(element, around(350));
    expect(valueNow()).toBe(0);
  });

  it("steps one position per wheel notch while focused", () => {
    const onValueChange = vi.fn();
    render(<Waves onValueChange={onValueChange} stepAngle={-30} />);
    const ignored = createEvent.wheel(dial(), { deltaY: 100 });
    fireEvent(dial(), ignored);
    expect(ignored.defaultPrevented).toBe(false);
    dial().focus();
    // Down is anticlockwise: a higher index when the positions run anticlockwise.
    const wheel = createEvent.wheel(dial(), { deltaY: 100 });
    fireEvent(dial(), wheel);
    expect(wheel.defaultPrevented).toBe(true);
    expect(onValueChange).toHaveBeenLastCalledWith("triangle", {
      event: wheel,
      reason: "wheel",
    });
  });

  it("takes the wheel with a ref of its own on the dial", () => {
    const objectRef = createRef<HTMLDivElement>();
    const callbackRef = vi.fn();
    for (const ref of [objectRef, callbackRef]) {
      const { unmount } = render(<Waves dial={{ ref }} />);
      dial().focus();
      fireEvent.wheel(dial(), { deltaY: -100 });
      expect(valueNow()).toBe(1);
      unmount();
    }
    expect(objectRef.current).toBeNull();
    expect(callbackRef).toHaveBeenCalledWith(expect.any(HTMLDivElement));
    expect(callbackRef).toHaveBeenLastCalledWith(null);
  });

  it("ignores the wheel when allowWheel is false", () => {
    render(<Waves allowWheel={false} />);
    dial().focus();
    fireEvent.wheel(dial(), { deltaY: -100 });
    expect(valueNow()).toBe(0);
  });

  it("is disabled by its prop or by the surrounding config", () => {
    const onValueChange = vi.fn();
    const { unmount } = render(
      <Waves disabled onValueChange={onValueChange} />
    );
    expect(dial()).toHaveAttribute("aria-disabled", "true");
    expect(dial()).toHaveAttribute("tabindex", "-1");
    expect(dial()).toHaveAttribute("data-disabled", "");
    press("ArrowUp");
    fireEvent.click(label("saw"));
    fireEvent.doubleClick(dial());
    const element = draggable();
    fireEvent.pointerDown(element, { button: 0, clientY: 100 });
    fireEvent.pointerMove(element, { clientY: 40 });
    expect(element).not.toHaveAttribute("data-dragging");
    element.focus();
    const wheel = createEvent.wheel(element, { deltaY: -100 });
    fireEvent(element, wheel);
    expect(wheel.defaultPrevented).toBe(false);
    expect(onValueChange).not.toHaveBeenCalled();
    unmount();

    render(
      <AudioConfigProvider value={{ disabled: true }}>
        <Waves />
      </AudioConfigProvider>
    );
    expect(dial()).toHaveAttribute("aria-disabled", "true");
  });

  it("sizes its dial like a Knob", () => {
    const { container } = render(
      <AudioConfigProvider value={{ size: "sm" }}>
        <Waves />
      </AudioConfigProvider>
    );
    expect(container.firstElementChild).toHaveAttribute("data-size", "sm");
    expect(container.firstElementChild).toHaveAttribute(
      "data-slot",
      "rotary-selector"
    );
  });

  it("hugs the cap with its drag circle, unless given a radius", () => {
    const { container, unmount } = render(<Waves />);
    expect(hitArea(container)).toHaveAttribute("r", "25.5");
    unmount();
    const bare = render(
      <RotarySelector values={WAVES}>
        {() => <RotarySelectorDial aria-label="Bare" />}
      </RotarySelector>
    );
    expect(hitArea(bare.container)).toHaveAttribute("r", "50");
    bare.unmount();
    const ringed = render(<Waves dial={{ hitRadius: 44 }} />);
    expect(hitArea(ringed.container)).toHaveAttribute("r", "44");
  });

  it("turns the cap to the selected position", () => {
    const { container } = render(<Waves defaultValue="saw" startAngle={-45} />);
    expect(
      container.querySelector("[data-slot='knob-cap-grain']")
    ).toHaveAttribute("transform", "rotate(15 50 50)");
    expect(dial().style.getPropertyValue("--knob-angle")).toBe("15deg");
  });
});

describe("RotarySelector click sound", () => {
  it("clicks on every position change, whatever its source", () => {
    const sources = clicking({ clickSound: true });
    press("ArrowUp");
    expect(sources).toHaveLength(1);
    fireEvent.click(label("square"));
    expect(sources).toHaveLength(2);
    dial().focus();
    fireEvent.wheel(dial(), { deltaY: 100 });
    expect(sources).toHaveLength(3);
    fireEvent.doubleClick(dial());
    expect(sources).toHaveLength(4);
    // Staying put is silent.
    press("ArrowDown");
    expect(sources).toHaveLength(4);
  });

  it("stays silent without clickSound", () => {
    const sources = clicking();
    press("ArrowUp");
    expect(sources).toHaveLength(0);
  });

  it("clicks at most every 33 ms", () => {
    const sources = clicking({ clickSound: true });
    const now = vi.spyOn(performance, "now").mockReturnValue(1_000_000);
    press("ArrowUp");
    now.mockReturnValue(1_000_032);
    press("ArrowUp");
    expect(sources).toHaveLength(1);
    now.mockReturnValue(1_000_033);
    press("ArrowUp");
    expect(sources).toHaveLength(2);
  });
});

/** One leader on the position at `angle`, drawn in a selector of one value. */
const leaderAt = (
  angle: number,
  props: Omit<ComponentProps<typeof RotarySelectorPositionLeader>, "position">
) =>
  render(
    <RotarySelector startAngle={angle} values={["only"]}>
      {({ positions }) => (
        <RotarySelectorDial aria-label="Leader">
          {positions.map((position) => (
            <RotarySelectorPositionLeader
              key={position.value}
              position={position}
              {...props}
            />
          ))}
        </RotarySelectorDial>
      )}
    </RotarySelector>
  );

const leaderPath = (container: HTMLElement) =>
  container.querySelector("[data-slot='rotary-selector-position-leader']");

describe("RotarySelectorPositionLeader", () => {
  it("runs a ray out from the dial, then across to a column", () => {
    const warn = warnings();
    const { container } = leaderAt(-90, {
      children: <text>label</text>,
      from: 53,
      ray: 7,
      to: { x: -18 },
    });
    expect(leaderPath(container)).toHaveAttribute(
      "d",
      "M -3 50 L -10 50 L -18 50"
    );
    expect(screen.getByText("label").parentElement).toHaveAttribute(
      "transform",
      "translate(-18 50)"
    );
    expect(leaderPath(container)).not.toHaveAttribute("data-invalid");
    expect(warn).not.toHaveBeenCalled();
  });

  it("runs to a row, and hands its geometry to a children function", () => {
    const children = vi.fn((leader: RotarySelectorLeader): ReactNode => (
      <text x={leader.end.x}>label</text>
    ));
    leaderAt(180, { children, from: 52, ray: 8, to: { y: 120 } });
    expect(children).toHaveBeenCalledWith({
      bend: { x: 50, y: 110 },
      end: { x: 50, y: 120 },
      path: "M 50 102 L 50 110 L 50 120",
    });
  });

  it("draws impossible geometry as invalid, with one warning per problem", () => {
    const warn = warnings();
    // At 3 o'clock the ray points right, away from a column on the left, and
    // ends inside the dial.
    const { container, rerender } = leaderAt(90, {
      from: 40,
      ray: 5,
      to: { x: -18 },
    });
    expect(leaderPath(container)).toHaveAttribute("data-invalid", "");
    expect(leaderPath(container)).toHaveAttribute(
      "d",
      "M 90 50 L 95 50 L -18 50"
    );
    expect(warn.mock.calls.map(([message]) => message)).toEqual([
      'RotarySelectorPositionLeader for "only": the bend is inside the dial (radius 45 < 50).',
      'RotarySelectorPositionLeader for "only": the ray points away from its column (x = -18).',
    ]);
    rerender(
      <RotarySelector startAngle={90} values={["only"]}>
        {({ positions }) => (
          <RotarySelectorDial aria-label="Leader">
            {positions.map((position) => (
              <RotarySelectorPositionLeader
                from={40}
                key={position.value}
                position={position}
                ray={5}
                to={{ x: -18 }}
              />
            ))}
          </RotarySelectorDial>
        )}
      </RotarySelector>
    );
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("warns when the ray is too short to read", () => {
    const warn = warnings();
    leaderAt(90, { from: 53, ray: 1, to: { x: 120 } });
    expect(warn).toHaveBeenCalledExactlyOnceWith(
      'RotarySelectorPositionLeader for "only": the ray is too short (1 < 2).'
    );
  });

  it("stays quiet about impossible geometry in production", () => {
    vi.stubEnv("NODE_ENV", "production");
    onTestFinished(() => {
      vi.unstubAllEnvs();
    });
    const warn = warnings();
    const { container } = leaderAt(90, { from: 40, ray: 5, to: { x: -18 } });
    expect(leaderPath(container)).toHaveAttribute("data-invalid", "");
    expect(warn).not.toHaveBeenCalled();
  });

  it("warns when the bend is past its column", () => {
    const warn = warnings();
    // From 9 o'clock the bend lands at x = -10, left of a column at x = 30.
    leaderAt(-90, { from: 53, ray: 7, to: { x: 30 } });
    expect(warn).toHaveBeenCalledExactlyOnceWith(
      'RotarySelectorPositionLeader for "only": the bend is past its column (x = 30), so the run doubles back.'
    );
  });
});

describe("RotarySelector types", () => {
  it("infers the value union from values", () => {
    const element = (
      <RotarySelector
        defaultValue="saw"
        format={(wave) => {
          expectTypeOf(wave).toEqualTypeOf<Wave>();
          return wave;
        }}
        onValueChange={(wave) => expectTypeOf(wave).toEqualTypeOf<Wave>()}
        onValueCommitted={(wave) => expectTypeOf(wave).toEqualTypeOf<Wave>()}
        resetValue="sine"
        values={WAVES}
      >
        {({ position, positions, value }) => {
          expectTypeOf(value).toEqualTypeOf<Wave>();
          expectTypeOf(positions).toEqualTypeOf<
            readonly RotarySelectorPosition<Wave>[]
          >();
          expectTypeOf(position("saw")).toEqualTypeOf<
            RotarySelectorPosition<Wave>
          >();
          // @ts-expect-error: not one of the values.
          position("sawtooth");
          return null;
        }}
      </RotarySelector>
    );
    const wrong = (
      <RotarySelector
        // @ts-expect-error: not one of the values.
        defaultValue="sawtooth"
        values={WAVES}
      >
        {() => null}
      </RotarySelector>
    );
    expect([element, wrong]).toHaveLength(2);
  });

  it("refuses a literal step that overflows a literal tuple", () => {
    const THIRTEEN = [...TWELVE, "XX"] as const;
    const fits = (
      <RotarySelector stepAngle={30} values={TWELVE}>
        {() => null}
      </RotarySelector>
    );
    const overflows = (
      <RotarySelector
        // @ts-expect-error: 13 values × 30° is over 360°.
        stepAngle={30}
        values={THIRTEEN}
      >
        {() => null}
      </RotarySelector>
    );
    const anticlockwise = (
      <RotarySelector
        // @ts-expect-error: 13 values × 30° is over 360°.
        stepAngle={-30}
        values={THIRTEEN}
      >
        {() => null}
      </RotarySelector>
    );
    const runtime: string[] = [...THIRTEEN];
    const unchecked = (
      <RotarySelector stepAngle={30} values={runtime}>
        {() => null}
      </RotarySelector>
    );
    expect([fits, overflows, anticlockwise, unchecked]).toHaveLength(4);
  });
});
