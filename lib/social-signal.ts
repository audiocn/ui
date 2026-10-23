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

const socialFrame: VisualFrame = {
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
};

export const socialVisualSource = createStillSource<VisualFrame>(socialFrame);

const SPECTRUM_BANDS = 48;

/** A fixed small variation per band, so the curve looks measured, not drawn. */
const bandJitter = (index: number) => {
  const value = Math.sin(index * 12.9898 + 4.1) * 43_758.5453;
  return (value - Math.floor(value) - 0.5) * 0.1;
};

const bump = (position: number, centre: number, width: number) =>
  Math.exp(-(((position - centre) / width) ** 2));

/**
 * A still music spectrum on log-spaced bands, for spectrum cards: strong bass
 * with a kick around 120 Hz, a dip in the low mids, presence around 3 kHz and
 * a roll-off to the top.
 */
export const socialSpectrumSource = createStillSource<VisualFrame>({
  ...socialFrame,
  bands: Float32Array.from({ length: SPECTRUM_BANDS }, (_, index) => {
    const position = index / (SPECTRUM_BANDS - 1);
    const level =
      0.74 -
      0.42 * position ** 1.7 +
      0.14 * bump(position, 0.17, 0.07) -
      0.07 * bump(position, 0.42, 0.09) +
      0.07 * bump(position, 0.7, 0.07) +
      bandJitter(index);
    return Math.min(0.95, Math.max(0.06, level));
  }),
});
