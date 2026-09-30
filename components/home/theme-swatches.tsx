"use client";

import type { CSSProperties } from "react";

import { THEMES, useSiteTheme } from "@/components/docs/theme-picker";
import { Button } from "@/components/ui/button";

/** Retheme the whole showcase at once, in sync with the navbar picker. */
export const ThemeSwatches = () => {
  const [theme, setTheme] = useSiteTheme();

  return (
    <fieldset className="flex flex-wrap items-center gap-1">
      <legend className="sr-only">Theme</legend>
      {THEMES.map((option) => (
        <Button
          aria-pressed={theme === option.value}
          key={option.value}
          onClick={() => setTheme(option.value)}
          size="xs"
          variant={theme === option.value ? "outline" : "ghost"}
        >
          <span
            aria-hidden
            className="ring-foreground/15 size-2.5 rounded-full bg-(--swatch) ring-1"
            style={{ "--swatch": option.swatch } as CSSProperties}
          />
          {option.label}
        </Button>
      ))}
    </fieldset>
  );
};
