"use client";

import { WebThreads } from "@/components/home/web-threads";
import { useReducedMotion } from "@/hooks/use-reduced-motion";

/**
 * Woven threads behind the hero, still under reduced motion. Light mode
 * inverts the dark render: over white that gives dark threads exactly as
 * strong as the light ones on the dark page, with no grey veil between them.
 */
export const HeroThreads = () => {
  const reducedMotion = useReducedMotion();

  return (
    <WebThreads
      brightness={0.55}
      className="invert dark:invert-0"
      color1="#ffffff"
      color2="#666666"
      color3="#ffffff"
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
