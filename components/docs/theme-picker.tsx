"use client";

import { useEffect, useSyncExternalStore } from "react";
import type { CSSProperties } from "react";

import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { THEMES, isTheme } from "@/lib/docs/site-themes";
import type { ThemeName } from "@/lib/docs/site-themes";

const STORAGE_KEY = "audiocn-theme";
const CHANGE_EVENT = "audiocn-theme-change";

const applyTheme = (theme: ThemeName) => {
  if (theme === "stone") {
    delete document.documentElement.dataset.theme;
  } else {
    document.documentElement.dataset.theme = theme;
  }
};

const readTheme = (): ThemeName => {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return isTheme(stored) ? stored : "stone";
  } catch {
    return "stone";
  }
};

const subscribe = (onChange: () => void) => {
  window.addEventListener(CHANGE_EVENT, onChange);
  window.addEventListener("storage", onChange);
  return () => {
    window.removeEventListener(CHANGE_EVENT, onChange);
    window.removeEventListener("storage", onChange);
  };
};

const getServerTheme = (): ThemeName => "stone";

const saveTheme = (theme: ThemeName) => {
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Not persisted; the theme still applies for this visit.
  }
  applyTheme(theme);
  window.dispatchEvent(new Event(CHANGE_EVENT));
};

/** The saved site theme, and a setter that saves and applies it everywhere. */
export const useSiteTheme = (): [ThemeName, (theme: ThemeName) => void] => {
  const theme = useSyncExternalStore(subscribe, readTheme, getServerTheme);

  // Whichever control is on the page applies the saved theme when it loads.
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  return [theme, saveTheme];
};

/** Docs only: switches the colour theme so every preview can be seen in it. */
export const ThemePicker = () => {
  const [theme, setTheme] = useSiteTheme();

  return (
    <Select
      items={THEMES.map(({ label, value }) => ({ label, value }))}
      onValueChange={(next) => {
        if (isTheme(next)) {
          setTheme(next);
        }
      }}
      value={theme}
    >
      <SelectTrigger aria-label="Theme" className="w-28" size="sm">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {THEMES.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              <span
                aria-hidden
                className="size-3 rounded-full bg-(--swatch)"
                style={{ "--swatch": option.swatch } as CSSProperties}
              />
              {option.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
};
