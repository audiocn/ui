"use client";

import { useEffect, useState } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";
import { bandsFromSpectrum, logBandEdges } from "@/lib/audio/bands";
import { dbToLevel, peakDb, rmsDb } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";

export type AnalyserInput = MediaStream | HTMLMediaElement | AudioNode | null;

export interface AudioAnalyserOptions {
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
  /** Pause analysis without tearing it down. Default true. */
  enabled?: boolean;
}

export type AudioAnalyserStatus = "idle" | "running" | "suspended";

export interface AudioAnalyser {
  meter: FrameSource<MeterFrame>;
  visual: FrameSource<VisualFrame>;
  status: AudioAnalyserStatus;
}

const mediaElementSources = new WeakMap<
  HTMLMediaElement,
  MediaElementAudioSourceNode
>();

/**
 * Connects a media element to the context once. Its audio then plays through
 * the context instead of directly, so it is also routed to the speakers.
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

const createSourceNode = (
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

interface Graph {
  analysers: AnalyserNode[];
  mix: AnalyserNode;
  sampleRate: number;
  dispose: () => void;
}

const buildGraph = (
  context: AudioContext,
  input: Exclude<AnalyserInput, null>,
  options: Required<
    Pick<AudioAnalyserOptions, "fftSize" | "smoothing" | "channels">
  >
): Graph => {
  const { node, owned } = createSourceNode(context, input);
  const createAnalyser = () => {
    const analyser = context.createAnalyser();
    analyser.fftSize = options.fftSize;
    analyser.smoothingTimeConstant = options.smoothing;
    return analyser;
  };

  const mix = createAnalyser();
  node.connect(mix);
  const analysers: AnalyserNode[] = [];
  let splitter: ChannelSplitterNode | null = null;

  if (options.channels === "stereo") {
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

  const dispose = () => {
    node.disconnect(mix);
    if (splitter) {
      node.disconnect(splitter);
      splitter.disconnect();
    }
    if (owned) {
      node.disconnect();
    }
  };

  return { analysers, dispose, mix, sampleRate: context.sampleRate };
};

/**
 * Turns a `MediaStream`, media element or `AudioNode` into meter and visual
 * frame sources. Analysis only runs while something is subscribed.
 */
export const useAudioAnalyser = (
  input: AnalyserInput,
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
    enabled = true,
  }: AudioAnalyserOptions = {}
): AudioAnalyser => {
  const { context, status: contextStatus } = useAudioContext();
  const [hub] = useState(() => {
    const meterSubscribers = new Set<(frame: MeterFrame) => void>();
    const visualSubscribers = new Set<(frame: VisualFrame) => void>();
    const listeners = new Set<() => void>();
    const sourceFor = <T>(
      subscribers: Set<(frame: T) => void>
    ): FrameSource<T> => ({
      subscribe: (callback) => {
        subscribers.add(callback);
        for (const listener of listeners) {
          listener();
        }
        return () => {
          subscribers.delete(callback);
          for (const listener of listeners) {
            listener();
          }
        };
      },
    });
    return {
      listeners,
      meter: sourceFor(meterSubscribers),
      meterSubscribers,
      visual: sourceFor(visualSubscribers),
      visualSubscribers,
    };
  });

  const hasInput = input !== null;

  useEffect(() => {
    if (!(context && input && enabled)) {
      return;
    }

    const graph = buildGraph(context, input, { channels, fftSize, smoothing });
    const timeDomain = new Float32Array(fftSize);
    const mixTimeDomain = new Float32Array(fftSize);
    const spectrum = new Float32Array(graph.mix.frequencyBinCount);
    const edges = logBandEdges(
      bands,
      minHz,
      Math.min(maxHz, graph.sampleRate / 2)
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
    let lastFrameMs = 0;
    let lastHistoryMs = 0;
    let stopLoop: (() => void) | null = null;

    const pushHistory = (level: number) => {
      const { history } = visualFrame;
      const size = history.length;
      if (visualFrame.historyLength < size) {
        history[(visualFrame.historyStart + visualFrame.historyLength) % size] =
          level;
        visualFrame.historyLength += 1;
      } else {
        history[visualFrame.historyStart] = level;
        visualFrame.historyStart = (visualFrame.historyStart + 1) % size;
      }
    };

    const tick = (nowMs: number) => {
      if (nowMs - lastFrameMs < intervalMs) {
        return;
      }
      lastFrameMs = nowMs;

      meterFrame.channels.length = graph.analysers.length;
      for (const [index, analyser] of graph.analysers.entries()) {
        analyser.getFloatTimeDomainData(timeDomain);
        meterFrame.channels[index] = {
          peakDb: peakDb(timeDomain),
          rmsDb: rmsDb(timeDomain),
        };
      }
      for (const subscriber of hub.meterSubscribers) {
        subscriber(meterFrame);
      }

      if (hub.visualSubscribers.size === 0) {
        return;
      }
      graph.mix.getFloatTimeDomainData(mixTimeDomain);
      graph.mix.getFloatFrequencyData(spectrum);
      bandsFromSpectrum(spectrum, graph.sampleRate, edges, visualFrame.bands);
      visualFrame.peakDb = peakDb(mixTimeDomain);
      if (nowMs - lastHistoryMs >= historyIntervalMs) {
        lastHistoryMs = nowMs;
        pushHistory(dbToLevel(visualFrame.peakDb));
      }
      for (const subscriber of hub.visualSubscribers) {
        subscriber(visualFrame);
      }
    };

    const updateLoop = () => {
      const active = hub.meterSubscribers.size + hub.visualSubscribers.size > 0;
      if (active && !stopLoop) {
        stopLoop = subscribeFrame(tick);
      } else if (!active && stopLoop) {
        stopLoop();
        stopLoop = null;
      }
    };

    hub.listeners.add(updateLoop);
    updateLoop();

    return () => {
      hub.listeners.delete(updateLoop);
      stopLoop?.();
      graph.dispose();
    };
  }, [
    bands,
    channels,
    context,
    enabled,
    fftSize,
    historyIntervalMs,
    historySize,
    hub,
    input,
    intervalMs,
    maxHz,
    minHz,
    smoothing,
  ]);

  let status: AudioAnalyserStatus = "idle";
  if (hasInput && enabled) {
    status = contextStatus === "running" ? "running" : "suspended";
  }

  return { meter: hub.meter, status, visual: hub.visual };
};
