import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createClickSound } from "@/lib/audio/click";
import { createFakeAudioContext } from "@/test/fake-audio";

const shared = vi.hoisted(() => ({ context: null as AudioContext | null }));

vi.mock(import("@/hooks/use-audio-context"), async (importOriginal) => ({
  ...(await importOriginal()),
  getSharedAudioContext: () => shared.context,
}));

const clock = { now: 0 };

/** A suspended context whose resume never settles, as without a gesture. */
const suspendedContext = () => {
  const audio = createFakeAudioContext();
  audio.fake.state = "suspended";
  audio.fake.resume.mockReturnValue(Promise.withResolvers<undefined>().promise);
  return audio;
};

beforeEach(() => {
  clock.now = 0;
  vi.spyOn(performance, "now").mockImplementation(() => clock.now);
});

afterEach(() => {
  shared.context = null;
  vi.restoreAllMocks();
});

describe("createClickSound", () => {
  it("merges changes deeply over the defaults", () => {
    const defaults = createClickSound().params;
    const sound = createClickSound({ tone: { hz: 900 }, volume: 0.5 });
    expect(sound.params).toEqual({
      ...defaults,
      tone: { ...defaults.tone, hz: 900 },
      volume: 0.5,
    });
  });

  it("derives a sound with changes merged over the base, not the defaults", () => {
    const thud = createClickSound({
      noise: { gain: 0.21 },
      tone: { gain: 1, hz: 460 },
      volume: 0.18,
    });
    const brighter = thud.with({ tone: { hz: 900 } });
    expect(brighter.params).toEqual({
      ...thud.params,
      tone: { ...thud.params.tone, hz: 900 },
    });
    expect(thud.params.tone.hz).toBe(460);
  });

  it("keeps the base value for changes left undefined", () => {
    const thud = createClickSound({ tone: { hz: 460 }, volume: 0.18 });
    expect(
      createClickSound({ tone: { hz: undefined }, volume: undefined }).params
    ).toEqual(createClickSound().params);
    expect(
      thud.with({ tone: { hz: undefined }, volume: undefined }).params
    ).toEqual(thud.params);
  });

  it("synthesises lengthMs of audio at the context's sample rate", () => {
    const audio = createFakeAudioContext();
    createClickSound({ lengthMs: 10 }).play({ context: audio.context });
    expect(audio.fake.createBuffer).toHaveBeenCalledWith(1, 480, 48_000);
  });

  it("starts on the noise snap, then rings at the tone's pitch", () => {
    const audio = createFakeAudioContext();
    vi.spyOn(Math, "random").mockReturnValue(1);
    const quarterCycleHz = audio.context.sampleRate / 4;
    createClickSound({ noise: { gain: 0.5 }, tone: { gain: 0 } }).play({
      context: audio.context,
    });
    createClickSound({
      noise: { gain: 0 },
      tone: { decayMs: 1000, gain: 0.5, hz: quarterCycleHz },
    }).play({ context: audio.context });
    const [snap, ring] = audio.sources.map(
      (source) => source.buffer?.getChannelData(0) ?? []
    );
    expect(snap?.[0]).toBe(0.5);
    expect(Math.abs(snap?.[1] ?? 0)).toBeLessThan(0.5);
    expect(ring?.[0]).toBe(0);
    expect(ring?.[1]).toBeCloseTo(0.5);
    expect(ring?.[3]).toBeCloseTo(-0.5);
  });

  it("builds its buffer once per context", () => {
    const first = createFakeAudioContext();
    const second = createFakeAudioContext();
    const sound = createClickSound();
    sound.play({ context: first.context });
    sound.play({ context: first.context });
    sound.play({ context: second.context });
    expect(first.fake.createBuffer).toHaveBeenCalledTimes(1);
    expect(second.fake.createBuffer).toHaveBeenCalledTimes(1);
    expect(first.sources[1]?.buffer).toBe(first.sources[0]?.buffer);
  });

  it("builds a separate buffer for a derived sound", () => {
    const audio = createFakeAudioContext();
    const sound = createClickSound();
    sound.play({ context: audio.context });
    sound.with({ tone: { hz: 900 } }).play({ context: audio.context });
    expect(audio.fake.createBuffer).toHaveBeenCalledTimes(2);
    expect(audio.sources[1]?.buffer).not.toBe(audio.sources[0]?.buffer);
  });
});

describe("ClickSound.play", () => {
  it("plays at volume into the given destination", () => {
    const audio = createFakeAudioContext();
    const bus = { connect: vi.fn() } as unknown as AudioNode;
    createClickSound({ volume: 0.3 }).play({
      context: audio.context,
      destination: bus,
    });
    const [source] = audio.sources;
    const [gain] = audio.gains;
    expect(source?.connect).toHaveBeenCalledWith(gain);
    expect(gain?.gain.value).toBe(0.3);
    expect(gain?.connect).toHaveBeenCalledWith(bus);
    expect(source?.start).toHaveBeenCalledOnce();
  });

  it("plays through the shared page context by default", () => {
    const audio = createFakeAudioContext();
    shared.context = audio.context;
    createClickSound().play();
    expect(audio.gains[0]?.connect).toHaveBeenCalledWith(
      audio.fake.destination
    );
    expect(audio.sources[0]?.start).toHaveBeenCalledOnce();
  });

  it("stays silent without audio support", () => {
    expect(() => createClickSound().play()).not.toThrow();
  });

  it("queues one click while the context resumes, and asks again each time", () => {
    const audio = suspendedContext();
    const sound = createClickSound();
    sound.play({ context: audio.context });
    sound.play({ context: audio.context });
    expect(audio.sources).toHaveLength(1);
    expect(audio.fake.resume).toHaveBeenCalledTimes(2);
  });

  it("always starts scheduled clicks on a suspended context", () => {
    const audio = suspendedContext();
    const sound = createClickSound();
    sound.play({ context: audio.context, when: 1 });
    sound.play({ context: audio.context, when: 2 });
    expect(audio.sources).toHaveLength(2);
  });

  it("plays when a later resume starts the context", async () => {
    const audio = suspendedContext();
    const sound = createClickSound();
    sound.play({ context: audio.context });
    const started = Promise.resolve();
    audio.fake.resume.mockImplementationOnce(() => {
      audio.fake.state = "running";
      return started;
    });
    sound.play({ context: audio.context });
    await started;
    sound.play({ context: audio.context });
    expect(audio.sources).toHaveLength(2);
  });

  it("queues a click again after a refused resume", async () => {
    const audio = createFakeAudioContext();
    audio.fake.state = "suspended";
    audio.fake.resume.mockRejectedValueOnce(new Error("Not allowed"));
    const sound = createClickSound();
    sound.play({ context: audio.context });
    // Each try is dropped until the refused resume frees the queue.
    await vi.waitFor(() => {
      sound.play({ context: audio.context });
      expect(audio.sources).toHaveLength(2);
    });
  });

  it("queues clicks per context", () => {
    const suspended = suspendedContext();
    const running = createFakeAudioContext();
    const sound = createClickSound();
    sound.play({ context: suspended.context });
    sound.play({ context: running.context });
    expect(running.sources).toHaveLength(1);
  });

  it("does nothing on a closed context", () => {
    const audio = createFakeAudioContext();
    audio.fake.state = "closed";
    createClickSound().play({ context: audio.context });
    createClickSound().play({ context: audio.context, when: 1 });
    expect(audio.fake.createBuffer).not.toHaveBeenCalled();
    expect(audio.sources).toHaveLength(0);
    expect(audio.gains).toHaveLength(0);
  });

  it("varies pitch within pitchSpread", () => {
    const audio = createFakeAudioContext();
    vi.spyOn(Math, "random").mockReturnValue(0);
    createClickSound({ pitchSpread: 0.1 }).play({ context: audio.context });
    createClickSound({ pitchSpread: 0 }).play({ context: audio.context });
    expect(audio.sources[0]?.playbackRate.value).toBeCloseTo(0.9);
    expect(audio.sources[1]?.playbackRate.value).toBe(1);
  });

  it("holds back clicks closer than minIntervalMs", () => {
    const audio = createFakeAudioContext();
    const sound = createClickSound();
    const play = () =>
      sound.play({ context: audio.context, minIntervalMs: 30 });
    play();
    clock.now = 20;
    play();
    clock.now = 30;
    play();
    expect(audio.sources).toHaveLength(2);
  });

  it("limits each sound on its own timer", () => {
    const audio = createFakeAudioContext();
    const options = { context: audio.context, minIntervalMs: 30 };
    const tick = createClickSound();
    tick.play(options);
    clock.now = 10;
    tick.play(options);
    expect(audio.sources).toHaveLength(1);
    tick.with({ volume: 0.5 }).play(options);
    expect(audio.sources).toHaveLength(2);
  });

  it("never holds back scheduled clicks", () => {
    const audio = createFakeAudioContext();
    const sound = createClickSound();
    sound.play({ context: audio.context, minIntervalMs: 30 });
    sound.play({ context: audio.context, when: 1 });
    sound.play({ context: audio.context, when: 1.01 });
    expect(audio.sources.map((source) => source.start.mock.calls[0])).toEqual([
      [undefined],
      [1],
      [1.01],
    ]);
  });

  it("disconnects its nodes once played", () => {
    const audio = createFakeAudioContext();
    createClickSound().play({ context: audio.context });
    const [source] = audio.sources;
    const [[event, onEnded]] = source?.addEventListener.mock.calls ?? [[]];
    expect(event).toBe("ended");
    onEnded();
    expect(source?.disconnect).toHaveBeenCalled();
    expect(audio.gains[0]?.disconnect).toHaveBeenCalled();
  });
});
