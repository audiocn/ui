"use client";

import {
  useEffect,
  useEffectEvent,
  useMemo,
  useSyncExternalStore,
} from "react";

import {
  createAnalyserTap,
  createInputNode,
  disconnectFrom,
} from "@/hooks/use-audio-analyser";
import type {
  AnalyserInput,
  AnalyserTap,
  AnalyserTapOptions,
} from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import { isChannelAudible } from "@/hooks/use-mixer";
import type { Mixer, MixerState } from "@/hooks/use-mixer";
import { dbToGain } from "@/lib/audio/decibels";
import { createFrameRelay } from "@/lib/audio/frame-source";
import type { FrameRelay } from "@/lib/audio/frame-source";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";

const RAMP_SECONDS = 0.005;
const MS_PER_SECOND = 1000;
const TIME_CONSTANTS_PER_RAMP = 3;
/** How long a duck holds after the trigger's last loud frame. */
const DUCK_HOLD_SECONDS = 0.2;

export interface DuckingOptions {
  /** The channel whose level triggers ducking, usually the microphone. */
  trigger: string;
  /** Channels that get quieter while the trigger is active. */
  targets: string[];
  /** Level that counts as active. Default −35 dBFS. */
  thresholdDb?: number;
  /** How much quieter the targets get. Default −12 dB. */
  amountDb?: number;
  /** Default 50 ms. */
  attackMs?: number;
  /** Default 400 ms. */
  releaseMs?: number;
}

export interface WebAudioMixerOptions {
  /** One input per channel id: a stream, a media element or an audio node. */
  inputs: Record<string, AnalyserInput | undefined>;
  /** Lower some channels while another is active. */
  ducking?: DuckingOptions | DuckingOptions[];
  /** A peak limiter on the master output. Default true. */
  limiter?: boolean;
  /** Analysis settings for the channel and master meters. */
  analyser?: Pick<
    AnalyserTapOptions,
    "fftSize" | "bands" | "historySize" | "smoothing"
  >;
  /** Build the graph. Default true. */
  enabled?: boolean;
}

export interface WebAudioMixerGraph {
  /** Post-fader meter per channel id. */
  meters: Record<string, FrameSource<MeterFrame>>;
  /** Post-fader visual frames per channel id. */
  visuals: Record<string, FrameSource<VisualFrame>>;
  master: { meter: FrameSource<MeterFrame>; visual: FrameSource<VisualFrame> };
  /** The mix as a stream, for recording or streaming. */
  output: MediaStream | null;
  /** The master bus input, for routing your own nodes into the mix. */
  destination: AudioNode | null;
  context: AudioContext | null;
}

interface Relays {
  meter: FrameRelay<MeterFrame>;
  visual: FrameRelay<VisualFrame>;
}

interface Core {
  context: AudioContext;
  masterGain: GainNode;
  monitorGain: GainNode;
  output: MediaStreamAudioDestinationNode;
  tap: AnalyserTap;
  dispose: () => void;
}

interface Strip {
  input: Exclude<AnalyserInput, null>;
  gain: GainNode;
  duck: GainNode;
  panner: StereoPannerNode;
  monitorSend: GainNode;
  tap: AnalyserTap;
  dispose: () => void;
}

interface Snapshot {
  context: AudioContext | null;
  destination: AudioNode | null;
  output: MediaStream | null;
}

const EMPTY_SNAPSHOT: Snapshot = {
  context: null,
  destination: null,
  output: null,
};

const objectIds = new WeakMap<object, number>();
let nextObjectId = 1;
const idOf = (value: object) => {
  let id = objectIds.get(value);
  if (id === undefined) {
    id = nextObjectId;
    nextObjectId += 1;
    objectIds.set(value, id);
  }
  return id;
};

const ramp = (param: AudioParam, value: number, context: BaseAudioContext) => {
  param.setTargetAtTime(value, context.currentTime, RAMP_SECONDS);
};

const createLimiter = (context: AudioContext) => {
  const limiter = context.createDynamicsCompressor();
  limiter.threshold.value = -1;
  limiter.knee.value = 0;
  limiter.ratio.value = 20;
  limiter.attack.value = 0.001;
  limiter.release.value = 0.05;
  return limiter;
};

const buildCore = (
  context: AudioContext,
  limiterEnabled: boolean,
  analyser: AnalyserTapOptions
): Core => {
  const masterGain = context.createGain();
  // Built silent; apply() fades in to the mixer's level.
  masterGain.gain.value = 0;
  const output = context.createMediaStreamDestination();
  const limiter = limiterEnabled ? createLimiter(context) : null;
  let last: AudioNode = masterGain;
  if (limiter) {
    masterGain.connect(limiter);
    last = limiter;
  }
  last.connect(output);
  const monitorGain = context.createGain();
  monitorGain.gain.value = 0;
  monitorGain.connect(context.destination);
  const tap = createAnalyserTap(context, last, {
    ...analyser,
    channels: "stereo",
  });

  return {
    context,
    dispose: () => {
      tap.dispose();
      masterGain.disconnect();
      limiter?.disconnect();
      monitorGain.disconnect();
    },
    masterGain,
    monitorGain,
    output,
    tap,
  };
};

const buildStrip = (
  core: Core,
  input: Exclude<AnalyserInput, null>,
  analyser: AnalyserTapOptions
): Strip => {
  const { context } = core;
  const { node, owned } = createInputNode(context, input);
  const takesOverElement = input instanceof HTMLMediaElement;
  if (takesOverElement) {
    disconnectFrom(node, context.destination);
  }
  const gain = context.createGain();
  const duck = context.createGain();
  const panner = context.createStereoPanner();
  const monitorSend = context.createGain();
  // Built silent, so a muted or soloed-out channel never leaks at unity
  // before apply() ramps it to its level.
  gain.gain.value = 0;
  monitorSend.gain.value = 0;
  node.connect(gain);
  gain.connect(duck);
  duck.connect(panner);
  panner.connect(core.masterGain);
  panner.connect(monitorSend);
  monitorSend.connect(core.monitorGain);
  const tap = createAnalyserTap(context, panner, {
    ...analyser,
    channels: "stereo",
  });

  return {
    dispose: () => {
      tap.dispose();
      // The input's owner may have disconnected it already.
      disconnectFrom(node, gain);
      gain.disconnect();
      duck.disconnect();
      panner.disconnect();
      monitorSend.disconnect();
      if (owned) {
        node.disconnect();
      }
      if (takesOverElement) {
        node.connect(context.destination);
      }
    },
    duck,
    gain,
    input,
    monitorSend,
    panner,
    tap,
  };
};

const loudestPeak = (frame: MeterFrame) => {
  let loudest = Number.NEGATIVE_INFINITY;
  for (const level of frame.channels) {
    loudest = Math.max(loudest, level.peakDb);
  }
  return loudest;
};

/**
 * Owns the Web Audio graph outside React. The hook drives it from effects and
 * reads its snapshot with `useSyncExternalStore`.
 */
const createMixerGraph = () => {
  const relays = new Map<string, Relays>();
  const strips = new Map<string, Strip>();
  const master: Relays = {
    meter: createFrameRelay<MeterFrame>(),
    visual: createFrameRelay<VisualFrame>(),
  };
  const listeners = new Set<() => void>();
  let core: Core | null = null;
  let analyser: AnalyserTapOptions = {};
  let state: MixerState | null = null;
  let lastInputs: Record<string, AnalyserInput | undefined> = {};
  let snapshot: Snapshot = EMPTY_SNAPSHOT;

  const notify = () => {
    snapshot = core
      ? {
          context: core.context,
          destination: core.masterGain,
          output: core.output.stream,
        }
      : EMPTY_SNAPSHOT;
    for (const listener of listeners) {
      listener();
    }
  };

  const relaysFor = (id: string): Relays => {
    let entry = relays.get(id);
    if (!entry) {
      entry = {
        meter: createFrameRelay<MeterFrame>(),
        visual: createFrameRelay<VisualFrame>(),
      };
      relays.set(id, entry);
    }
    return entry;
  };

  const removeStrip = (id: string) => {
    strips.get(id)?.dispose();
    strips.delete(id);
    const entry = relays.get(id);
    entry?.meter.setSource(null);
    entry?.visual.setSource(null);
  };

  const apply = () => {
    if (!(core && state)) {
      return;
    }
    for (const channel of state.channels) {
      const strip = strips.get(channel.id);
      if (strip) {
        const audible = isChannelAudible(state, channel.id);
        ramp(
          strip.gain.gain,
          audible ? dbToGain(channel.gainDb) : 0,
          core.context
        );
        ramp(strip.panner.pan, channel.pan, core.context);
        ramp(strip.monitorSend.gain, channel.monitor ? 1 : 0, core.context);
      }
    }
    const level = state.master.muted ? 0 : dbToGain(state.master.gainDb);
    ramp(core.masterGain.gain, level, core.context);
    ramp(core.monitorGain.gain, level, core.context);
  };

  const reconcile = (inputs: Record<string, AnalyserInput | undefined>) => {
    lastInputs = inputs;
    if (!core) {
      return;
    }
    for (const [id, strip] of strips) {
      if (inputs[id] !== strip.input) {
        removeStrip(id);
      }
    }
    for (const [id, input] of Object.entries(inputs)) {
      if (input && !strips.has(id)) {
        const strip = buildStrip(core, input, analyser);
        strips.set(id, strip);
        const entry = relaysFor(id);
        entry.meter.setSource(strip.tap.meter);
        entry.visual.setSource(strip.tap.visual);
      }
    }
    apply();
  };

  const duck = (rules: DuckingOptions[]) => {
    const unsubscribers = rules.map((rule) => {
      const {
        amountDb = -12,
        attackMs = 50,
        releaseMs = 400,
        targets,
        thresholdDb = -35,
        trigger,
      } = rule;
      const attack = attackMs / MS_PER_SECOND / TIME_CONSTANTS_PER_RAMP;
      const release = releaseMs / MS_PER_SECOND / TIME_CONSTANTS_PER_RAMP;
      return relaysFor(trigger).meter.subscribe((frame) => {
        if (!core || loudestPeak(frame) < thresholdDb) {
          return;
        }
        // Each loud frame ducks and schedules its own release. The duck
        // holds while the trigger is loud, lets go when it goes quiet or is
        // removed, and reaches strips built mid-duck on the next frame.
        const now = core.context.currentTime;
        for (const target of targets) {
          const param = strips.get(target)?.duck.gain;
          if (param) {
            param.cancelScheduledValues(now);
            param.setTargetAtTime(dbToGain(amountDb), now, attack);
            param.setTargetAtTime(1, now + DUCK_HOLD_SECONDS, release);
          }
        }
      });
    });
    return () => {
      for (const unsubscribe of unsubscribers) {
        unsubscribe();
      }
      if (core) {
        for (const strip of strips.values()) {
          ramp(strip.duck.gain, 1, core.context);
        }
      }
    };
  };

  return {
    apply: (next: MixerState) => {
      state = next;
      apply();
    },
    duck,
    getSnapshot: () => snapshot,
    master,
    reconcile,
    relaysFor,
    start: (
      context: AudioContext,
      options: { limiter: boolean; analyser: AnalyserTapOptions }
    ) => {
      ({ analyser } = options);
      core = buildCore(context, options.limiter, analyser);
      master.meter.setSource(core.tap.meter);
      master.visual.setSource(core.tap.visual);
      reconcile(lastInputs);
      notify();
    },
    stop: () => {
      for (const id of strips.keys()) {
        removeStrip(id);
      }
      master.meter.setSource(null);
      master.visual.setSource(null);
      core?.dispose();
      core = null;
      notify();
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
};

const getServerSnapshot = () => EMPTY_SNAPSHOT;

/**
 * Builds a Web Audio graph from mixer state: a gain, ducking stage and panner
 * per channel, a master bus with an optional limiter, per-channel monitor
 * sends to the speakers, and post-fader meters everywhere.
 */
export const useWebAudioMixer = (
  mixer: Mixer,
  {
    inputs,
    ducking,
    limiter = true,
    analyser,
    enabled = true,
  }: WebAudioMixerOptions
): WebAudioMixerGraph => {
  const { context } = useAudioContext();
  const graph = useMemo(() => createMixerGraph(), []);
  const snapshot = useSyncExternalStore(
    graph.subscribe,
    graph.getSnapshot,
    getServerSnapshot
  );
  const analyserKey = JSON.stringify(analyser ?? {});
  const duckingKey = JSON.stringify(ducking ?? []);
  const channelIds = mixer.channels.map((channel) => channel.id);
  const idsKey = channelIds.join("|");
  const inputsKey = channelIds
    .map((id) => {
      const input = inputs[id];
      return `${id}:${input ? idOf(input) : 0}`;
    })
    .join("|");

  // Inputs are matched by identity through inputsKey; the nodes themselves
  // are read when the graph reconciles.
  const readInputs = useEffectEvent(() => inputs);

  useEffect(() => {
    if (!(context && enabled)) {
      return;
    }
    graph.start(context, {
      analyser: JSON.parse(analyserKey) as AnalyserTapOptions,
      limiter,
    });
    return () => {
      graph.stop();
    };
  }, [analyserKey, context, enabled, graph, limiter]);

  useEffect(() => {
    const latest = readInputs();
    const wanted: Record<string, AnalyserInput | undefined> = {};
    for (const entry of inputsKey.split("|")) {
      const [id] = entry.split(":");
      if (id) {
        wanted[id] = latest[id];
      }
    }
    graph.reconcile(wanted);
  }, [graph, inputsKey]);

  useEffect(() => {
    graph.apply(mixer.state);
  }, [graph, mixer.state]);

  useEffect(() => {
    const parsed = JSON.parse(duckingKey) as DuckingOptions | DuckingOptions[];
    return graph.duck(Array.isArray(parsed) ? parsed : [parsed]);
  }, [duckingKey, graph]);

  const sources = useMemo(() => {
    const meters: Record<string, FrameSource<MeterFrame>> = {};
    const visuals: Record<string, FrameSource<VisualFrame>> = {};
    for (const id of idsKey === "" ? [] : idsKey.split("|")) {
      const relays = graph.relaysFor(id);
      meters[id] = relays.meter;
      visuals[id] = relays.visual;
    }
    return { meters, visuals };
  }, [graph, idsKey]);

  return {
    context: snapshot.context,
    destination: snapshot.destination,
    master: graph.master,
    meters: sources.meters,
    output: snapshot.output,
    visuals: sources.visuals,
  };
};
