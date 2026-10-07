import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { describe, expect, it, onTestFinished, vi } from "vitest";

import { AudioDeviceSelect } from "@/components/ui/audio-device-select";
import { MuteToggle } from "@/components/ui/channel-toggle";
import { Fader } from "@/components/ui/fader";
import {
  DialProvider,
  Knob,
  KnobCap,
  KnobDial,
  KnobLabel,
  KnobPointer,
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
import { AudioContextProvider } from "@/hooks/use-audio-context";
import { createClickSound } from "@/lib/audio/click";
import { createFakeAudioContext } from "@/test/fake-audio";

/** The shared AudioContext the knob clicks through, null like on the server. */
const audio = vi.hoisted(() => {
  const shared = {
    context: null as AudioContext | null,
    getSharedAudioContext: vi.fn((): AudioContext | null => shared.context),
  };
  return shared;
});

vi.mock(import("@/hooks/use-audio-context"), async (importOriginal) => ({
  ...(await importOriginal()),
  getSharedAudioContext: audio.getSharedAudioContext,
}));

/** A clock that only moves forward, 100 ms per read, past every click limit. */
const clock = { now: 0 };
const spaceClicksApart = () => {
  vi.spyOn(performance, "now").mockImplementation(() => {
    clock.now += 100;
    return clock.now;
  });
  onTestFinished(() => {
    vi.restoreAllMocks();
  });
};

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

  it("turns with the wheel by default while focused", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = screen.getByRole("slider");
    dial.focus();
    const wheel = createEvent.wheel(dial, { deltaY: -100 });
    fireEvent(dial, wheel);
    expect(onValueChange).toHaveBeenLastCalledWith(51, {
      event: wheel,
      reason: "wheel",
    });
    expect(wheel.defaultPrevented).toBe(true);
  });

  it("leaves the wheel to the page while unfocused", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = screen.getByRole("slider");
    const wheel = createEvent.wheel(dial, { deltaY: -100 });
    fireEvent(dial, wheel);
    expect(onValueChange).not.toHaveBeenCalled();
    expect(wheel.defaultPrevented).toBe(false);
  });

  it("turns by fineStep with Shift+wheel, which some systems turn sideways", () => {
    const onValueChange = vi.fn();
    render(
      <Knob aria-label="Gain" defaultValue={50} onValueChange={onValueChange} />
    );
    const dial = screen.getByRole("slider");
    dial.focus();
    fireEvent.wheel(dial, { deltaX: -100, shiftKey: true });
    expect(onValueChange).toHaveBeenLastCalledWith(50.1, expect.anything());
  });

  it("ignores the wheel when allowWheel is false", () => {
    const onValueChange = vi.fn();
    render(
      <Knob
        allowWheel={false}
        aria-label="Gain"
        defaultValue={50}
        onValueChange={onValueChange}
      />
    );
    const dial = screen.getByRole("slider");
    dial.focus();
    fireEvent.wheel(dial, { deltaY: -100 });
    expect(onValueChange).not.toHaveBeenCalled();
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

  it("turns from where it is grabbed when circled, and stops at its ends", () => {
    const onValueChange = vi.fn();
    render(
      <Knob
        aria-label="Gain"
        defaultValue={50}
        dragDirection="circular"
        onValueChange={onValueChange}
      />
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
      <Knob
        aria-label="Gain"
        defaultValue={50}
        dragDirection="circular"
        onValueChange={onValueChange}
      />
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

  it("marks the mini cap with an engraved line that turns with the value", () => {
    const { container } = render(
      <Knob aria-label="Gain" defaultValue={50}>
        <KnobDial>
          <KnobCap variant="mini" />
        </KnobDial>
      </Knob>
    );
    const cap = container.querySelector("[data-slot='knob-cap']");
    const pointer = () =>
      container.querySelector("[data-slot='knob-cap-pointer']");
    const end = (axis: "x" | "y") =>
      Number(pointer()?.getAttribute(`${axis}2`));
    expect(cap).toHaveAttribute("data-variant", "mini");
    expect(container.querySelector("[data-slot='knob-cap-dot']")).toBeNull();
    // At the middle of the arc the line points straight up.
    expect(end("x")).toBeCloseTo(50);
    expect(end("y")).toBeLessThan(50);
    fireEvent.keyDown(screen.getByRole("slider"), { key: "End" });
    expect(end("x")).toBeGreaterThan(50);
    expect(end("y")).toBeGreaterThan(50);
    fireEvent.keyDown(screen.getByRole("slider"), { key: "Home" });
    expect(end("x")).toBeLessThan(50);
    expect(end("y")).toBeGreaterThan(50);
  });

  it("clicks on graduations only with clickSound", () => {
    const shared = createFakeAudioContext();
    audio.context = shared.context;
    onTestFinished(() => {
      audio.context = null;
    });
    spaceClicksApart();
    const { unmount } = render(<Knob aria-label="Quiet" defaultValue={50} />);
    fireEvent.keyDown(screen.getByRole("slider"), { key: "PageUp" });
    expect(shared.sources).toHaveLength(0);
    unmount();

    // Without a scale it clicks every largeStep: 59 is silent, 60 clicks.
    const { unmount: unmountPlain } = render(
      <Knob aria-label="Volume" clickSound defaultValue={58} />
    );
    const plain = screen.getByRole("slider");
    fireEvent.keyDown(plain, { key: "ArrowUp" });
    expect(shared.sources).toHaveLength(0);
    fireEvent.keyDown(plain, { key: "ArrowUp" });
    expect(shared.sources).toHaveLength(1);
    // Leaving a graduation is silent; reaching the one below clicks.
    fireEvent.keyDown(plain, { key: "ArrowDown" });
    expect(shared.sources).toHaveLength(1);
    fireEvent.keyDown(plain, { key: "PageDown" });
    expect(shared.sources).toHaveLength(2);
    unmountPlain();

    // With a scale it clicks on the long ticks, every 5 here.
    render(
      <Knob aria-label="Volume" clickSound defaultValue={33}>
        <KnobDial>
          <KnobScale majorEvery={5} ticks={100} />
        </KnobDial>
      </Knob>
    );
    const dial = screen.getByRole("slider");
    fireEvent.keyDown(dial, { key: "ArrowUp" });
    expect(shared.sources).toHaveLength(2);
    fireEvent.keyDown(dial, { key: "ArrowUp" });
    expect(shared.sources).toHaveLength(3);
  });

  it("leaves audio alone until it clicks", () => {
    const shared = createFakeAudioContext();
    audio.context = shared.context;
    audio.getSharedAudioContext.mockClear();
    // useAudioContext reaches the real getter, which would build this one.
    vi.stubGlobal(
      "AudioContext",
      class {
        addEventListener = shared.fake.addEventListener;
        removeEventListener = shared.fake.removeEventListener;
        state = "suspended";
      }
    );
    const listen = vi.spyOn(document, "addEventListener");
    spaceClicksApart();
    onTestFinished(() => {
      audio.context = null;
      vi.unstubAllGlobals();
    });
    render(
      <>
        <Knob defaultValue={50}>
          <KnobDial />
          <KnobLabel>Quiet</KnobLabel>
        </Knob>
        <Knob clickSound defaultValue={59}>
          <KnobDial />
          <KnobLabel>Volume</KnobLabel>
        </Knob>
      </>
    );
    expect(audio.getSharedAudioContext).not.toHaveBeenCalled();
    expect(shared.fake.addEventListener).not.toHaveBeenCalled();
    for (const gesture of ["pointerdown", "keydown", "touchend"]) {
      expect(listen).not.toHaveBeenCalledWith(
        gesture,
        expect.anything(),
        expect.anything()
      );
    }

    fireEvent.keyDown(screen.getByRole("slider", { name: "Volume" }), {
      key: "ArrowUp",
    });
    expect(shared.sources).toHaveLength(1);
    expect(audio.getSharedAudioContext).toHaveBeenCalled();
  });

  it("clicks through the AudioContextProvider's context", () => {
    const provided = createFakeAudioContext();
    const shared = createFakeAudioContext();
    audio.context = shared.context;
    onTestFinished(() => {
      audio.context = null;
    });
    spaceClicksApart();
    render(
      <AudioContextProvider context={provided.context}>
        <Knob aria-label="Volume" clickSound defaultValue={59} />
      </AudioContextProvider>
    );
    fireEvent.keyDown(screen.getByRole("slider"), { key: "ArrowUp" });
    expect(provided.sources).toHaveLength(1);
    expect(provided.sources[0]?.start).toHaveBeenCalledOnce();
    expect(provided.gains[0]?.connect).toHaveBeenCalledWith(
      provided.fake.destination
    );
    expect(shared.sources).toHaveLength(0);
  });

  it("plays a custom click sound in place of its own", () => {
    const provided = createFakeAudioContext();
    const thud = createClickSound({ tone: { hz: 460 } });
    thud.play({ context: provided.context, when: 0 });
    render(
      <AudioContextProvider context={provided.context}>
        <Knob aria-label="Volume" clickSound={thud} defaultValue={59} />
      </AudioContextProvider>
    );
    fireEvent.keyDown(screen.getByRole("slider"), { key: "ArrowUp" });
    expect(provided.sources).toHaveLength(2);
    expect(provided.sources[1]?.buffer).toBe(provided.sources[0]?.buffer);
  });

  it("clicks at most every 30 ms", () => {
    const provided = createFakeAudioContext();
    const now = vi.spyOn(performance, "now").mockReturnValue(0);
    onTestFinished(() => {
      now.mockRestore();
    });
    render(
      <AudioContextProvider context={provided.context}>
        <Knob
          aria-label="Volume"
          clickSound={createClickSound()}
          defaultValue={50}
        />
      </AudioContextProvider>
    );
    const dial = screen.getByRole("slider");
    fireEvent.keyDown(dial, { key: "PageUp" });
    now.mockReturnValue(10);
    fireEvent.keyDown(dial, { key: "PageUp" });
    expect(provided.sources).toHaveLength(1);
    now.mockReturnValue(40);
    fireEvent.keyDown(dial, { key: "PageUp" });
    expect(provided.sources).toHaveLength(2);
  });

  it("reads typed values", () => {
    expect(parseKnobValue("\u221212 dB")).toBe(-12);
    expect(parseKnobValue("1.2k")).toBe(1200);
    expect(parseKnobValue("gain")).toBeNull();
  });

  it("keeps the knob's drag area on the whole dial under a cap", () => {
    const { container } = render(
      <Knob aria-label="Volume" defaultValue={50}>
        <KnobDial>
          <KnobCap />
        </KnobDial>
      </Knob>
    );
    expect(
      container.querySelector("[data-slot='knob-hit-area']")
    ).toHaveAttribute("r", "50");
  });
});

/** A dial of its own, outside any Knob, that keeps the cap's radius in state. */
const HuggingDial = ({
  children,
  onRender,
}: {
  children: ReactNode;
  onRender: () => void;
}) => {
  const [geometry, setGeometry] = useState<{ radius: number | null }>({
    radius: null,
  });
  onRender();
  return (
    <svg data-cap-radius={geometry.radius ?? undefined}>
      <DialProvider
        angle={0}
        onCapRadiusChange={(radius) => setGeometry({ radius })}
      >
        {children}
      </DialProvider>
    </svg>
  );
};

describe("DialProvider", () => {
  it("turns the cap and pointer to the angle of any dial", () => {
    const { container, rerender } = render(
      <svg>
        <DialProvider angle={90}>
          <KnobCap />
          <KnobPointer />
        </DialProvider>
      </svg>
    );
    const part = (slot: string) =>
      container.querySelector(`[data-slot='${slot}']`);
    const coordinate = (slot: string, name: string) =>
      Number(part(slot)?.getAttribute(name));
    expect(part("knob-cap-grain")).toHaveAttribute(
      "transform",
      "rotate(90 50 50)"
    );
    // At 90 degrees both marks point straight right.
    expect(coordinate("knob-cap-dot", "cx")).toBeGreaterThan(50);
    expect(coordinate("knob-cap-dot", "cy")).toBeCloseTo(50);
    expect(coordinate("knob-pointer", "x2")).toBeGreaterThan(50);
    expect(coordinate("knob-pointer", "y2")).toBeCloseTo(50);
    rerender(
      <svg>
        <DialProvider angle={-90}>
          <KnobCap />
          <KnobPointer />
        </DialProvider>
      </svg>
    );
    expect(part("knob-cap-grain")).toHaveAttribute(
      "transform",
      "rotate(-90 50 50)"
    );
    expect(coordinate("knob-cap-dot", "cx")).toBeLessThan(50);
    expect(coordinate("knob-pointer", "x2")).toBeLessThan(50);
  });

  it("reports the radius of the bezel each cap variant draws", () => {
    const onCapRadiusChange = vi.fn();
    const { container, rerender } = render(
      <svg>
        <DialProvider angle={0} onCapRadiusChange={onCapRadiusChange}>
          <KnobCap />
        </DialProvider>
      </svg>
    );
    const bezelRadius = () =>
      Number(
        container
          .querySelector("[data-slot='knob-cap'] circle[fill$='-bezel)']")
          ?.getAttribute("r")
      );
    expect(onCapRadiusChange).toHaveBeenLastCalledWith(bezelRadius());
    expect(onCapRadiusChange).toHaveBeenLastCalledWith(25.5);
    rerender(
      <svg>
        <DialProvider angle={0} onCapRadiusChange={onCapRadiusChange}>
          <KnobCap variant="mini" />
        </DialProvider>
      </svg>
    );
    expect(onCapRadiusChange).toHaveBeenLastCalledWith(bezelRadius());
    expect(onCapRadiusChange).toHaveBeenLastCalledWith(32);
  });

  it("clears the reported radius when the cap goes away", () => {
    const onCapRadiusChange = vi.fn();
    const { rerender } = render(
      <svg>
        <DialProvider angle={0} onCapRadiusChange={onCapRadiusChange}>
          <KnobCap />
        </DialProvider>
      </svg>
    );
    rerender(
      <svg>
        <DialProvider angle={0} onCapRadiusChange={onCapRadiusChange}>
          <KnobPointer />
        </DialProvider>
      </svg>
    );
    expect(onCapRadiusChange).toHaveBeenCalledTimes(2);
    expect(onCapRadiusChange).toHaveBeenLastCalledWith(null);
  });

  it("reports once while the dial turns, whatever its callback", () => {
    const report = vi.fn();
    const { rerender } = render(
      <svg>
        <DialProvider angle={0} onCapRadiusChange={(radius) => report(radius)}>
          <KnobCap />
        </DialProvider>
      </svg>
    );
    rerender(
      <svg>
        <DialProvider angle={90} onCapRadiusChange={(radius) => report(radius)}>
          <KnobCap />
        </DialProvider>
      </svg>
    );
    expect(report).toHaveBeenCalledTimes(1);
  });

  it("lets a dial keep the reported radius in state", () => {
    const onRender = vi.fn();
    const { container } = render(
      <HuggingDial onRender={onRender}>
        <KnobCap />
      </HuggingDial>
    );
    expect(container.querySelector("svg")).toHaveAttribute(
      "data-cap-radius",
      "25.5"
    );
    expect(onRender.mock.calls.length).toBeLessThanOrEqual(2);
  });

  it("refuses a cap or pointer outside any dial", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => null);
    onTestFinished(() => {
      quiet.mockRestore();
    });
    expect(() =>
      render(
        <svg>
          <KnobCap />
        </svg>
      )
    ).toThrow("KnobCap must be used inside a dial, like KnobDial.");
    expect(() =>
      render(
        <svg>
          <KnobPointer />
        </svg>
      )
    ).toThrow("KnobPointer must be used inside a dial, like KnobDial.");
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
