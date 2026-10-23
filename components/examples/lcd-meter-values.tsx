import {
  LcdDisplay,
  LcdDisplayLabel,
  LcdDisplayRow,
} from "@/components/ui/lcd-display";
import { LcdMeter } from "@/components/ui/lcd-meter";

const levels = [
  { label: "Quiet", peakDb: -42 },
  { label: "Good", peakDb: -16 },
  { label: "Hot", peakDb: -7 },
  { label: "Clipping", peakDb: 0 },
];

const LcdMeterValues = () => (
  <LcdDisplay className="w-full max-w-md">
    {levels.map((level) => (
      <LcdDisplayRow key={level.label}>
        {/* One width for every label, so the meters line up. */}
        <LcdDisplayLabel className="w-[6em] shrink-0">
          {level.label}
        </LcdDisplayLabel>
        <LcdMeter peakDb={level.peakDb} />
      </LcdDisplayRow>
    ))}
  </LcdDisplay>
);

export default LcdMeterValues;
