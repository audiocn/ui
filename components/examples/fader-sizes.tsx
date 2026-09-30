"use client";

import { Fader } from "@/components/ui/fader";

const sizes = ["sm", "default", "lg"] as const;

const FaderSizes = () => (
  <div className="grid w-full max-w-sm gap-6">
    {sizes.map((size) => (
      <Fader
        aria-label={`${size} fader`}
        defaultValue={-12}
        key={size}
        size={size}
      />
    ))}
    <Fader aria-label="Disabled fader" defaultValue={-24} disabled />
  </div>
);

export default FaderSizes;
