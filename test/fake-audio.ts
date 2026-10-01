import { vi } from "vitest";

/** An AudioParam that records its automation calls. */
export const fakeParam = (value = 1) => ({
  cancelScheduledValues: vi.fn(),
  setTargetAtTime: vi.fn(),
  setValueAtTime: vi.fn(),
  value,
});

const fakeNode = () => ({ connect: vi.fn(), disconnect: vi.fn() });

/**
 * Just enough of an AudioContext for hooks that build graphs: every node
 * records connect/disconnect, and gains are kept in `gains` for inspection.
 */
export const createFakeAudioContext = () => {
  const clock = { now: 0 };
  const gains: ReturnType<typeof fakeGain>[] = [];
  const fakeGain = () => ({ ...fakeNode(), gain: fakeParam() });
  const context = {
    addEventListener: vi.fn(),
    createAnalyser: () => ({
      ...fakeNode(),
      fftSize: 2048,
      frequencyBinCount: 1024,
      getFloatFrequencyData: vi.fn(),
      getFloatTimeDomainData: vi.fn(),
      smoothingTimeConstant: 0,
    }),
    createBufferSource: () => ({
      ...fakeNode(),
      addEventListener: vi.fn(),
      buffer: null,
      loop: false,
      playbackRate: fakeParam(),
      start: vi.fn(),
      stop: vi.fn(),
    }),
    createChannelSplitter: () => fakeNode(),
    createGain: () => {
      const gain = fakeGain();
      gains.push(gain);
      return gain;
    },
    createMediaElementSource: vi.fn(() => fakeNode()),
    createMediaStreamDestination: () => ({
      ...fakeNode(),
      stream: { id: "destination" },
    }),
    createMediaStreamSource: vi.fn(() => fakeNode()),
    get currentTime() {
      return clock.now;
    },
    destination: fakeNode(),
    removeEventListener: vi.fn(),
    resume: vi.fn(() => Promise.resolve()),
    sampleRate: 48_000,
    state: "running",
  };
  return {
    clock,
    context: context as unknown as AudioContext,
    fake: context,
    gains,
  };
};
