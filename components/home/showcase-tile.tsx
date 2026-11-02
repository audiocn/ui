"use client";

import { lazy, Suspense, useEffect, useRef, useState } from "react";
import type { ComponentType, LazyExoticComponent, ReactNode } from "react";

import { Skeleton } from "@/components/ui/skeleton";

interface Tile {
  Component: LazyExoticComponent<ComponentType>;
  /** As tall as the tile, so nothing shifts when it loads. */
  placeholder: ReactNode;
}

const TILES = {
  channel: {
    Component: lazy(() => import("@/components/home/tiles/channel-tile")),
    placeholder: <Skeleton className="h-34 w-full" />,
  },
  "compact-player": {
    Component: lazy(
      () => import("@/components/home/tiles/compact-player-tile")
    ),
    placeholder: <Skeleton className="h-10 w-full" />,
  },
  faders: {
    Component: lazy(() => import("@/components/home/tiles/faders-tile")),
    placeholder: <Skeleton className="h-62 w-full" />,
  },
  knobs: {
    Component: lazy(() => import("@/components/home/tiles/knobs-tile")),
    placeholder: <Skeleton className="h-23 w-full" />,
  },
  "live-waveform": {
    Component: lazy(() => import("@/components/home/tiles/live-waveform-tile")),
    placeholder: <Skeleton className="h-31 w-full" />,
  },
  meters: {
    Component: lazy(() => import("@/components/home/tiles/meters-tile")),
    placeholder: <Skeleton className="h-62 w-full" />,
  },
  mixer: {
    Component: lazy(() => import("@/components/home/tiles/mixer-tile")),
    placeholder: <Skeleton className="h-75 w-full" />,
  },
  music: {
    Component: lazy(() => import("@/components/home/tiles/music-tile")),
    placeholder: <Skeleton className="h-82 w-full @lg:h-40" />,
  },
  output: {
    Component: lazy(() => import("@/components/home/tiles/output-tile")),
    placeholder: <Skeleton className="h-19 w-full" />,
  },
  "sound-pads": {
    Component: lazy(() => import("@/components/home/tiles/sound-pads-tile")),
    placeholder: <Skeleton className="h-76 w-full @sm:h-50" />,
  },
  spectrum: {
    Component: lazy(() => import("@/components/home/tiles/spectrum-tile")),
    placeholder: <Skeleton className="h-32 w-full" />,
  },
  voice: {
    Component: lazy(() => import("@/components/home/tiles/voice-tile")),
    placeholder: <Skeleton className="h-40 w-full" />,
  },
  "volume-knob": {
    Component: lazy(() => import("@/components/home/tiles/volume-knob-tile")),
    placeholder: <Skeleton className="size-44" />,
  },
  waveform: {
    Component: lazy(() => import("@/components/home/tiles/waveform-tile")),
    placeholder: <Skeleton className="h-36 w-full" />,
  },
} satisfies Record<string, Tile>;

export type ShowcaseTileName = keyof typeof TILES;

/** Tiles start loading this far before they scroll into view. */
const PRELOAD_MARGIN = "200px";

interface ShowcaseTileProps {
  name: ShowcaseTileName;
}

/**
 * Loads a live tile when it nears the viewport, so signals and demo audio
 * below the fold cost nothing until the visitor scrolls to them.
 */
export const ShowcaseTile = ({ name }: ShowcaseTileProps) => {
  const ref = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);

  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setNear(true);
          observer.disconnect();
        }
      },
      { rootMargin: PRELOAD_MARGIN }
    );
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, []);

  const { Component, placeholder } = TILES[name];

  return (
    <div className="@container flex w-full min-w-0 justify-center" ref={ref}>
      {near ? (
        <Suspense fallback={placeholder}>
          <Component />
        </Suspense>
      ) : (
        placeholder
      )}
    </div>
  );
};
