"use client";

import { TempoControl } from "@/components/ui/tempo-control";

const TempoControlStates = () => (
  <div className="flex w-full max-w-xs flex-col gap-4">
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted-foreground text-sm">Small</span>
      <TempoControl aria-label="Small tempo" defaultValue={80} size="sm" />
    </div>
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted-foreground text-sm">Large</span>
      <TempoControl aria-label="Large tempo" defaultValue={128} size="lg" />
    </div>
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted-foreground text-sm">Disabled</span>
      <TempoControl aria-label="Disabled tempo" defaultValue={100} disabled />
    </div>
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted-foreground text-sm">Read-only</span>
      <TempoControl aria-label="Read-only tempo" readOnly value={140} />
    </div>
  </div>
);

export default TempoControlStates;
