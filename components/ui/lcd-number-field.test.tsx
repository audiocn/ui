import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";

import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import {
  LcdNumberField,
  LcdNumberFieldGroup,
  LcdNumberFieldInput,
  LcdNumberFieldReset,
  LcdNumberFieldScrubArea,
  LcdNumberFieldStepper,
  LcdNumberFieldUnit,
} from "@/components/ui/lcd-number-field";

const TempoRow = ({
  onValueChange,
  onValueCommitted,
  withReset = false,
  withUnit = true,
}: {
  onValueChange?: (value: number | null) => void;
  onValueCommitted?: (value: number | null) => void;
  withReset?: boolean;
  withUnit?: boolean;
}) => (
  <LcdDisplay>
    <LcdDisplayRow>
      <LcdNumberField
        defaultValue={120}
        largeStep={10}
        max={130}
        min={60}
        onValueChange={onValueChange}
        onValueCommitted={onValueCommitted}
        step={1}
      >
        <LcdNumberFieldScrubArea>
          <LcdDisplayLabel>Tempo</LcdDisplayLabel>
        </LcdNumberFieldScrubArea>
        <LcdNumberFieldGroup>
          {withReset ? <LcdNumberFieldReset /> : null}
          <LcdNumberFieldInput />
          {withUnit ? <LcdNumberFieldUnit>BPM</LcdNumberFieldUnit> : null}
          <LcdNumberFieldStepper />
        </LcdNumberFieldGroup>
      </LcdNumberField>
    </LcdDisplayRow>
  </LcdDisplay>
);

it("names the input by its row's label and describes it by the unit", () => {
  render(<TempoRow />);
  const input = screen.getByRole("textbox", { name: "Tempo" });
  expect(input).toHaveValue("120");
  expect(input).toHaveAccessibleDescription("BPM");
  for (const slot of [
    "lcd-number-field-scrub-area",
    "lcd-number-field-group",
    "lcd-number-field-input",
    "lcd-number-field-unit",
    "lcd-number-field-stepper",
    "lcd-number-field-increment",
    "lcd-number-field-decrement",
  ]) {
    expect(document.querySelector(`[data-slot='${slot}']`)).not.toBeNull();
  }
});

it("drops the description when the unit goes away", () => {
  const { rerender } = render(<TempoRow />);
  rerender(<TempoRow withUnit={false} />);
  expect(screen.getByRole("textbox", { name: "Tempo" })).not.toHaveAttribute(
    "aria-describedby"
  );
});

it("steps with the arrow keys, Shift for large steps, within the range", () => {
  const onValueChange = vi.fn();
  render(<TempoRow onValueChange={onValueChange} />);
  const input = screen.getByRole("textbox", { name: "Tempo" });
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input).toHaveValue("121");
  fireEvent.keyDown(input, { key: "ArrowUp", shiftKey: true });
  expect(input).toHaveValue("130");
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input).toHaveValue("130");
  expect(onValueChange).toHaveBeenLastCalledWith(130, expect.anything());
});

it("steps with the pixel buttons", async () => {
  const user = userEvent.setup();
  const onValueCommitted = vi.fn();
  render(<TempoRow onValueCommitted={onValueCommitted} />);
  await user.click(screen.getByRole("button", { name: "Decrease" }));
  expect(screen.getByRole("textbox", { name: "Tempo" })).toHaveValue("119");
  await user.click(screen.getByRole("button", { name: "Increase" }));
  expect(screen.getByRole("textbox", { name: "Tempo" })).toHaveValue("120");
  expect(onValueCommitted).toHaveBeenLastCalledWith(120, expect.anything());
});

it("commits a typed value on blur, clamped to the range", async () => {
  const user = userEvent.setup();
  const onValueCommitted = vi.fn();
  render(<TempoRow onValueCommitted={onValueCommitted} />);
  const input = screen.getByRole("textbox", { name: "Tempo" });
  await user.clear(input);
  await user.type(input, "200");
  await user.tab();
  expect(input).toHaveValue("130");
  expect(onValueCommitted).toHaveBeenLastCalledWith(130, expect.anything());
});

const Controlled = () => {
  const [gain, setGain] = useState<number | null>(-3);
  return <LcdNumberField onValueChange={setGain} value={gain} />;
};

it("renders the input and steppers by default, controlled", () => {
  render(<Controlled />);
  const input = screen.getByRole("textbox");
  expect(input).toHaveValue("-3");
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(input).toHaveValue("-4");
  expect(screen.getByRole("button", { name: "Increase" })).toBeInTheDocument();
});

it("resets to defaultValue, like the Parameter Slider and the Fader", async () => {
  const user = userEvent.setup();
  const onValueChange = vi.fn();
  const onValueCommitted = vi.fn();
  render(
    <TempoRow
      onValueChange={onValueChange}
      onValueCommitted={onValueCommitted}
      withReset
    />
  );
  const input = screen.getByRole("textbox", { name: "Tempo" });
  const reset = screen.getByRole("button", { name: "Reset" });
  // Nothing to reset yet, so it is disabled and hidden.
  expect(reset).toBeDisabled();
  fireEvent.keyDown(input, { key: "ArrowUp", shiftKey: true });
  expect(input).toHaveValue("130");
  expect(reset).toBeEnabled();
  expect(reset).toHaveAttribute("data-modified");
  await user.click(reset);
  expect(input).toHaveValue("120");
  expect(onValueChange).toHaveBeenLastCalledWith(
    120,
    expect.objectContaining({ reason: "reset" })
  );
  expect(onValueCommitted).toHaveBeenLastCalledWith(
    120,
    expect.objectContaining({ reason: "reset" })
  );
  expect(reset).toBeDisabled();
});

const ControlledTempo = () => {
  const [tempo, setTempo] = useState<number | null>(140);
  return (
    <LcdNumberField onValueChange={setTempo} resetValue={128} value={tempo}>
      <LcdNumberFieldGroup>
        <LcdNumberFieldReset />
        <LcdNumberFieldInput aria-label="Tempo" />
      </LcdNumberFieldGroup>
    </LcdNumberField>
  );
};

it("resets a controlled field to resetValue", async () => {
  const user = userEvent.setup();
  render(<ControlledTempo />);
  await user.click(screen.getByRole("button", { name: "Reset" }));
  expect(screen.getByRole("textbox", { name: "Tempo" })).toHaveValue("128");
});
