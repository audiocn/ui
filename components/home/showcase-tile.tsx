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
  mixer: {
    Component: lazy(() => import("@/components/home/tiles/mixer-tile")),
    placeholder: <Skeleton className="h-75 w-full" />,
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
    <div className="flex w-full min-w-0 justify-center" ref={ref}>
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
