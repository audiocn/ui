"use client";

import {
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from "react";

import { createInputNode, disconnectFrom } from "@/hooks/use-audio-analyser";
import type { AnalyserInput } from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import type { AudioContextStatus } from "@/hooks/use-audio-context";
import { VU_BALLISTICS, VU_SCALE } from "@/lib/audio/needle";
import type { NeedleOptions, NeedleScale } from "@/lib/audio/needle";
import { loadVuProcessor, VU_PROCESSOR_NAME } from "@/lib/audio/vu-worklet";

export interface VuNeedleOptions
  extends Partial<NeedleOptions>, Partial<NeedleScale> {
  /** Which channel moves the needle, or the louder of the two. Default `max`. */
  channel?: 0 | 1 | "max";
  /** How often the audio thread sends the position, in ms. Default 1000/120. */
  intervalMs?: number;
}

export type VuNeedleStatus =
  | "idle"
  | "loading"
  | "running"
  | "suspended"
  | "error";

export interface VuNeedle {
  status: VuNeedleStatus;
  error: Error | null;
  /** Puts the needle back at rest. */
  reset: () => void;
}

const DEFAULT_INTERVAL_MS = 1000 / 120;

/** Context states the needle can't recover from by itself. */
const CONTEXT_ERRORS: Partial<Record<AudioContextStatus, Error>> = {
  closed: new Error("The audio context is closed."),
  unsupported: new Error("Web Audio is unavailable in this browser."),
};

type NeedleParams = NeedleOptions & NeedleScale;

/**
 * Runs a VU needle on the audio thread: |x| → resonant low-pass → stops,
 * every sample. Calls `onPosition` with the scale position (0 and 1 are the
 * ends of the arc) several times per frame, outside React.
 */
export const useVuNeedle = (
  input: AnalyserInput,
  onPosition: (position: number) => void,
  {
    channel = "max",
    intervalMs = DEFAULT_INTERVAL_MS,
    lowPassCutoffHz = VU_BALLISTICS.lowPassCutoffHz,
    lowPassQ = VU_BALLISTICS.lowPassQ,
    rebound = VU_BALLISTICS.rebound,
    referenceDb = VU_SCALE.referenceDb,
    minDb = VU_SCALE.minDb,
    maxDb = VU_SCALE.maxDb,
  }: VuNeedleOptions = {}
): VuNeedle => {
  const { context, status: contextStatus } = useAudioContext();
  const nodeRef = useRef<AudioWorkletNode | null>(null);
  const params = useMemo<NeedleParams>(
    () => ({ lowPassCutoffHz, lowPassQ, maxDb, minDb, rebound, referenceDb }),
    [lowPassCutoffHz, lowPassQ, maxDb, minDb, rebound, referenceDb]
  );
  const connection = useMemo(
    () => ({ channel, context, input, intervalMs }),
    [channel, context, input, intervalMs]
  );
  const [result, setResult] = useState<{
    connection: typeof connection;
    error: Error | null;
  } | null>(null);
  const emit = useEffectEvent(onPosition);
  const currentParams = useEffectEvent(() => params);

  useEffect(() => {
    if (!(context && input)) {
      return;
    }
    let cancelled = false;
    let node: AudioWorkletNode | null = null;
    let source: { node: AudioNode; owned: boolean } | null = null;
    const listeners = new AbortController();

    const release = () => {
      listeners.abort();
      if (node) {
        node.port.postMessage("stop", []);
        node.port.close();
        node.disconnect();
        if (source) {
          disconnectFrom(source.node, node);
        }
      }
      if (source?.owned) {
        source.node.disconnect();
      }
      if (nodeRef.current === node) {
        nodeRef.current = null;
      }
      node = null;
      source = null;
    };

    const connect = async () => {
      try {
        await loadVuProcessor(context);
        if (cancelled) {
          return;
        }
        source = createInputNode(context, input);
        node = new AudioWorkletNode(context, VU_PROCESSOR_NAME, {
          channelCount: 2,
          channelCountMode: "explicit",
          channelInterpretation: "speakers",
          numberOfInputs: 1,
          numberOfOutputs: 1,
          outputChannelCount: [1],
          parameterData: { ...currentParams() },
          processorOptions: {
            channel,
            intervalMs:
              Number.isFinite(intervalMs) && intervalMs > 0
                ? intervalMs
                : DEFAULT_INTERVAL_MS,
          },
        });
        node.addEventListener(
          "processorerror",
          () => {
            release();
            setResult({
              connection,
              error: new Error("The VU needle stopped on the audio thread."),
            });
          },
          { signal: listeners.signal }
        );
        node.port.addEventListener(
          "message",
          (event: MessageEvent<number>) => emit(event.data),
          { signal: listeners.signal }
        );
        node.port.start();
        source.node.connect(node);
        // The output is silent; connecting it keeps the processor running.
        node.connect(context.destination);
        nodeRef.current = node;
        setResult({ connection, error: null });
      } catch (error) {
        release();
        if (!cancelled) {
          setResult({
            connection,
            error: error instanceof Error ? error : new Error(String(error)),
          });
        }
      }
    };

    connect();
    return () => {
      cancelled = true;
      release();
    };
  }, [channel, connection, context, input, intervalMs]);

  useEffect(() => {
    const node = nodeRef.current;
    if (!(node && context)) {
      return;
    }
    for (const [name, value] of Object.entries(params)) {
      node.parameters.get(name)?.setValueAtTime(value, context.currentTime);
    }
  }, [context, params]);

  const reset = useCallback(() => {
    nodeRef.current?.port.postMessage("reset", []);
  }, []);

  const current = result?.connection === connection ? result : null;
  if (!input) {
    return { error: null, reset, status: "idle" };
  }
  const contextError = CONTEXT_ERRORS[contextStatus];
  if (contextError) {
    return { error: contextError, reset, status: "error" };
  }
  if (current?.error) {
    return { error: current.error, reset, status: "error" };
  }
  if (!current) {
    return { error: null, reset, status: "loading" };
  }
  return {
    error: null,
    reset,
    status: contextStatus === "running" ? "running" : "suspended",
  };
};
