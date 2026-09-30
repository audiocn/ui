"use client";

import { useState } from "react";

import { formatPan, PanControl } from "@/components/ui/pan-control";

const PanControlDemo = () => {
  const [pan, setPan] = useState(-0.3);

  return (
    <div className="grid w-full max-w-xs gap-2">
      <div className="flex justify-between text-sm">
        <span className="font-medium">Pan</span>
        <span className="text-muted-foreground font-mono text-xs">
          {formatPan(pan)}
        </span>
      </div>
      <PanControl onValueChange={setPan} value={pan} />
      <div className="text-muted-foreground flex justify-between text-xs">
        <span>L</span>
        <span>R</span>
      </div>
    </div>
  );
};

export default PanControlDemo;
