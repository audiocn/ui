import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

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

  it("flickers a value when its text changes", () => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame"] });
    const { rerender } = render(<LcdDisplayValue>Low</LcdDisplayValue>);
    const value = screen.getByText("Low");
    expect(value).not.toHaveAttribute("data-changed");
    rerender(<LcdDisplayValue>High</LcdDisplayValue>);
    expect(value).toHaveAttribute("data-changed");
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(value).not.toHaveAttribute("data-changed");
    vi.useRealTimers();
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
    expect(
      option.closest("[data-slot='lcd-display-select-content']")
    ).toHaveAttribute("data-variant", "amber");
    await user.click(option);
    expect(onValueChange).toHaveBeenLastCalledWith("low", expect.anything());
    expect(screen.getByRole("combobox")).toHaveTextContent("Low");
  });
});
