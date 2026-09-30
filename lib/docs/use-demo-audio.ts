"use client";

import { useEffect, useState } from "react";

import {
  DEMO_SOUNDS,
  DEMO_TRACKS,
  renderDemoSound,
  renderDemoTrackUrl,
} from "@/lib/docs/demo-audio";

export interface DemoTrackSource {
  id: string;
  title: string;
  artist: string;
  duration: number;
  src: string;
}

export interface DemoSoundSource {
  id: string;
  label: string;
  hotkey: string;
  accent: string;
  src: AudioBuffer;
}

const renderTracks = () =>
  Promise.all(
    DEMO_TRACKS.map(async (track) => ({
      ...track,
      src: await renderDemoTrackUrl(track.id),
    }))
  );

const renderSounds = () =>
  Promise.all(
    DEMO_SOUNDS.map(async (sound) => ({
      ...sound,
      src: await renderDemoSound(sound.id),
    }))
  );

/** Runs `render` once on the client and keeps its result. */
const useRendered = <T>(render: () => Promise<T[]>): T[] => {
  const [items, setItems] = useState<T[]>([]);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const rendered = await render();
        if (!cancelled) {
          setItems(rendered);
        }
      } catch {
        // Demo audio needs OfflineAudioContext; without it the preview stays empty.
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [render]);

  return items;
};

/** Docs only: synthesised demo tracks with object URLs. */
export const useDemoTracks = (): DemoTrackSource[] => useRendered(renderTracks);

/** Docs only: synthesised demo sound effects as AudioBuffers. */
export const useDemoSounds = (): DemoSoundSource[] => useRendered(renderSounds);
