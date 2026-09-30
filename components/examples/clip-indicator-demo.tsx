"use client";

import { ClipIndicator } from "@/components/ui/clip-indicator";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const ClipIndicatorDemo = () => {
  const signal = useDemoSignal({ kind: "music", seed: 11 });

  return (
    <div className="flex items-center gap-6">
      <ClipIndicator source={signal.meter} />
      <ClipIndicator showCount source={signal.meter} />
      <ClipIndicator source={signal.meter}>Clip</ClipIndicator>
    </div>
  );
};

export default ClipIndicatorDemo;
