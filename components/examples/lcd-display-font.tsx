import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
  LcdDisplaySeparator,
  LcdDisplayStatus,
  LcdDisplayValue,
} from "@/components/ui/lcd-display";

/** Doto is a dot-matrix face, so the screen's own dot mask is turned off. */
const LcdDisplayFont = () => (
  <LcdDisplay
    className="w-full max-w-80 [--lcd-display-font:var(--font-doto)]"
    matrix={false}
  >
    <LcdDisplayRow>
      <LcdDisplayLabel>Model</LcdDisplayLabel>
      <LcdDisplayValue>Opus 5.5</LcdDisplayValue>
    </LcdDisplayRow>
    <LcdDisplaySeparator />
    <LcdDisplayRow>
      <LcdDisplayLabel>Effort</LcdDisplayLabel>
      <LcdDisplayValue>High</LcdDisplayValue>
    </LcdDisplayRow>
    <LcdDisplayStatus state="busy" />
  </LcdDisplay>
);

export default LcdDisplayFont;
