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

/** Docs only: synthesised demo tracks with object URLs. */
export const useDemoTracks = (): DemoTrackSource[] => {
  const [tracks, setTracks] = useState<DemoTrackSource[]>([]);

  useEffect(() => {
    let cancelled = false;
    Promise.all(
      DEMO_TRACKS.map(async (track) => ({
        ...track,
        src: await renderDemoTrackUrl(track.id),
      }))
    )
      .then((rendered) => {
        if (!cancelled) {
          setTracks(rendered);
        }
      })
      .catch(() => {
        // Demo audio needs OfflineAudioContext; without it the preview stays empty.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return tracks;
};

/** Docs only: synthesised demo sound effects as AudioBuffers. */
export const useDemoSounds = (): DemoSoundSource[] => {
  const [sounds, setSounds] = useState<DemoSoundSource[]>([]);

  useEffect(() => {
    let cancelled = false;
    Promise.all(
      DEMO_SOUNDS.map(async (sound) => ({
        ...sound,
        src: await renderDemoSound(sound.id),
      }))
    )
      .then((rendered) => {
        if (!cancelled) {
          setSounds(rendered);
        }
      })
      .catch(() => {
        // Demo audio needs OfflineAudioContext; without it the preview stays empty.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return sounds;
};
