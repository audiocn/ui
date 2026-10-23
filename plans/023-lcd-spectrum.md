# 023 — LCD Spectrum

Status: IMPLEMENTED (2026-10-23). Follows plans 004 and 020.

A spectrum analyzer drawn as LCD segments, so a screen can show a spectrum like a hardware panel. It is a larger sibling of the LCD VU Meter (plan 024) and the existing Spectrum (plan 004).

## Behaviour

- One column per band, each made of lit and unlit segments in the display's ink.
- Built on the Spectrum (plan 004): the same `minDb`, `maxDb`, `minHz`, `maxHz`, `scale` and `grid`, the same level and frequency axes, and the same defaults. `LcdSpectrumCanvas` replaces `SpectrumCanvas`.
- `mode="bar"` lights every segment up to the level; `mode="dot"` lights only the top one, like the LCD VU Meter (plan 024).
- `peakHold` is off by default, as on the Spectrum.
- A peak cap per column. It sticks at the top for `PEAK_HOLD_MS`, then falls, accelerating under a gravity of `12 × rows` rows per second squared, and fades as it reaches the bar. It never falls below the bar's level.
- The caps are painted before the bars, so a falling cap passes behind the segments.
- Under reduced motion the caps are hidden and the bars repaint only when the source changes.

## Delivery

1. `components/ui/lcd-spectrum.tsx` and `components/ui/lcd-spectrum.test.tsx`.
2. Example `lcd-spectrum-demo` and `content/docs/components/lcd-spectrum.mdx`.
3. Docs nav (Controls, after LCD Display), the components index, the README table and a `registry.json` item.
4. Typecheck, lint, unit tests, and a visual check in light and dark.
