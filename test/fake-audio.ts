import { vi } from "vitest";

/** An AudioParam that records its automation calls. */
export const fakeParam = (value = 1) => ({
  cancelScheduledValues: vi.fn(),
  setTargetAtTime: vi.fn(),
  setValueAtTime: vi.fn(),
  value,
});

const fakeNode = () => ({ connect: vi.fn(), disconnect: vi.fn() });

const fakeBuffer = (length: number, sampleRate: number) => {
  const samples = new Float32Array(length);
  return { getChannelData: (_channel: number) => samples, length, sampleRate };
};

const fakeBufferSource = () => ({
  ...fakeNode(),
  addEventListener: vi.fn(),
  buffer: null as ReturnType<typeof fakeBuffer> | null,
  loop: false,
  playbackRate: fakeParam(),
  start: vi.fn(),
  stop: vi.fn(),
});

/**
 * Just enough of an AudioContext for hooks that build graphs: every node
 * records connect/disconnect, and gains and buffer sources are kept in
 * `gains` and `sources` for inspection.
 */
export const createFakeAudioContext = () => {
  const clock = { now: 0 };
  const gains: ReturnType<typeof fakeGain>[] = [];
  const sources: ReturnType<typeof fakeBufferSource>[] = [];
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
    createBuffer: vi.fn(
      (_channels: number, length: number, sampleRate: number) =>
        fakeBuffer(length, sampleRate)
    ),
    createBufferSource: () => {
      const source = fakeBufferSource();
      sources.push(source);
      return source;
    },
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
    state: "running" as AudioContextState,
  };
  return {
    clock,
    context: context as unknown as AudioContext,
    fake: context,
    gains,
    sources,
  };
};
