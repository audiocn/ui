import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
  LcdDisplayStatus,
  LcdDisplayValue,
} from "@/components/ui/lcd-display";

const LcdDisplayTheme = () => (
  <LcdDisplay className="w-full max-w-xs" variant="primary">
    <LcdDisplayRow>
      <LcdDisplayLabel>Track</LcdDisplayLabel>
      <LcdDisplayValue>03</LcdDisplayValue>
    </LcdDisplayRow>
    <LcdDisplaySeparator />
    <LcdDisplayRow>
      <LcdDisplayLabel>Gain</LcdDisplayLabel>
      <LcdDisplayValue>-6.0 dB</LcdDisplayValue>
    </LcdDisplayRow>
    <LcdDisplayStatus />
  </LcdDisplay>
);

export default LcdDisplayTheme;
