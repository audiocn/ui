/** The docs site's colour themes, in picker order. Stone is the default. */
export const THEMES = [
  { label: "Stone", swatch: "oklch(0.216 0.006 56.043)", value: "stone" },
  { label: "Ocean", swatch: "oklch(0.546 0.215 262.881)", value: "ocean" },
  { label: "Rose", swatch: "oklch(0.586 0.253 17.585)", value: "rose" },
  { label: "Forest", swatch: "oklch(0.627 0.194 149.214)", value: "forest" },
  { label: "Violet", swatch: "oklch(0.541 0.281 293.009)", value: "violet" },
  { label: "Mono", swatch: "oklch(0.556 0 0)", value: "mono" },
] as const;

export type ThemeName = (typeof THEMES)[number]["value"];

export const isTheme = (value: unknown): value is ThemeName =>
  THEMES.some((theme) => theme.value === value);
