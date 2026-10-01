"use client";

import { useEffect, useMemo, useRef } from "react";

import { useAudioContext } from "@/hooks/use-audio-context";

const DEFAULT_TIME_CONSTANT = 0.01;

export interface UseGainNodeOptions {
  /**
   * What feeds the node: a `MediaStream` (wrapped in a source node for you)
   * or any `AudioNode`.
   */
  input?: MediaStream | AudioNode | null;
  /** Linear gain, ramped so changes never click. Default 1. */
  gain?: number;
  /**
   * Where the node plays. Leave it out for the speakers, pass `null` to
   * route it yourself, or pass a node such as a mixer input.
   */
  destination?: AudioNode | null;
  /** Ramp time constant, in seconds. Default 0.01. */
  timeConstant?: number;
}

const disconnectFrom = (node: AudioNode, target: AudioNode) => {
  try {
    node.disconnect(target);
  } catch {
    // Already disconnected, for example by the node's owner.
  }
};

/**
 * A gain node on the shared AudioContext, wired from `input` to
 * `destination`. Returns null on the server and before the context exists.
 */
export const useGainNode = ({
  input,
  gain = 1,
  destination,
  timeConstant = DEFAULT_TIME_CONSTANT,
}: UseGainNodeOptions = {}): GainNode | null => {
  const { context } = useAudioContext();
  const node = useMemo(() => context?.createGain() ?? null, [context]);
  // Nodes that have had their first gain, which is set, not ramped, so a
  // muted node is never heard at unity while it starts.
  const startedRef = useRef(new WeakSet<GainNode>());

  useEffect(() => {
    if (!(context && node)) {
      return;
    }
    if (startedRef.current.has(node)) {
      node.gain.setTargetAtTime(gain, context.currentTime, timeConstant);
    } else {
      startedRef.current.add(node);
      node.gain.setValueAtTime(gain, context.currentTime);
    }
  }, [context, gain, node, timeConstant]);

  useEffect(() => {
    if (!(context && node && input)) {
      return;
    }
    const owned = input instanceof MediaStream;
    const source = owned ? context.createMediaStreamSource(input) : input;
    source.connect(node);
    return () => {
      if (owned) {
        source.disconnect();
      } else {
        disconnectFrom(source, node);
      }
    };
  }, [context, input, node]);

  useEffect(() => {
    if (!(context && node)) {
      return;
    }
    const target =
      destination === undefined ? context.destination : destination;
    if (!target) {
      return;
    }
    node.connect(target);
    return () => {
      disconnectFrom(node, target);
    };
  }, [context, destination, node]);

  return node;
};
