import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import type { CSSProperties } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySelect,
  LcdDisplaySelectContent,
  LcdDisplaySelectItem,
  LcdDisplaySelectTrigger,
  LcdDisplaySeparator,
  LcdDisplayStatus,
  LcdDisplayValue,
  useLcdDisplay,
} from "@/components/ui/lcd-display";

const EFFORTS = [
  { label: "Low", value: "low" },
  { label: "High", value: "high" },
];

const EffortRow = ({
  onValueChange,
}: {
  onValueChange?: (value: string | null) => void;
}) => (
  <LcdDisplay>
    <LcdDisplayRow>
      <LcdDisplayLabel>Effort</LcdDisplayLabel>
      <LcdDisplaySelect
        defaultValue="high"
        items={EFFORTS}
        onValueChange={onValueChange}
      >
        <LcdDisplaySelectTrigger />
        <LcdDisplaySelectContent>
          {EFFORTS.map((item) => (
            <LcdDisplaySelectItem key={item.value} value={item.value}>
              {item.label}
            </LcdDisplaySelectItem>
          ))}
        </LcdDisplaySelectContent>
      </LcdDisplaySelect>
    </LcdDisplayRow>
  </LcdDisplay>
);

// A failed test mustn't leave its fake animation frames to the next one.
afterEach(() => {
  vi.useRealTimers();
});

describe("LcdDisplay", () => {
  it("marks every part with a slot and merges className last", () => {
    const { container } = render(
      <LcdDisplay className="text-2xl">
        <LcdDisplayRow className="py-1">
          <LcdDisplayLabel className="opacity-80">Tempo</LcdDisplayLabel>
          <LcdDisplayValue className="text-white">128</LcdDisplayValue>
        </LcdDisplayRow>
        <LcdDisplaySeparator />
        <LcdDisplayStatus />
      </LcdDisplay>
    );
    for (const slot of [
      "lcd-display",
      "lcd-display-screen",
      "lcd-display-content",
      "lcd-display-row",
      "lcd-display-label",
      "lcd-display-value",
      "lcd-display-separator",
      "lcd-display-status",
    ]) {
      expect(container.querySelector(`[data-slot="${slot}"]`)).not.toBeNull();
    }
    const root = container.querySelector("[data-slot='lcd-display']");
    expect(root).toHaveClass("text-2xl");
    expect(root).not.toHaveClass("text-lg");
    expect(screen.getByText("128")).toHaveClass("text-white");
    expect(screen.getByRole("group")).toBe(root);
    expect(screen.getByRole("separator")).toBeInTheDocument();
  });

  it("sets the ink from variant and the text from size", () => {
    const { container } = render(<LcdDisplay size="lg" variant="green" />);
    const root = container.querySelector("[data-slot='lcd-display']");
    expect(root).toHaveAttribute("data-variant", "green");
    expect(root).toHaveAttribute("data-size", "lg");
    expect(root).toHaveClass("text-2xl");
    expect(root?.className).toContain("--lcd-display-ink:oklch(0.85_0.19_142)");
  });

  it("follows the theme unless an appearance is pinned", () => {
    const { container, rerender } = render(<LcdDisplay />);
    const root = () => container.querySelector("[data-slot='lcd-display']");
    // Backlit with dark pixels by default, and the glowing set under `.dark`.
    expect(root()).toHaveAttribute("data-appearance", "auto");
    expect(root()?.className).toContain(
      "[--lcd-display-lit:color-mix(in_oklab,var(--lcd-display-ink),black_62%)]"
    );
    expect(root()?.className).toContain(
      "dark:[--lcd-display-lit:var(--lcd-display-ink)]"
    );
    rerender(<LcdDisplay appearance="dark" />);
    expect(root()).toHaveAttribute("data-appearance", "dark");
    expect(root()?.className).toContain(
      "[--lcd-display-lit:var(--lcd-display-ink)]"
    );
    expect(root()?.className).not.toContain("dark:");
    expect(root()?.className).not.toContain("black_62%");
    rerender(<LcdDisplay appearance="light" />);
    expect(root()?.className).toContain("black_62%");
    expect(root()?.className).not.toContain("dark:");
  });

  it("takes the ink from the theme's primary colour by default", () => {
    const { container } = render(<LcdDisplay />);
    const root = container.querySelector("[data-slot='lcd-display']");
    expect(root).toHaveAttribute("data-variant", "primary");
    expect(root?.className).toContain("[--lcd-display-ink:var(--primary)]");
  });

  it("drops the dot mask and scanlines when asked", () => {
    const { container } = render(
      <LcdDisplay matrix={false} scanlines={false} />
    );
    const root = container.querySelector("[data-slot='lcd-display']");
    expect(root).not.toHaveAttribute("data-matrix");
    const content = container.querySelector(
      "[data-slot='lcd-display-content']"
    );
    expect(content?.className).not.toContain("mask-");
    const glass = container.querySelector("[data-slot='lcd-display-screen']");
    expect(glass?.className).not.toContain("repeating-linear-gradient");
  });

  it("announces a value only when live", () => {
    render(
      <>
        <LcdDisplayValue>Quiet</LcdDisplayValue>
        <LcdDisplayValue live>Loud</LcdDisplayValue>
      </>
    );
    expect(screen.getByText("Quiet")).not.toHaveAttribute("aria-live");
    expect(screen.getByText("Loud")).toHaveAttribute("aria-live", "polite");
  });

  it("flickers a value when its text changes", async () => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame"] });
    const { rerender } = render(<LcdDisplayValue>Low</LcdDisplayValue>);
    const value = screen.getByText("Low");
    expect(value).not.toHaveAttribute("data-changed");
    // The flicker watches the DOM, and mutation records arrive as a microtask.
    await act(async () => {
      rerender(<LcdDisplayValue>High</LcdDisplayValue>);
      await Promise.resolve();
    });
    expect(value).toHaveAttribute("data-changed");
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(value).not.toHaveAttribute("data-changed");
  });

  it("keeps a caller's ref beside its own", async () => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame"] });
    const ref = createRef<HTMLSpanElement>();
    const { rerender } = render(
      <LcdDisplayValue ref={ref}>Low</LcdDisplayValue>
    );
    expect(ref.current).toBe(screen.getByText("Low"));
    await act(async () => {
      rerender(<LcdDisplayValue ref={ref}>High</LcdDisplayValue>);
      await Promise.resolve();
    });
    expect(ref.current).toHaveAttribute("data-changed");
  });

  it("blinks the status pixel by state", () => {
    const { container } = render(<LcdDisplayStatus state="busy" />);
    const status = container.querySelector("[data-slot='lcd-display-status']");
    expect(status).toHaveAttribute("data-state", "busy");
    expect(status).toHaveAttribute("aria-hidden");
  });
});

describe("LcdDisplaySelect", () => {
  it("names the trigger by its row's label and shows the item label", () => {
    render(<EffortRow />);
    const trigger = screen.getByRole("combobox");
    expect(trigger).toHaveAccessibleName("Effort");
    expect(trigger).toHaveTextContent("High");
  });

  it("picks a choice from the popup", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    render(<EffortRow onValueChange={onValueChange} />);
    await user.click(screen.getByRole("combobox"));
    const option = await screen.findByRole("option", { name: "Low" });
    const popup = option.closest("[data-slot='lcd-display-select-content']");
    expect(popup).toHaveAttribute("data-variant", "primary");
    // The popup is portalled, so it carries the display's look itself.
    expect(popup).toHaveAttribute("data-appearance", "auto");
    await user.click(option);
    expect(onValueChange).toHaveBeenLastCalledWith("low", expect.anything());
    expect(screen.getByRole("combobox")).toHaveTextContent("Low");
  });

  it("flickers the trigger when the choice changes", async () => {
    const user = userEvent.setup();
    render(<EffortRow />);
    const trigger = screen.getByRole("combobox");
    await user.click(trigger);
    await user.click(await screen.findByRole("option", { name: "Low" }));
    // Base UI redraws the value on its own, outside the trigger's render.
    expect(trigger).toHaveAttribute("data-changed");
  });

  it("names the trigger by a label with its own id", () => {
    render(
      <LcdDisplay>
        <LcdDisplayRow>
          <LcdDisplayLabel id="effort-label">Effort</LcdDisplayLabel>
          <LcdDisplaySelect defaultValue="high" items={EFFORTS}>
            <LcdDisplaySelectTrigger />
          </LcdDisplaySelect>
        </LcdDisplayRow>
      </LcdDisplay>
    );
    const trigger = screen.getByRole("combobox");
    expect(trigger).toHaveAttribute("aria-labelledby", "effort-label");
    expect(trigger).toHaveAccessibleName("Effort");
  });

  it("tells parts on its screen its settings, and nothing outside one", () => {
    const seen: (string | null)[] = [];
    const Probe = () => {
      const display = useLcdDisplay();
      seen.push(display ? `${display.size} ${display.matrix}` : null);
      return null;
    };
    render(
      <>
        <Probe />
        <LcdDisplay matrix={false} size="lg">
          <Probe />
        </LcdDisplay>
      </>
    );
    expect(seen).toEqual([null, "lg false"]);
  });

  it("keeps a select's popup on the defaults outside a display", async () => {
    const user = userEvent.setup();
    render(
      <LcdDisplaySelect defaultValue="high" items={EFFORTS}>
        <LcdDisplaySelectTrigger aria-label="Effort" />
        <LcdDisplaySelectContent>
          {EFFORTS.map((item) => (
            <LcdDisplaySelectItem key={item.value} value={item.value}>
              {item.label}
            </LcdDisplaySelectItem>
          ))}
        </LcdDisplaySelectContent>
      </LcdDisplaySelect>
    );
    await user.click(screen.getByRole("combobox"));
    const option = await screen.findByRole("option", { name: "Low" });
    const popup = option.closest("[data-slot='lcd-display-select-content']");
    expect(popup).toHaveAttribute("data-appearance", "auto");
    expect(popup).toHaveAttribute("data-variant", "primary");
  });

  it("carries a custom ink into the portalled popup", async () => {
    const user = userEvent.setup();
    render(
      <LcdDisplay
        style={{ "--lcd-display-ink": "var(--chart-1)" } as CSSProperties}
      >
        <LcdDisplayRow>
          <LcdDisplayLabel>Effort</LcdDisplayLabel>
          <LcdDisplaySelect defaultValue="high" items={EFFORTS}>
            <LcdDisplaySelectTrigger />
            <LcdDisplaySelectContent>
              {EFFORTS.map((item) => (
                <LcdDisplaySelectItem key={item.value} value={item.value}>
                  {item.label}
                </LcdDisplaySelectItem>
              ))}
            </LcdDisplaySelectContent>
          </LcdDisplaySelect>
        </LcdDisplayRow>
      </LcdDisplay>
    );
    await user.click(screen.getByRole("combobox"));
    const option = await screen.findByRole("option", { name: "Low" });
    const popup = option.closest<HTMLElement>(
      "[data-slot='lcd-display-select-content']"
    );
    expect(popup?.style.getPropertyValue("--lcd-display-ink")).toBe(
      "var(--chart-1)"
    );
  });
});
