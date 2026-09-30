"use client";

import { QuickAudioPopover } from "@/components/blocks/quick-audio-popover/quick-audio-popover";
import { Button } from "@/components/ui/button";

const QuickAudioPopoverDemo = () => (
  <QuickAudioPopover>
    <Button className="w-full" size="sm" variant="ghost">
      More audio settings
    </Button>
  </QuickAudioPopover>
);

export default QuickAudioPopoverDemo;
