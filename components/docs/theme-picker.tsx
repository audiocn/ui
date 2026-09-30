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

const STORAGE_KEY = "audiocn-theme";
const CHANGE_EVENT = "audiocn-theme-change";

export const THEMES = [
  { label: "Stone", swatch: "oklch(0.216 0.006 56.043)", value: "stone" },
  { label: "Ocean", swatch: "oklch(0.546 0.215 262.881)", value: "ocean" },
  { label: "Rose", swatch: "oklch(0.586 0.253 17.585)", value: "rose" },
  { label: "Mono", swatch: "oklch(0.556 0 0)", value: "mono" },
] as const;

export type ThemeName = (typeof THEMES)[number]["value"];

const isTheme = (value: unknown): value is ThemeName =>
  THEMES.some((theme) => theme.value === value);

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
export const useSiteTheme = (): [ThemeName, (theme: ThemeName) => void] => [
  useSyncExternalStore(subscribe, readTheme, getServerTheme),
  saveTheme,
];

/** Docs only: switches the colour theme so every preview can be seen in it. */
export const ThemePicker = () => {
  const [theme, setTheme] = useSiteTheme();

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

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
