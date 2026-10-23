"use client";

import { LcdDisplay } from "@/components/ui/lcd-display";
import { LcdSpectrum } from "@/components/ui/lcd-spectrum";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const LcdSpectrumDemo = () => {
  const signal = useDemoSignal({ bands: 32, kind: "music" });
  return (
    <LcdDisplay className="w-full max-w-xl" matrix={false}>
      <LcdSpectrum
        className="h-44"
        maxDb={0}
        minDb={-60}
        source={signal.visual}
      />
    </LcdDisplay>
  );
};

export default LcdSpectrumDemo;
