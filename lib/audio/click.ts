import { getSharedAudioContext } from "@/hooks/use-audio-context";

export interface ClickSoundParams {
  /** A ring: a sine at `hz` that fades over `decayMs`. */
  readonly tone: {
    readonly hz: number;
    readonly decayMs: number;
    readonly gain: number;
  };
  /** A snap: white noise that fades over `decayMs`. */
  readonly noise: { readonly decayMs: number; readonly gain: number };
  readonly lengthMs: number;
  /** Random pitch per click, as a ratio: 0.04 is ±4%. */
  readonly pitchSpread: number;
  /** Linear output gain. */
  readonly volume: number;
}

export type ClickSoundChanges = {
  readonly [Key in keyof ClickSoundParams]?: Partial<ClickSoundParams[Key]>;
};

export type ClickPlayOptions = {
  /** Default: the shared page context. */
  context?: AudioContext | null;
  /** Default: the context's speakers. */
  destination?: AudioNode;
} & (
  | {
      when?: undefined;
      /** Clicks closer than this to this sound's last one are dropped. */
      minIntervalMs?: number;
    }
  | {
      /** Context time to play at. Scheduled clicks are never held back. */
      when: number;
      minIntervalMs?: never;
    }
);

export interface ClickSound {
  /** Plays once. Resumes a suspended context (needs a user gesture). */
  play: (options?: ClickPlayOptions) => void;
  /** A new sound with these parameters deep-merged over this one's. */
  with: (changes: ClickSoundChanges) => ClickSound;
  /** The full parameters after merging, for docs or a tuning UI. */
  readonly params: ClickSoundParams;
}

/** A short noise snap over a high, fast-damped ring: a tick, not a thud. */
const DEFAULT_PARAMS: ClickSoundParams = {
  lengthMs: 6,
  noise: { decayMs: 0.4, gain: 0.6 },
  pitchSpread: 0.04,
  tone: { decayMs: 1.2, gain: 0.4, hz: 4200 },
  volume: 0.12,
};

const MS_PER_SECOND = 1000;

const merge = (
  base: ClickSoundParams,
  changes: ClickSoundChanges
): ClickSoundParams => ({
  ...base,
  ...changes,
  noise: { ...base.noise, ...changes.noise },
  tone: { ...base.tone, ...changes.tone },
});

/** The sum of a decaying noise snap and a decaying sine ring. */
const synthesize = (
  context: BaseAudioContext,
  { lengthMs, noise, tone }: ClickSoundParams
) => {
  const { sampleRate } = context;
  const buffer = context.createBuffer(
    1,
    Math.ceil((lengthMs / MS_PER_SECOND) * sampleRate),
    sampleRate
  );
  const noiseDecay = noise.decayMs / MS_PER_SECOND;
  const toneDecay = tone.decayMs / MS_PER_SECOND;
  const samples = buffer.getChannelData(0);
  for (let index = 0; index < samples.length; index += 1) {
    const time = index / sampleRate;
    const snap =
      noise.gain * (Math.random() * 2 - 1) * Math.exp(-time / noiseDecay);
    const ring =
      tone.gain *
      Math.sin(2 * Math.PI * tone.hz * time) *
      Math.exp(-time / toneDecay);
    samples[index] = snap + ring;
  }
  return buffer;
};

const resumeContext = async (context: AudioContext) => {
  try {
    await context.resume();
  } catch {
    // Without a user gesture the browser refuses; the next click tries again.
  }
};

const clickSound = (params: ClickSoundParams): ClickSound => {
  const buffers = new WeakMap<BaseAudioContext, AudioBuffer>();
  let lastPlayedAt = Number.NEGATIVE_INFINITY;

  const bufferFor = (context: BaseAudioContext) => {
    const buffer = buffers.get(context) ?? synthesize(context, params);
    buffers.set(context, buffer);
    return buffer;
  };

  const play = (options: ClickPlayOptions = {}) => {
    const context = options.context ?? getSharedAudioContext();
    if (!context) {
      return;
    }
    if (options.when === undefined) {
      const now = performance.now();
      if (now - lastPlayedAt < (options.minIntervalMs ?? 0)) {
        return;
      }
      lastPlayedAt = now;
    }
    if (context.state === "suspended") {
      resumeContext(context);
    }
    const source = context.createBufferSource();
    source.buffer = bufferFor(context);
    source.playbackRate.value =
      1 + (Math.random() - 0.5) * params.pitchSpread * 2;
    const gain = context.createGain();
    gain.gain.value = params.volume;
    source.connect(gain);
    gain.connect(options.destination ?? context.destination);
    source.addEventListener("ended", () => {
      source.disconnect();
      gain.disconnect();
    });
    source.start(options.when);
  };

  return {
    params,
    play,
    with: (changes) => clickSound(merge(params, changes)),
  };
};

/** A synthesised click. Without changes, Knob's tick. */
export const createClickSound = (changes: ClickSoundChanges = {}) =>
  clickSound(merge(DEFAULT_PARAMS, changes));
