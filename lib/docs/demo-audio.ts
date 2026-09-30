/**
 * Demo audio for the docs, synthesised in the browser with an
 * OfflineAudioContext. No audio files ship with the site, so every preview is
 * covered by the project's MIT licence.
 */

const SAMPLE_RATE = 44_100;
const BITS_PER_SAMPLE = 16;
const MAX_16_BIT = 0x7f_ff;

export interface DemoTrack {
  id: string;
  title: string;
  artist: string;
  duration: number;
}

export interface DemoSound {
  id: string;
  label: string;
  hotkey: string;
  accent: string;
}

export const DEMO_TRACKS: DemoTrack[] = [
  {
    artist: "audiocn",
    duration: 19.2,
    id: "night-drive",
    title: "Night Drive",
  },
  { artist: "audiocn", duration: 24, id: "low-tide", title: "Low Tide" },
  { artist: "audiocn", duration: 13.7, id: "arcade", title: "Arcade" },
];

export const DEMO_SOUNDS: DemoSound[] = [
  { accent: "oklch(0.7 0.2 30)", hotkey: "1", id: "airhorn", label: "Airhorn" },
  {
    accent: "oklch(0.75 0.15 85)",
    hotkey: "2",
    id: "applause",
    label: "Applause",
  },
  { accent: "oklch(0.7 0.15 250)", hotkey: "3", id: "ding", label: "Ding" },
  { accent: "oklch(0.7 0.2 320)", hotkey: "4", id: "laser", label: "Laser" },
  {
    accent: "oklch(0.72 0.17 150)",
    hotkey: "5",
    id: "success",
    label: "Success",
  },
  { accent: "oklch(0.65 0.2 20)", hotkey: "6", id: "fail", label: "Fail" },
  { accent: "oklch(0.7 0.1 200)", hotkey: "7", id: "whoosh", label: "Whoosh" },
  {
    accent: "oklch(0.68 0.16 60)",
    hotkey: "8",
    id: "drumroll",
    label: "Drum roll",
  },
];

type Render = (context: OfflineAudioContext) => void;

const noiseBuffer = (context: BaseAudioContext, seconds: number) => {
  const buffer = context.createBuffer(
    1,
    Math.ceil(seconds * context.sampleRate),
    context.sampleRate
  );
  const data = buffer.getChannelData(0);
  let seed = 7;
  for (let index = 0; index < data.length; index += 1) {
    seed = (seed * 16_807) % 2_147_483_647;
    data[index] = (seed / 2_147_483_647) * 2 - 1;
  }
  return buffer;
};

const envelope = (
  param: AudioParam,
  start: number,
  peak: number,
  attack: number,
  decay: number
) => {
  param.setValueAtTime(0.0001, start);
  param.exponentialRampToValueAtTime(peak, start + attack);
  param.exponentialRampToValueAtTime(0.0001, start + attack + decay);
};

const tone = (
  context: OfflineAudioContext,
  destination: AudioNode,
  options: {
    type: OscillatorType;
    frequency: number;
    start: number;
    peak: number;
    attack: number;
    decay: number;
    endFrequency?: number;
    detune?: number;
  }
) => {
  const oscillator = context.createOscillator();
  const gain = context.createGain();
  oscillator.type = options.type;
  oscillator.frequency.setValueAtTime(options.frequency, options.start);
  if (options.endFrequency) {
    oscillator.frequency.exponentialRampToValueAtTime(
      options.endFrequency,
      options.start + options.attack + options.decay
    );
  }
  oscillator.detune.value = options.detune ?? 0;
  envelope(
    gain.gain,
    options.start,
    options.peak,
    options.attack,
    options.decay
  );
  oscillator.connect(gain).connect(destination);
  oscillator.start(options.start);
  oscillator.stop(options.start + options.attack + options.decay + 0.05);
};

const noiseHit = (
  context: OfflineAudioContext,
  destination: AudioNode,
  start: number,
  {
    peak,
    decay,
    filter,
    frequency,
  }: {
    peak: number;
    decay: number;
    filter: BiquadFilterType;
    frequency: number;
  }
) => {
  const source = context.createBufferSource();
  source.buffer = noiseBuffer(context, decay + 0.05);
  const biquad = context.createBiquadFilter();
  biquad.type = filter;
  biquad.frequency.value = frequency;
  const gain = context.createGain();
  envelope(gain.gain, start, peak, 0.002, decay);
  source.connect(biquad).connect(gain).connect(destination);
  source.start(start);
};

const kick = (
  context: OfflineAudioContext,
  destination: AudioNode,
  start: number,
  peak = 0.9
) =>
  tone(context, destination, {
    attack: 0.002,
    decay: 0.35,
    endFrequency: 40,
    frequency: 150,
    peak,
    start,
    type: "sine",
  });

const hat = (
  context: OfflineAudioContext,
  destination: AudioNode,
  start: number,
  peak = 0.15
) =>
  noiseHit(context, destination, start, {
    decay: 0.05,
    filter: "highpass",
    frequency: 7000,
    peak,
  });

const snare = (
  context: OfflineAudioContext,
  destination: AudioNode,
  start: number,
  peak = 0.4
) => {
  noiseHit(context, destination, start, {
    decay: 0.18,
    filter: "bandpass",
    frequency: 1800,
    peak,
  });
  tone(context, destination, {
    attack: 0.002,
    decay: 0.12,
    frequency: 190,
    peak: peak * 0.6,
    start,
    type: "triangle",
  });
};

const midi = (note: number) => 440 * 2 ** ((note - 69) / 12);

const drumPattern = (
  context: OfflineAudioContext,
  destination: AudioNode,
  { bpm, bars, swing = 0 }: { bpm: number; bars: number; swing?: number }
) => {
  const beat = 60 / bpm;
  for (let bar = 0; bar < bars; bar += 1) {
    for (let step = 0; step < 8; step += 1) {
      const time =
        (bar * 4 + step / 2) * beat + (step % 2 === 1 ? swing * beat : 0);
      if (step === 0 || step === 5) {
        kick(context, destination, time);
      }
      if (step === 2 || step === 6) {
        snare(context, destination, time);
      }
      hat(context, destination, time, step % 2 === 0 ? 0.12 : 0.07);
    }
  }
};

const chordProgression = (
  context: OfflineAudioContext,
  destination: AudioNode,
  {
    chords,
    beatsPerChord,
    bpm,
    type,
    peak,
  }: {
    chords: number[][];
    beatsPerChord: number;
    bpm: number;
    type: OscillatorType;
    peak: number;
  }
) => {
  const beat = 60 / bpm;
  const filter = context.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 1800;
  filter.connect(destination);
  for (const [index, chord] of chords.entries()) {
    const start = index * beatsPerChord * beat;
    for (const note of chord) {
      for (const detune of [-7, 7]) {
        tone(context, filter, {
          attack: 0.4,
          decay: beatsPerChord * beat,
          detune,
          frequency: midi(note),
          peak,
          start,
          type,
        });
      }
    }
    tone(context, destination, {
      attack: 0.02,
      decay: beatsPerChord * beat * 0.9,
      frequency: midi((chord[0] ?? 48) - 12),
      peak: peak * 2.2,
      start,
      type: "sine",
    });
  }
};

const TRACKS: Record<string, { seconds: number; render: Render }> = {
  arcade: {
    render: (context) => {
      const bpm = 140;
      const beat = 60 / bpm;
      const notes = [72, 76, 79, 84, 79, 76, 72, 67];
      for (let step = 0; step < 64; step += 1) {
        const root = [0, 0, 5, 7][Math.floor(step / 16) % 4] ?? 0;
        tone(context, context.destination, {
          attack: 0.005,
          decay: beat / 2,
          frequency: midi((notes[step % notes.length] ?? 72) + root),
          peak: 0.08,
          start: (step * beat) / 2,
          type: "square",
        });
      }
      drumPattern(context, context.destination, { bars: 8, bpm });
    },
    seconds: 13.7,
  },
  "low-tide": {
    render: (context) => {
      const bpm = 80;
      chordProgression(context, context.destination, {
        beatsPerChord: 4,
        bpm,
        chords: [
          [57, 60, 64, 67],
          [53, 57, 60, 64],
          [55, 59, 62, 65],
          [52, 55, 59, 62],
          [57, 60, 64, 67],
          [53, 57, 60, 64],
          [55, 59, 62, 65],
          [52, 55, 59, 62],
        ],
        peak: 0.03,
        type: "triangle",
      });
      drumPattern(context, context.destination, { bars: 8, bpm, swing: 0.08 });
    },
    seconds: 24,
  },
  "night-drive": {
    render: (context) => {
      const bpm = 100;
      chordProgression(context, context.destination, {
        beatsPerChord: 4,
        bpm,
        chords: [
          [50, 53, 57, 60],
          [46, 50, 53, 57],
          [48, 52, 55, 59],
          [45, 48, 52, 55],
          [50, 53, 57, 60],
          [46, 50, 53, 57],
          [48, 52, 55, 59],
          [45, 48, 52, 55],
        ],
        peak: 0.025,
        type: "sawtooth",
      });
      drumPattern(context, context.destination, { bars: 8, bpm });
    },
    seconds: 19.2,
  },
};

const SOUNDS: Record<string, { seconds: number; render: Render }> = {
  airhorn: {
    render: (context) => {
      for (const frequency of [440, 554, 659]) {
        for (const start of [0, 0.35]) {
          tone(context, context.destination, {
            attack: 0.02,
            decay: start === 0 ? 0.28 : 0.9,
            frequency,
            peak: 0.12,
            start,
            type: "sawtooth",
          });
        }
      }
    },
    seconds: 1.4,
  },
  applause: {
    render: (context) => {
      for (let clap = 0; clap < 60; clap += 1) {
        const start = ((clap * 0.037) % 2) + (clap % 7) * 0.003;
        noiseHit(context, context.destination, start, {
          decay: 0.06,
          filter: "bandpass",
          frequency: 1200 + (clap % 5) * 300,
          peak: 0.25 * (1 - start / 2.4),
        });
      }
    },
    seconds: 2.4,
  },
  ding: {
    render: (context) => {
      for (const [index, ratio] of [1, 2.76, 5.4].entries()) {
        tone(context, context.destination, {
          attack: 0.002,
          decay: 1.6 / (index + 1),
          frequency: 880 * ratio,
          peak: 0.3 / (index + 1),
          start: 0,
          type: "sine",
        });
      }
    },
    seconds: 1.7,
  },
  drumroll: {
    render: (context) => {
      for (let hit = 0; hit < 40; hit += 1) {
        snare(context, context.destination, hit * 0.045, 0.1 + hit * 0.007);
      }
      kick(context, context.destination, 1.85);
      noiseHit(context, context.destination, 1.85, {
        decay: 0.9,
        filter: "highpass",
        frequency: 5000,
        peak: 0.3,
      });
    },
    seconds: 2.8,
  },
  fail: {
    render: (context) => {
      for (const [index, note] of [67, 66, 65, 64].entries()) {
        tone(context, context.destination, {
          attack: 0.01,
          decay: index === 3 ? 0.9 : 0.3,
          frequency: midi(note - 12),
          peak: 0.2,
          start: index * 0.3,
          type: "triangle",
        });
      }
    },
    seconds: 2,
  },
  laser: {
    render: (context) => {
      for (const start of [0, 0.18]) {
        tone(context, context.destination, {
          attack: 0.005,
          decay: 0.22,
          endFrequency: 180,
          frequency: 1800,
          peak: 0.2,
          start,
          type: "square",
        });
      }
    },
    seconds: 0.5,
  },
  success: {
    render: (context) => {
      for (const [index, note] of [72, 76, 79, 84].entries()) {
        tone(context, context.destination, {
          attack: 0.005,
          decay: index === 3 ? 0.8 : 0.25,
          frequency: midi(note),
          peak: 0.2,
          start: index * 0.09,
          type: "triangle",
        });
      }
    },
    seconds: 1.3,
  },
  whoosh: {
    render: (context) => {
      const source = context.createBufferSource();
      source.buffer = noiseBuffer(context, 1.2);
      const filter = context.createBiquadFilter();
      filter.type = "bandpass";
      filter.Q.value = 2;
      filter.frequency.setValueAtTime(300, 0);
      filter.frequency.exponentialRampToValueAtTime(4000, 0.6);
      filter.frequency.exponentialRampToValueAtTime(600, 1.1);
      const gain = context.createGain();
      gain.gain.setValueAtTime(0.0001, 0);
      gain.gain.exponentialRampToValueAtTime(0.5, 0.55);
      gain.gain.exponentialRampToValueAtTime(0.0001, 1.15);
      source.connect(filter).connect(gain).connect(context.destination);
      source.start(0);
    },
    seconds: 1.2,
  },
};

const encodeWav = (buffer: AudioBuffer): Blob => {
  const channels = buffer.numberOfChannels;
  const frames = buffer.length;
  const bytesPerSample = BITS_PER_SAMPLE / 8;
  const dataSize = frames * channels * bytesPerSample;
  const view = new DataView(new ArrayBuffer(44 + dataSize));
  const writeText = (offset: number, text: string) => {
    for (let index = 0; index < text.length; index += 1) {
      view.setUint8(offset + index, text.codePointAt(index) ?? 0);
    }
  };
  writeText(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeText(8, "WAVE");
  writeText(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channels, true);
  view.setUint32(24, buffer.sampleRate, true);
  view.setUint32(28, buffer.sampleRate * channels * bytesPerSample, true);
  view.setUint16(32, channels * bytesPerSample, true);
  view.setUint16(34, BITS_PER_SAMPLE, true);
  writeText(36, "data");
  view.setUint32(40, dataSize, true);
  const data = Array.from({ length: channels }, (_, channel) =>
    buffer.getChannelData(channel)
  );
  let offset = 44;
  for (let frame = 0; frame < frames; frame += 1) {
    for (const channelData of data) {
      const sample = Math.max(-1, Math.min(1, channelData[frame] ?? 0));
      view.setInt16(offset, Math.round(sample * MAX_16_BIT), true);
      offset += bytesPerSample;
    }
  }
  return new Blob([view], { type: "audio/wav" });
};

const renderCache = new Map<string, Promise<AudioBuffer>>();
const urlCache = new Map<string, Promise<string>>();

const renderBuffer = (
  key: string,
  recipe: { seconds: number; render: Render }
) => {
  const cached = renderCache.get(key);
  if (cached) {
    return cached;
  }
  const context = new OfflineAudioContext(
    1,
    Math.ceil(recipe.seconds * SAMPLE_RATE),
    SAMPLE_RATE
  );
  recipe.render(context);
  const rendering = context.startRendering();
  renderCache.set(key, rendering);
  return rendering;
};

/** Renders a demo sound effect into an AudioBuffer. */
export const renderDemoSound = (id: string): Promise<AudioBuffer> => {
  const recipe = SOUNDS[id];
  if (!recipe) {
    return Promise.reject(new Error(`Unknown demo sound: ${id}`));
  }
  return renderBuffer(`sound:${id}`, recipe);
};

/** Renders a demo track and returns an object URL for an audio element. */
export const renderDemoTrackUrl = (id: string): Promise<string> => {
  const cached = urlCache.get(id);
  if (cached) {
    return cached;
  }
  const recipe = TRACKS[id];
  if (!recipe) {
    return Promise.reject(new Error(`Unknown demo track: ${id}`));
  }
  const render = async () => {
    const buffer = await renderBuffer(`track:${id}`, recipe);
    return URL.createObjectURL(encodeWav(buffer));
  };
  const url = render();
  urlCache.set(id, url);
  return url;
};
