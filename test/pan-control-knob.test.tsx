import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import PanControlKnob from "@/components/examples/pan-control-knob";
import { AudioConfigProvider } from "@/hooks/use-audio-config";

describe("PanControlKnob", () => {
  it("names the dial and describes its range and centre", () => {
    render(<PanControlKnob />);
    const dial = screen.getByRole("slider", { name: "Pan" });
    expect(dial).toHaveAttribute("tabindex", "0");
    expect(dial).toHaveAttribute("aria-valuemin", "-1");
    expect(dial).toHaveAttribute("aria-valuemax", "1");
    expect(dial).toHaveAttribute("aria-valuenow", "0");
    expect(dial).toHaveAttribute("aria-valuetext", "Center");
    expect(screen.getByText("C")).toBeInTheDocument();
  });

  it("describes keyboard changes in words and resets to centre", () => {
    render(<PanControlKnob />);
    const dial = screen.getByRole("slider", { name: "Pan" });
    fireEvent.keyDown(dial, { key: "ArrowLeft" });
    expect(dial).toHaveAttribute("aria-valuetext", "5% left");
    fireEvent.keyDown(dial, { key: "PageDown" });
    expect(dial).toHaveAttribute("aria-valuetext", "30% left");
    expect(screen.getByText("L30")).toBeInTheDocument();
    fireEvent.keyDown(dial, { key: "PageUp" });
    expect(dial).toHaveAttribute("aria-valuetext", "5% left");
    fireEvent.keyDown(dial, { key: "ArrowRight", shiftKey: true });
    expect(dial).toHaveAttribute("aria-valuetext", "20% right");
    fireEvent.keyDown(dial, { key: "Home" });
    expect(dial).toHaveAttribute("aria-valuenow", "-1");
    expect(dial).toHaveAttribute("aria-valuetext", "100% left");
    fireEvent.keyDown(dial, { key: "End" });
    expect(dial).toHaveAttribute("aria-valuenow", "1");
    expect(dial).toHaveAttribute("aria-valuetext", "100% right");
    fireEvent.doubleClick(dial);
    expect(dial).toHaveAttribute("aria-valuetext", "Center");
  });

  it("supports value entry by keyboard and restores focus", () => {
    render(<PanControlKnob />);
    const dial = screen.getByRole("slider", { name: "Pan" });
    fireEvent.keyDown(dial, { key: "Enter" });
    const input = screen.getByRole("textbox", { name: "Value" });
    expect(input).toHaveFocus();
    fireEvent.change(input, { target: { value: "L30" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(dial).toHaveAttribute("aria-valuenow", "-0.3");
    expect(dial).toHaveAttribute("aria-valuetext", "30% left");
    expect(dial).toHaveFocus();

    fireEvent.keyDown(dial, { key: "Enter" });
    const editor = screen.getByRole("textbox", { name: "Value" });
    fireEvent.change(editor, { target: { value: "R30" } });
    fireEvent.keyDown(editor, { key: "Escape" });
    expect(dial).toHaveAttribute("aria-valuenow", "-0.3");
    expect(dial).toHaveFocus();
  });

  it("removes the disabled dial from the tab order and blocks input", () => {
    render(
      <AudioConfigProvider value={{ disabled: true }}>
        <PanControlKnob />
      </AudioConfigProvider>
    );
    const dial = screen.getByRole("slider", { name: "Pan" });
    expect(dial).toHaveAttribute("aria-disabled", "true");
    expect(dial).toHaveAttribute("tabindex", "-1");
    fireEvent.keyDown(dial, { key: "ArrowRight" });
    fireEvent.keyDown(dial, { key: "Enter" });
    expect(dial).toHaveAttribute("aria-valuenow", "0");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
