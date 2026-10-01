import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, VisualFrame } from "@/lib/audio/types";

export const socialPeaks = Float32Array.from({ length: 320 }, (_, index) =>
  Math.min(
    1,
    0.12 + Math.abs(Math.sin(index * 0.21) * Math.cos(index * 0.047)) * 0.8
  )
);

export const createStillSource = <T>(frame: T): FrameSource<T> => ({
  subscribe: (listener) => subscribeFrame(() => listener(frame)),
});

export const socialVisualSource = createStillSource<VisualFrame>({
  bands: Float32Array.from(
    { length: 64 },
    (_, index) => 0.2 + Math.abs(Math.sin(index * 0.27)) * 0.65
  ),
  history: socialPeaks,
  historyLength: socialPeaks.length,
  historyStart: 0,
  peakDb: -8,
  timeDomain: Float32Array.from(
    { length: 512 },
    (_, index) => Math.sin(index * 0.12) * Math.cos(index * 0.031) * 0.8
  ),
});
