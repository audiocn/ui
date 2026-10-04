"use client";

import { useEffect, useMemo } from "react";

import { createDemoSignal } from "@/hooks/use-demo-signal";
import type { DemoSignalOptions } from "@/hooks/use-demo-signal";
import { isChannelAudible } from "@/hooks/use-mixer";
import type { MixerState } from "@/hooks/use-mixer";
import { dbToGain, gainToDb } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, MeterFrame } from "@/lib/audio/types";

interface DemoMixerChannel extends DemoSignalOptions {
  id: string;
}

/** Docs-only metering: sum peak amplitudes and RMS powers, with mono on both sides. */
export const createDemoMixer = (channels: readonly DemoMixerChannel[]) => {
  const signals = channels.map(({ id, ...options }) => ({
    id,
    signal: createDemoSignal(options),
  }));
  let masterGain = 1;
  const subscribers = new Set<(frame: MeterFrame) => void>();
  let release: (() => void) | undefined;

  const master: FrameSource<MeterFrame> = {
    subscribe: (listener) => {
      subscribers.add(listener);
      if (!release) {
        const frames = new Map<string, MeterFrame>();
        const unsubscribes = signals.map(({ id, signal }) =>
          signal.meter.subscribe((frame) => frames.set(id, frame))
        );
        // All inputs update first, so each mix uses the same animation frame.
        const stop = subscribeFrame(() => {
          const frame: MeterFrame = { channels: [] };
          for (let side = 0; side < 2; side += 1) {
            let peak = 0;
            let power = 0;
            for (const input of frames.values()) {
              const level =
                input.channels[input.channels.length === 1 ? 0 : side];
              if (level) {
                peak += dbToGain(level.peakDb);
                power += dbToGain(level.rmsDb ?? level.peakDb) ** 2;
              }
            }
            frame.channels.push({
              peakDb: gainToDb(peak * masterGain),
              rmsDb: gainToDb(Math.sqrt(power) * masterGain),
            });
          }
          for (const subscriber of subscribers) {
            subscriber(frame);
          }
        }, "update");
        release = () => {
          stop();
          for (const unsubscribe of unsubscribes) {
            unsubscribe();
          }
        };
      }
      return () => {
        subscribers.delete(listener);
        if (subscribers.size === 0) {
          release?.();
          release = undefined;
        }
      };
    },
  };

  return {
    configure: (state: MixerState) => {
      for (const { id, signal } of signals) {
        const channel = state.channels.find((item) => item.id === id);
        signal.configure({
          gainDb: channel?.gainDb ?? 0,
          playing: isChannelAudible(state, id),
        });
      }
      masterGain = state.master.muted ? 0 : dbToGain(state.master.gainDb);
    },
    master,
    sources: Object.fromEntries(
      signals.map(({ id, signal }) => [id, signal.meter])
    ),
  };
};

/** Keep channel definitions stable; gain, mute and solo follow mixer state. */
export const useDemoMixer = (
  channels: readonly DemoMixerChannel[],
  state: MixerState
) => {
  const demo = useMemo(() => createDemoMixer(channels), [channels]);
  useEffect(() => {
    demo.configure(state);
  }, [demo, state]);
  return demo;
};
