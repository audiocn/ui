"use client";

import { useTheme } from "next-themes";

import { WebThreads } from "@/components/home/web-threads";
import { useReducedMotion } from "@/hooks/use-reduced-motion";

const DARK_THREADS = {
  color1: "#ffffff",
  color2: "#666666",
  color3: "#ffffff",
};

// Light mode paints the threads as ink over the page's own white.
const LIGHT_THREADS = {
  ...DARK_THREADS,
  backgroundColor: "#ffffff",
  lightMode: true,
};

/** Woven threads behind the hero, still under reduced motion. */
export const HeroThreads = () => {
  const { resolvedTheme } = useTheme();
  const reducedMotion = useReducedMotion();
  const theme = resolvedTheme === "light" ? LIGHT_THREADS : DARK_THREADS;

  return (
    <WebThreads
      {...theme}
      brightness={0.55}
      falloff={0.59}
      fanMode="center"
      frequency={10.5}
      glow={0.013}
      grain={false}
      grainIntensity={0.06}
      mirror
      mouseInteraction={false}
      mouseStrength={0.55}
      opacity={1}
      position={0.16}
      shimmer={false}
      speed={reducedMotion ? 0 : 0.1}
      spread={0.07}
      taper={1.5}
      thickness={1.05}
      threadCount={5}
    />
  );
};
