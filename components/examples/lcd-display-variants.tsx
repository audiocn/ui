import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplayValue,
} from "@/components/ui/lcd-display";

const READINGS = [
  { label: "Tempo", value: "128.0", variant: "amber" },
  { label: "Input", value: "-12 dB", variant: "green" },
  { label: "Sync", value: "MIDI", variant: "cyan" },
  { label: "Rec", value: "00:42", variant: "red" },
] as const;

const LcdDisplayVariants = () => (
  <div className="grid w-full max-w-xl gap-4 sm:grid-cols-2">
    {READINGS.map((reading) => (
      <LcdDisplay key={reading.variant} variant={reading.variant}>
        <LcdDisplayRow>
          <LcdDisplayLabel>{reading.label}</LcdDisplayLabel>
          <LcdDisplayValue>{reading.value}</LcdDisplayValue>
        </LcdDisplayRow>
      </LcdDisplay>
    ))}
  </div>
);

export default LcdDisplayVariants;
