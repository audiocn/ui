"use client";

import { Waveform } from "@/components/ui/waveform";

const peaks = Float32Array.from({ length: 240 }, (_, index) => {
  const envelope = Math.sin((index / 240) * Math.PI) ** 0.6;
  const detail = 0.55 + 0.45 * Math.abs(Math.sin(index * 0.37) * Math.cos(index * 0.11));
  return envelope * detail;
});

const variants = ["bars", "mirror", "line"] as const;

const WaveformVariants = () => (
  <div className="grid w-full max-w-lg gap-5">
    {variants.map((variant) => (
      <div className="grid gap-1.5" key={variant}>
        <span className="font-mono text-muted-foreground text-xs">{variant}</span>
        <Waveform
          aria-label={`${variant} waveform`}
          className="h-14"
          defaultCurrentTime={12}
          duration={30}
          peaks={peaks}
          variant={variant}
        />
      </div>
    ))}
  </div>
);

export default WaveformVariants;
