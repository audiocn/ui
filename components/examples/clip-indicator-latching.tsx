"use client";

import { useRef } from "react";

import { Button } from "@/components/ui/button";
import { ClipIndicator } from "@/components/ui/clip-indicator";
import type { ClipIndicatorActions } from "@/components/ui/clip-indicator";

const ClipIndicatorLatching = () => {
  const clip = useRef<ClipIndicatorActions>(null);

  return (
    <div className="flex items-center gap-4">
      <ClipIndicator
        actionsRef={clip}
        holdMs={Number.POSITIVE_INFINITY}
        showCount
      />
      <Button
        onClick={() => clip.current?.report(0)}
        size="sm"
        variant="outline"
      >
        Simulate a clip
      </Button>
      <Button onClick={() => clip.current?.reset()} size="sm" variant="ghost">
        Reset
      </Button>
    </div>
  );
};

export default ClipIndicatorLatching;
