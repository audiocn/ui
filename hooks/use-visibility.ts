"use client";

import { useEffect, useRef } from "react";
import type { RefObject } from "react";

/**
 * Tracks whether an element is on screen, in a ref, so a painter can skip
 * frames nobody can see. Reading it never re-renders.
 */
export const useVisibility = (
  target: RefObject<Element | null>
): RefObject<boolean> => {
  const visibleRef = useRef(true);
  useEffect(() => {
    const element = target.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        visibleRef.current = entry.isIntersecting;
      }
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [target]);
  return visibleRef;
};
