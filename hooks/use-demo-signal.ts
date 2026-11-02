"use client";

import { useEffect, useMemo } from "react";

import { clamp, dbToGain, dbToLevel, gainToDb } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { appendHistory } from "@/lib/audio/history";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";

export type DemoSignalKind = "speech" | "music" | "tone" | "noise" | "silence";

export interface DemoSignalOptions {
  /** What the signal sounds like. Default `speech`. */
  kind?: DemoSignalKind;
  /** 1 for mono, 2 for stereo. Default 1. */
  channels?: number;
  /** Changes the pattern while keeping it repeatable. Default 1. */
  seed?: number;
  /** Frequency bands per visual frame. Default 32. */
  bands?: number;
  /** Entries in the level history ring. Default 60. */
  historySize?: number;
  /** Time between history entries. Default 50 ms. */
  historyIntervalMs?: number;
  /** Gain applied to the signal in dB. Default 0; -Infinity silences it. */
  gainDb?: number;
  /** When false the signal falls silent. Default true. */
  playing?: boolean;
}

export interface DemoSignal {
  meter: FrameSource<MeterFrame>;
  visual: FrameSource<VisualFrame>;
  configure: (options: DemoSignalOptions) => void;
}

const TIME_DOMAIN_SIZE = 256;
const TIME_DOMAIN_RATE = 12_800;
const TWO_PI = Math.PI * 2;
const ROOM_TONE_DB = -52;
const MS_PER_SECOND = 1000;

const CREST_DB: Record<DemoSignalKind, number> = {
  music: 8,
  noise: 11,
  silence: 0,
  speech: 12,
  tone: 3.01,
};

const fract = (value: number) => value - Math.floor(value);

const hash = (seed: number, index: number) =>
  fract(Math.sin(seed * 12.9898 + index * 78.233) * 43_758.5453);

const smoothNoise = (seed: number, time: number) => {
  const index = Math.floor(time);
  const phase = time - index;
  const eased = phase * phase * (3 - 2 * phase);
  const from = hash(seed, index);
  const to = hash(seed, index + 1);
  return from + (to - from) * eased;
};

const speechAmplitude = (seed: number, seconds: number) => {
  const phraseLength = 2.4;
  const phrase = Math.floor(seconds / phraseLength);
  const phraseTime = seconds - phrase * phraseLength;
  const speaking = phraseTime < 1.5 + 0.5 * hash(seed, phrase);
  const room =
    dbToGain(ROOM_TONE_DB) * (0.6 + 0.4 * smoothNoise(seed, seconds * 9));
  if (!speaking) {
    return room;
  }
  const syllableRate = 5;
  const syllable = Math.floor(seconds * syllableRate);
  const shape = Math.sin(Math.PI * fract(seconds * syllableRate)) ** 1.5;
  const chance = hash(seed + 3, syllable);
  let syllableDb = -18 + 12 * hash(seed, syllable);
  if (chance > 0.985) {
    syllableDb = -0.4;
  } else if (chance > 0.95) {
    syllableDb = -4;
  }
  return room + dbToGain(syllableDb) * shape;
};

const musicAmplitude = (seed: number, seconds: number) => {
  const beat = 0.5;
  const phase = fract(seconds / beat);
  const kick = dbToGain(-6) * Math.exp(-phase * 8);
  const body = dbToGain(-16) * (0.8 + 0.2 * smoothNoise(seed, seconds * 2));
  const bar = Math.floor(seconds / (beat * 4));
  const barPhase = fract(seconds / (beat * 4));
  const crash =
    hash(seed + 5, bar) > 0.85 ? dbToGain(-3) * Math.exp(-barPhase * 3) : 0;
  return body + kick + crash;
};

const amplitudeFor = (kind: DemoSignalKind, seed: number, seconds: number) => {
  switch (kind) {
    case "speech": {
      return speechAmplitude(seed, seconds);
    }
    case "music": {
      return musicAmplitude(seed, seconds);
    }
    case "tone": {
      return dbToGain(-12);
    }
    case "noise": {
      return dbToGain(-20) * (0.7 + 0.3 * smoothNoise(seed, seconds * 8));
    }
    case "silence": {
      return 0;
    }
    default: {
      return 0;
    }
  }
};

const gaussian = (x: number, center: number, width: number) =>
  Math.exp(-(((x - center) / width) ** 2));

const bandTemplate = (kind: DemoSignalKind, x: number) => {
  switch (kind) {
    case "speech": {
      return 0.25 + gaussian(x, 0.4, 0.16) + 0.6 * gaussian(x, 0.66, 0.1);
    }
    case "music": {
      return 1.05 - 0.55 * x + 0.3 * gaussian(x, 0.85, 0.08);
    }
    case "tone": {
      return 0.05 + gaussian(x, 0.54, 0.03);
    }
    case "noise": {
      return 0.85 - 0.2 * x;
    }
    case "silence": {
      return 0;
    }
    default: {
      return 0;
    }
  }
};

const sampleWave = (kind: DemoSignalKind, seed: number, time: number) => {
  switch (kind) {
    case "speech": {
      return (
        0.6 * Math.sin(TWO_PI * 140 * time) +
        0.3 * Math.sin(TWO_PI * 420 * time) +
        0.1 * Math.sin(TWO_PI * 1100 * time)
      );
    }
    case "music": {
      return (
        0.5 * Math.sin(TWO_PI * 55 * time) +
        0.3 * Math.sin(TWO_PI * 220 * time) +
        0.2 * Math.sin(TWO_PI * 880 * time)
      );
    }
    case "tone": {
      return Math.sin(TWO_PI * 1000 * time);
    }
    case "noise": {
      return hash(seed, time * TIME_DOMAIN_RATE) * 2 - 1;
    }
    case "silence": {
      return 0;
    }
    default: {
      return 0;
    }
  }
};

/**
 * Creates a synthetic signal as frame sources, so meters and visualizers can
 * move without a microphone. It only runs while something is subscribed.
 */
export const createDemoSignal = (
  initialOptions: DemoSignalOptions = {}
): DemoSignal => {
  let kind: DemoSignalKind = "speech";
  let channels = 1;
  let seed = 1;
  let playing = true;
  let gainDb = 0;
  let historyIntervalMs = 50;
  let bands = new Float32Array(32);
  let history = new Float32Array(60);
  let startMs: number | null = null;
  const timeDomain = new Float32Array(TIME_DOMAIN_SIZE);
  const meterFrame: MeterFrame = { channels: [] };
  const visualFrame: VisualFrame = {
    bands,
    history,
    historyLength: 0,
    historyStart: 0,
    peakDb: Number.NEGATIVE_INFINITY,
    timeDomain,
  };

  const meterSubscribers = new Set<(frame: MeterFrame) => void>();
  const visualSubscribers = new Set<(frame: VisualFrame) => void>();
  let stopLoop: (() => void) | null = null;

  const configure = (options: DemoSignalOptions) => {
    kind = options.kind ?? kind;
    channels = clamp(Math.round(options.channels ?? channels), 1, 8);
    seed = options.seed ?? seed;
    playing = options.playing ?? playing;
    gainDb = options.gainDb ?? gainDb;
    historyIntervalMs = options.historyIntervalMs ?? historyIntervalMs;
    if (options.bands !== undefined && options.bands !== bands.length) {
      bands = new Float32Array(options.bands);
      visualFrame.bands = bands;
    }
    if (
      options.historySize !== undefined &&
      options.historySize !== history.length
    ) {
      history = new Float32Array(options.historySize);
      visualFrame.historyStart = 0;
      visualFrame.historyLength = 0;
      visualFrame.historyPreviousLevel = undefined;
      visualFrame.history = history;
    }
  };

  const produce = (nowMs: number) => {
    startMs ??= nowMs;
    const seconds = (nowMs - startMs) / MS_PER_SECOND;
    const activeKind: DemoSignalKind = playing ? kind : "silence";
    const inputAmplitude = Math.min(1, amplitudeFor(activeKind, seed, seconds));
    const gain = dbToGain(gainDb);
    const amplitude = inputAmplitude * gain;
    const crestGain = dbToGain(-CREST_DB[activeKind]);

    meterFrame.channels.length = channels;
    let loudest = 0;
    for (let channel = 0; channel < channels; channel += 1) {
      const wobble =
        channel === 0
          ? 1
          : 1 + 0.3 * (smoothNoise(seed + 17 * channel, seconds * 3) - 0.5);
      const channelAmplitude = Math.min(1, inputAmplitude * wobble) * gain;
      loudest = Math.max(loudest, channelAmplitude);
      meterFrame.channels[channel] = {
        peakDb: gainToDb(channelAmplitude),
        rmsDb: gainToDb(channelAmplitude * crestGain),
      };
    }

    const level = dbToLevel(gainToDb(loudest));
    const count = bands.length;
    for (let band = 0; band < count; band += 1) {
      const x = count > 1 ? band / (count - 1) : 0;
      const movement = 0.7 + 0.6 * smoothNoise(seed + band * 13, seconds * 6);
      bands[band] = clamp(bandTemplate(activeKind, x) * level * movement, 0, 1);
    }

    appendHistory(visualFrame, level, nowMs, historyIntervalMs);

    for (let index = 0; index < TIME_DOMAIN_SIZE; index += 1) {
      const time = seconds + index / TIME_DOMAIN_RATE;
      timeDomain[index] = amplitude * sampleWave(activeKind, seed, time);
    }

    visualFrame.peakDb = gainToDb(loudest);

    for (const subscriber of meterSubscribers) {
      subscriber(meterFrame);
    }
    for (const subscriber of visualSubscribers) {
      subscriber(visualFrame);
    }
  };

  const updateLoop = () => {
    const active = meterSubscribers.size + visualSubscribers.size > 0;
    if (active && !stopLoop) {
      stopLoop = subscribeFrame(produce, "update");
    } else if (!active && stopLoop) {
      stopLoop();
      stopLoop = null;
    }
  };

  const sourceFor = <T>(
    subscribers: Set<(frame: T) => void>
  ): FrameSource<T> => ({
    subscribe: (listener) => {
      subscribers.add(listener);
      updateLoop();
      return () => {
        subscribers.delete(listener);
        updateLoop();
      };
    },
  });

  configure(initialOptions);

  return {
    configure,
    meter: sourceFor(meterSubscribers),
    visual: sourceFor(visualSubscribers),
  };
};

/** A synthetic signal for previews, prototypes and tests. */
export const useDemoSignal = (options: DemoSignalOptions = {}): DemoSignal => {
  const signal = useMemo(() => createDemoSignal(), []);
  const {
    bands,
    channels,
    gainDb,
    historyIntervalMs,
    historySize,
    kind,
    playing,
    seed,
  } = options;

  useEffect(() => {
    signal.configure({
      bands,
      channels,
      gainDb,
      historyIntervalMs,
      historySize,
      kind,
      playing,
      seed,
    });
  }, [
    signal,
    bands,
    channels,
    gainDb,
    historyIntervalMs,
    historySize,
    kind,
    playing,
    seed,
  ]);

  return signal;
};
