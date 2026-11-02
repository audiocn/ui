"use client";

import { useEffect, useMemo } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";
import { bandsFromSpectrum, logBandEdges } from "@/lib/audio/bands";
import { dbToLevel, peakDb, rmsDb } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import { createFrameRelay } from "@/lib/audio/frame-source";
import { appendHistory } from "@/lib/audio/history";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";

export type AnalyserInput = MediaStream | HTMLMediaElement | AudioNode | null;

export interface AnalyserTapOptions {
  /** Analyser FFT size. Default 2048. */
  fftSize?: number;
  /** Analyser smoothing constant, 0..1. Default 0.3. */
  smoothing?: number;
  /** Frequency bands per visual frame. Default 32. */
  bands?: number;
  /** Lowest band frequency. Default 40 Hz. */
  minHz?: number;
  /** Highest band frequency. Default 16 kHz. */
  maxHz?: number;
  /** Entries in the level history ring. Default 60. */
  historySize?: number;
  /** Time between history entries. Default 50 ms. */
  historyIntervalMs?: number;
  /** Minimum time between frames. 0 sends a frame every animation frame. */
  intervalMs?: number;
  /** `stereo` measures left and right separately. Default `mono`. */
  channels?: "mono" | "stereo";
}

export interface AudioAnalyserOptions extends AnalyserTapOptions {
  /** `false` disconnects and releases the analysis. Default true. */
  enabled?: boolean;
}

export type AudioAnalyserStatus = "idle" | "running" | "suspended";

export interface AudioAnalyser {
  meter: FrameSource<MeterFrame>;
  visual: FrameSource<VisualFrame>;
  status: AudioAnalyserStatus;
}

export interface AnalyserTap {
  meter: FrameSource<MeterFrame>;
  visual: FrameSource<VisualFrame>;
  dispose: () => void;
}

const mediaElementSources = new WeakMap<
  HTMLMediaElement,
  MediaElementAudioSourceNode
>();

/**
 * Connects a media element to the context once, and routes it to the
 * speakers through the context so it keeps playing.
 */
export const getMediaElementSource = (
  context: AudioContext,
  element: HTMLMediaElement
): MediaElementAudioSourceNode => {
  const existing = mediaElementSources.get(element);
  if (existing) {
    return existing;
  }
  const node = context.createMediaElementSource(element);
  node.connect(context.destination);
  mediaElementSources.set(element, node);
  return node;
};

/** Turns any analyser input into an audio node, and says whether you own it. */
export const createInputNode = (
  context: AudioContext,
  input: Exclude<AnalyserInput, null>
): { node: AudioNode; owned: boolean } => {
  if (input instanceof MediaStream) {
    return { node: context.createMediaStreamSource(input), owned: true };
  }
  if (input instanceof HTMLMediaElement) {
    return { node: getMediaElementSource(context, input), owned: false };
  }
  return { node: input, owned: false };
};

/**
 * Disconnects one connection, if it still exists. A node you don't own may
 * already have been disconnected by its owner, and Web Audio throws then.
 */
export const disconnectFrom = (node: AudioNode, destination: AudioNode) => {
  try {
    node.disconnect(destination);
  } catch {
    // Already disconnected.
  }
};

/**
 * Taps an audio node with analysers and exposes meter and visual frame
 * sources. Analysis runs only while something is subscribed.
 */
export const createAnalyserTap = (
  context: BaseAudioContext,
  node: AudioNode,
  {
    fftSize = 2048,
    smoothing = 0.3,
    bands = 32,
    minHz = 40,
    maxHz = 16_000,
    historySize = 60,
    historyIntervalMs = 50,
    intervalMs = 0,
    channels = "mono",
  }: AnalyserTapOptions = {}
): AnalyserTap => {
  const createAnalyser = () => {
    const analyser = context.createAnalyser();
    analyser.fftSize = fftSize;
    analyser.smoothingTimeConstant = smoothing;
    return analyser;
  };

  const mix = createAnalyser();
  node.connect(mix);
  const analysers: AnalyserNode[] = [];
  let splitter: ChannelSplitterNode | null = null;
  if (channels === "stereo") {
    splitter = context.createChannelSplitter(2);
    node.connect(splitter);
    for (let channel = 0; channel < 2; channel += 1) {
      const analyser = createAnalyser();
      splitter.connect(analyser, channel);
      analysers.push(analyser);
    }
  } else {
    analysers.push(mix);
  }

  const timeDomain = new Float32Array(fftSize);
  const mixTimeDomain = new Float32Array(fftSize);
  const spectrum = new Float32Array(mix.frequencyBinCount);
  const edges = logBandEdges(
    bands,
    minHz,
    Math.min(maxHz, context.sampleRate / 2)
  );
  const meterFrame: MeterFrame = { channels: [] };
  const visualFrame: VisualFrame = {
    bands: new Float32Array(bands),
    history: new Float32Array(historySize),
    historyLength: 0,
    historyStart: 0,
    peakDb: Number.NEGATIVE_INFINITY,
    timeDomain: mixTimeDomain,
  };
  const meterSubscribers = new Set<(frame: MeterFrame) => void>();
  const visualSubscribers = new Set<(frame: VisualFrame) => void>();
  let lastFrameMs = 0;
  let stopLoop: (() => void) | null = null;
  let disposed = false;

  const tick = (nowMs: number) => {
    if (nowMs - lastFrameMs < intervalMs) {
      return;
    }
    lastFrameMs = nowMs;

    if (meterSubscribers.size > 0) {
      meterFrame.channels.length = analysers.length;
      for (const [index, analyser] of analysers.entries()) {
        analyser.getFloatTimeDomainData(timeDomain);
        meterFrame.channels[index] = {
          peakDb: peakDb(timeDomain),
          rmsDb: rmsDb(timeDomain),
        };
      }
      for (const subscriber of meterSubscribers) {
        subscriber(meterFrame);
      }
    }

    if (visualSubscribers.size === 0) {
      return;
    }
    mix.getFloatTimeDomainData(mixTimeDomain);
    mix.getFloatFrequencyData(spectrum);
    bandsFromSpectrum(spectrum, context.sampleRate, edges, visualFrame.bands);
    visualFrame.peakDb = peakDb(mixTimeDomain);
    appendHistory(
      visualFrame,
      dbToLevel(visualFrame.peakDb),
      nowMs,
      historyIntervalMs
    );
    for (const subscriber of visualSubscribers) {
      subscriber(visualFrame);
    }
  };

  const updateLoop = () => {
    const active =
      !disposed && meterSubscribers.size + visualSubscribers.size > 0;
    if (active && !stopLoop) {
      stopLoop = subscribeFrame(tick, "update");
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

  const dispose = () => {
    disposed = true;
    updateLoop();
    disconnectFrom(node, mix);
    if (splitter) {
      disconnectFrom(node, splitter);
      splitter.disconnect();
    }
  };

  return {
    dispose,
    meter: sourceFor(meterSubscribers),
    visual: sourceFor(visualSubscribers),
  };
};

/**
 * Turns a `MediaStream`, media element or `AudioNode` into meter and visual
 * frame sources. The sources stay the same when the input changes.
 */
export const useAudioAnalyser = (
  input: AnalyserInput,
  {
    fftSize,
    smoothing,
    bands,
    minHz,
    maxHz,
    historySize,
    historyIntervalMs,
    intervalMs,
    channels,
    enabled = true,
  }: AudioAnalyserOptions = {}
): AudioAnalyser => {
  const { context, status: contextStatus } = useAudioContext();
  const relays = useMemo(
    () => ({
      meter: createFrameRelay<MeterFrame>(),
      visual: createFrameRelay<VisualFrame>(),
    }),
    []
  );

  useEffect(() => {
    if (!(context && input && enabled)) {
      return;
    }
    const { node, owned } = createInputNode(context, input);
    const tap = createAnalyserTap(context, node, {
      bands,
      channels,
      fftSize,
      historyIntervalMs,
      historySize,
      intervalMs,
      maxHz,
      minHz,
      smoothing,
    });
    relays.meter.setSource(tap.meter);
    relays.visual.setSource(tap.visual);

    return () => {
      relays.meter.setSource(null);
      relays.visual.setSource(null);
      tap.dispose();
      if (owned) {
        node.disconnect();
      }
    };
  }, [
    bands,
    channels,
    context,
    enabled,
    fftSize,
    historyIntervalMs,
    historySize,
    input,
    intervalMs,
    maxHz,
    minHz,
    relays,
    smoothing,
  ]);

  let status: AudioAnalyserStatus = "idle";
  if (input !== null && enabled) {
    status = contextStatus === "running" ? "running" : "suspended";
  }

  return { meter: relays.meter, status, visual: relays.visual };
};
