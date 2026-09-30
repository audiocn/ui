"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import { createAnalyserTap, createInputNode } from "@/hooks/use-audio-analyser";
import type {
  AnalyserInput,
  AnalyserTap,
  AnalyserTapOptions,
} from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import { isChannelAudible } from "@/hooks/use-mixer";
import type { Mixer } from "@/hooks/use-mixer";
import { dbToGain } from "@/lib/audio/decibels";
import { createFrameRelay } from "@/lib/audio/frame-source";
import type { FrameRelay } from "@/lib/audio/frame-source";
import type { FrameSource, MeterFrame, VisualFrame } from "@/lib/audio/types";

const RAMP_SECONDS = 0.005;
const MS_PER_SECOND = 1000;

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
  limiter: DynamicsCompressorNode | null;
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

const buildCore = (
  context: AudioContext,
  limiterEnabled: boolean,
  analyser: AnalyserTapOptions
): Core => {
  const masterGain = context.createGain();
  const output = context.createMediaStreamDestination();
  let limiter: DynamicsCompressorNode | null = null;
  let last: AudioNode = masterGain;
  if (limiterEnabled) {
    limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -1;
    limiter.knee.value = 0;
    limiter.ratio.value = 20;
    limiter.attack.value = 0.001;
    limiter.release.value = 0.05;
    masterGain.connect(limiter);
    last = limiter;
  }
  last.connect(output);
  const monitorGain = context.createGain();
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
    limiter,
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
    try {
      node.disconnect(context.destination);
    } catch {
      // Already routed elsewhere.
    }
  }
  const gain = context.createGain();
  const duck = context.createGain();
  const panner = context.createStereoPanner();
  const monitorSend = context.createGain();
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
      node.disconnect(gain);
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

const relaysFor = (map: Map<string, Relays>, id: string): Relays => {
  let relays = map.get(id);
  if (!relays) {
    relays = {
      meter: createFrameRelay<MeterFrame>(),
      visual: createFrameRelay<VisualFrame>(),
    };
    map.set(id, relays);
  }
  return relays;
};

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
    analyser = {},
    enabled = true,
  }: WebAudioMixerOptions
): WebAudioMixerGraph => {
  const { context } = useAudioContext();
  const relaysRef = useRef(new Map<string, Relays>());
  const [masterRelays] = useState<Relays>(() => ({
    meter: createFrameRelay<MeterFrame>(),
    visual: createFrameRelay<VisualFrame>(),
  }));
  const [core, setCore] = useState<Core | null>(null);
  const stripsRef = useRef(new Map<string, Strip>());
  const [stripsVersion, setStripsVersion] = useState(0);
  const analyserKey = JSON.stringify(analyser);

  const channelIds = mixer.channels.map((channel) => channel.id);
  const idsKey = channelIds.join("|");

  const inputsKey = channelIds
    .map((id) => {
      const input = inputs[id];
      return `${id}:${input ? idOf(input) : 0}`;
    })
    .join("|");

  const inputsRef = useRef(inputs);
  inputsRef.current = inputs;

  useEffect(() => {
    if (!(context && enabled)) {
      return;
    }
    const built = buildCore(
      context,
      limiter,
      JSON.parse(analyserKey) as AnalyserTapOptions
    );
    masterRelays.meter.setSource(built.tap.meter);
    masterRelays.visual.setSource(built.tap.visual);
    setCore(built);
    const strips = stripsRef.current;
    return () => {
      for (const [id, strip] of strips) {
        strip.dispose();
        const relays = relaysRef.current.get(id);
        relays?.meter.setSource(null);
        relays?.visual.setSource(null);
      }
      strips.clear();
      masterRelays.meter.setSource(null);
      masterRelays.visual.setSource(null);
      built.dispose();
      setCore(null);
    };
  }, [analyserKey, context, enabled, limiter, masterRelays]);

  useEffect(() => {
    if (!core) {
      return;
    }
    const strips = stripsRef.current;
    const wanted = new Set(idsKey === "" ? [] : idsKey.split("|"));
    let changed = false;

    for (const [id, strip] of strips) {
      const input = inputsRef.current[id];
      if (!wanted.has(id) || input !== strip.input) {
        strip.dispose();
        strips.delete(id);
        const relays = relaysRef.current.get(id);
        relays?.meter.setSource(null);
        relays?.visual.setSource(null);
        changed = true;
      }
    }
    for (const id of wanted) {
      const input = inputsRef.current[id];
      if (input && !strips.has(id)) {
        const strip = buildStrip(
          core,
          input,
          JSON.parse(analyserKey) as AnalyserTapOptions
        );
        strips.set(id, strip);
        const relays = relaysFor(relaysRef.current, id);
        relays.meter.setSource(strip.tap.meter);
        relays.visual.setSource(strip.tap.visual);
        changed = true;
      }
    }
    if (changed) {
      setStripsVersion((version) => version + 1);
    }
  }, [analyserKey, core, idsKey, inputsKey]);

  useEffect(() => {
    if (!core) {
      return;
    }
    const { state } = mixer;
    for (const channel of state.channels) {
      const strip = stripsRef.current.get(channel.id);
      if (!strip) {
        continue;
      }
      const audible = isChannelAudible(state, channel.id);
      ramp(
        strip.gain.gain,
        audible ? dbToGain(channel.gainDb) : 0,
        core.context
      );
      ramp(strip.panner.pan, channel.pan, core.context);
      ramp(strip.monitorSend.gain, channel.monitor ? 1 : 0, core.context);
    }
    const masterLevel = state.master.muted ? 0 : dbToGain(state.master.gainDb);
    ramp(core.masterGain.gain, masterLevel, core.context);
    ramp(core.monitorGain.gain, masterLevel, core.context);
  }, [core, mixer, stripsVersion]);

  const duckingKey = JSON.stringify(ducking ?? null);

  useEffect(() => {
    if (!core) {
      return;
    }
    const rules =
      (JSON.parse(duckingKey) as DuckingOptions | DuckingOptions[] | null) ??
      [];
    const list = Array.isArray(rules) ? rules : [rules];
    const unsubscribers: (() => void)[] = [];

    for (const rule of list) {
      const {
        trigger,
        targets,
        thresholdDb = -35,
        amountDb = -12,
        attackMs = 50,
        releaseMs = 400,
      } = rule;
      const relays = relaysRef.current.get(trigger);
      if (!relays) {
        continue;
      }
      let ducked = false;
      unsubscribers.push(
        relays.meter.subscribe((frame) => {
          let loudest = Number.NEGATIVE_INFINITY;
          for (const level of frame.channels) {
            loudest = Math.max(loudest, level.peakDb);
          }
          const active = loudest >= thresholdDb;
          if (active === ducked) {
            return;
          }
          ducked = active;
          const timeConstant =
            (active ? attackMs : releaseMs) / MS_PER_SECOND / 3;
          for (const target of targets) {
            const strip = stripsRef.current.get(target);
            strip?.duck.gain.setTargetAtTime(
              active ? dbToGain(amountDb) : 1,
              core.context.currentTime,
              timeConstant
            );
          }
        })
      );
    }

    return () => {
      for (const unsubscribe of unsubscribers) {
        unsubscribe();
      }
      for (const strip of stripsRef.current.values()) {
        strip.duck.gain.setTargetAtTime(
          1,
          core.context.currentTime,
          RAMP_SECONDS
        );
      }
    };
  }, [core, duckingKey, stripsVersion]);

  const sources = useMemo(() => {
    const meters: Record<string, FrameSource<MeterFrame>> = {};
    const visuals: Record<string, FrameSource<VisualFrame>> = {};
    for (const id of idsKey === "" ? [] : idsKey.split("|")) {
      const relays = relaysFor(relaysRef.current, id);
      meters[id] = relays.meter;
      visuals[id] = relays.visual;
    }
    return { meters, visuals };
  }, [idsKey]);

  return {
    context: core?.context ?? null,
    destination: core?.masterGain ?? null,
    master: masterRelays,
    meters: sources.meters,
    output: core?.output.stream ?? null,
    visuals: sources.visuals,
  };
};
