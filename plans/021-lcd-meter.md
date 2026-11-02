# 021 — LCD Meter

Status: IMPLEMENTED (2026-10-23). See [Outcome](#outcome). Follows plan 002; builds on `LevelMeter` (plan 004) and `LcdDisplay` (plan 020).

A level meter drawn on the LCD Display's screen, like the bargraph on a pocket recorder: one row of block segments per channel, unlit segments showing faintly, a held peak segment, a dB readout, a printed scale and a CLIP flag. It lives inside an `LcdDisplay`, so it shares that screen's ink, glass, dot mask and glow, and it can sit under settings rows on the same screen.

```
┌────────────────────────────────┐
│ INPUT               USB MIC    │
│ ······························ │
│ L ▮▮▮▮▮▮▮▮▮▮▮▯▯▯▯▯▯▯▮▯▯  -12.4 │
│ R ▮▮▮▮▮▮▮▮▯▯▯▯▯▯▯▯▯▮▯▯▯  -15.0 │
│   48  24  12  [6   3   0] CLIP▪│
└────────────────────────────────┘
```

## Decisions

- **Inside `LcdDisplay`, not a second screen.** One bezel, one set of screen props (`variant`, `appearance`, `size`, `matrix`, `scanlines`). `LcdMeter` throws outside a display, the way `LevelMeterBar` throws outside a `LevelMeter`, since it would have no ink to draw with.
- **Built on `LevelMeter`, not a new painter.** The metering is the same: sources, declarative values, the ref handle, ballistics, peak hold, the clip hold, ARIA throttling, sleeping when settled or off screen. `LcdMeter` renders a `LevelMeter` and swaps its parts for LCD ones that read `--meter-level`, `--meter-rms` and `--meter-hold`. The VU meter needed its own component because its physics differ. This one only looks different.
- **Block segments.** Each segment is 3 dots wide with a 1-dot gap. Both are variables, so a fine ladder (1 + 1) or a solid bar (gap 0) is one class.
- **One ink.** The meter is drawn in the display's ink, like a real LCD. A bracket printed over the clip zone and a CLIP flag that inverts during the clip hold take the place of zone colours. `data-zone` is still written for anyone who wants to colour it.
- **Every theme, both modes.** The meter sets no colours of its own. Every pixel is mixed from the display's variables, so it follows the theme's `--primary` or a fixed hardware ink, the light (backlit) and dark (glowing) appearances, and a pinned `appearance`, the same way the display does.

## Anatomy

```tsx
<LcdDisplay>
  <LcdDisplayRow>
    <LcdDisplayLabel>Input</LcdDisplayLabel>
    <LcdDisplayValue>USB Mic</LcdDisplayValue>
  </LcdDisplayRow>
  <LcdDisplaySeparator />
  <LcdMeter aria-label="Input level" source={analyser.meter}>
    <LcdMeterChannel index={0}>
      <LcdMeterLabel>L</LcdMeterLabel>
      <LcdMeterTrack>
        <LcdMeterBar />
        <LcdMeterHold />
      </LcdMeterTrack>
      <LcdMeterValue />
    </LcdMeterChannel>
    <LcdMeterChannel index={1}>{/* … */}</LcdMeterChannel>
    <LcdMeterScale />
    <LcdMeterClip />
  </LcdMeter>
  <LcdDisplayStatus />
</LcdDisplay>
```

With no children, `<LcdMeter source={…} />` renders the screen above: one row per channel (labelled L and R for two channels, 1, 2, 3… for more, unlabelled for one), each with a track, a bar, a held peak and a readout, then the scale with the CLIP flag at its end.

| Part | Built on | Notes |
| --- | --- | --- |
| `LcdMeter` | `LevelMeter` | `role="meter"`. Takes `LevelMeter`'s data and scale props: `source`, `peakDb`, `rmsDb`, `channels`, `channelCount`, `minDb`, `maxDb`, `zones`, `ballistics`, `taper`, `actionsRef`. No `variant`, `segments`, `orientation` or `size`: the look is fixed and the size comes from the display. A three-column grid: label, track, readout |
| `LcdMeterChannel` | `LevelMeterChannel` | One grid row (subgrid). Gives its `index` to the readout inside it |
| `LcdMeterLabel` | `span` | Dim ink, in a label column a whole number of dots wide |
| `LcdMeterTrack` | `div` | The unlit segments. Snaps its length to a whole number of segments |
| `LcdMeterBar` | `div` | The lit segments. `measure`: `peak` (default) or `rms`. Lights whole segments only |
| `LcdMeterHold` | `div` | One lit segment at the held peak. Hidden at silence |
| `LcdMeterValue` | `LevelMeterValue` | Lit ink, `tabular-nums`, no unit (`-12.4`, `−∞`). Shows its channel inside a channel, the loudest outside one. No change flicker: it changes four times a second |
| `LcdMeterScale` | `LevelMeterScale` | In the track column, as long as the snapped track. Dim ink, ticks at −48, −24, −12, −6, −3 and 0 (inside the range), printed without the minus sign (`48 24 12 6 3 0`) as recorder screens do. A tick above 0 keeps its plus sign. Prints a bracket over the clip zone |
| `LcdMeterClip` | `LevelMeterClip` | The text `CLIP` (children replace it). An unlit ghost until the clip hold is on, then inverted: lit background, glass-coloured text, like a highlighted select item |

Layering a `measure="rms"` bar under a dimmed `measure="peak"` bar gives a dual meter, as with `LevelMeterBar`.

## Look

- **Segments are whole dots.** `--lcd-meter-segment` (3 × `--lcd-display-pitch`), `--lcd-meter-gap` (1 × pitch) and `--lcd-meter-thickness` (3, 4 or 6 dots for the display's `sm`, `default` and `lg`) are lengths in the display's pitch. Their size is fixed and their count follows the width, so `LcdMeter` has no `segments` prop. Starting values; tune them in the browser.
- **Whole segments light, never part of one.** The bar's length is `round(up, level × track, segment + gap)` with CSS `round()`, so the first segment lights as soon as there is any signal and the bar steps a segment at a time. The held segment sits at `round(up, hold × track, period) − period`. The track itself is `round(down, …)` of its column, so the last segment is never cut. Clipping the bar with `clip-path` rather than a width keeps the frames paint-only.
- **Unlit segments** are `--lcd-meter-unlit`, the lit pixels at 14%: twice the glass's unlit dots (7%), so the ladder reads as a printed element with nothing lit.
- **Dots come from the display.** The meter sits inside the screen's dot mask and under its glow, so the segments break into dots and glow (dark) or cast a faint shadow (light) with no code of their own. When the display's mask is off (`matrix={false}`, for a dot-matrix font), the track masks itself with the same dot grid, so the segments stay pixels.
- **On the grid.** The display's mask starts at the screen's content box, so a segment edge must land between dots or it cuts a dot in half. The label column (`round(up, 1.5em, pitch)`), the column gaps, the row gaps and the meter's padding are all whole dots, and the screen's padding already is. If the visual check still shows cut dots (text rows above can end off the grid with a custom pitch), a layout effect measures the track's offset from the content box and nudges the meter onto the grid.
- **The clip bracket** is a 1-dot rule in dim ink over the clip zone (from the `clip` zone's `fromDb` to `maxDb`), with 1-dot ends, between the tracks and the numbers.
- **Reduced motion** is `LevelMeter`'s: instant ballistics, painted four times a second. The CLIP flag inverts without a transition.

## Theming

| Variable or attribute | Meaning |
| --- | --- |
| `--lcd-meter-segment` | A segment's width. Default 3 dots |
| `--lcd-meter-gap` | The gap between segments. Default 1 dot. `0px` gives a solid bar |
| `--lcd-meter-thickness` | A track's height. Default 3, 4 or 6 dots by display size |
| `--lcd-meter-unlit` | Unlit segments and the CLIP ghost. Default: the lit pixels at 14% |
| `--lcd-display-*` | Everything else: ink, lit and dim pixels, glass, glow, pitch, font |
| `--meter-level`, `--meter-rms`, `--meter-hold` | On each channel, 0..1, from `LevelMeter` |
| `data-zone` | On each channel and the root: `ok`, `warn` or `clip` |
| `data-active` | On a channel above the floor |
| `data-clipping` | On the root during the clip hold |

## Changes to existing components

1. **`lcd-display.tsx`** exports `useLcdDisplay()`: the display's settings (`appearance`, `matrix`, `size`, `variant`), or `null` outside a display. The context's default becomes `null`; the select's popup falls back to the defaults it uses today.
2. **`level-meter.tsx`** exports `useLevelMeter()` (it throws outside a meter, as it does inside the file today), and its context gains `zones` and `channelCount`, so a meter built on it can print the clip bracket and render one row per channel.

## Not in this plan

- **Vertical meters** (mixer-style columns). The scale, label and readout layout would all need a second arrangement. A follow-up if it's wanted.
- **Zone inks.** One ink by decision. `data-zone` leaves the door open.
- **A standalone bezel.** Wrap it in `LcdDisplay`.

## Delivery

1. The two exports above, with tests.
2. `components/ui/lcd-meter.tsx` and `components/ui/lcd-meter.test.tsx`: throws outside a display; default rows and labels for 1, 2 and 4 channels; the readout follows its channel; ticks print without the minus; the bracket starts at the clip zone; the CLIP flag follows `data-clipping`; the track masks itself when `matrix` is off; thickness follows the display's size.
3. Examples: `lcd-meter-demo` (the screen above, on `useDemoSignal({ kind: "music" })`, with the status pixel), `lcd-meter-inks` (amber, green and cyan, and a dark screen pinned on a light page), `lcd-meter-segments` (blocks, a fine ladder and a solid bar, from the variables), `lcd-meter-microphone` (live input, like the other meters' microphone examples).
4. `content/docs/components/lcd-meter.mdx` in the plan 002 template, with Feeding data. Links to it from the LCD Display and Level Meter pages.
5. Docs nav (Meters and visualizers, after Level Meter), the components index, the README table, a `registry.json` item (`registryDependencies`: `@audiocn/core`, `@audiocn/level-meter`, `@audiocn/lcd-display`) and a social card.
6. Add `/docs/components/lcd-meter` to `e2e/themes.spec.ts`, which loads it in all six themes, light and dark, at 320 and 1440 px.
7. Typecheck, `pnpm exec ultracite check`, unit tests, the registry build, and a visual check at 1× and 2× in every site theme (Stone, Ocean, Rose, Forest, Violet, Mono), light and dark, and with each hardware ink and a pinned `appearance`. Look for: segment edges between dots; unlit segments visible but clearly off (Mono's grey glass is the hardest case); the glow on a full bar in dark; the inverted CLIP flag in both appearances; the frame cost of two running meters under the glow filter in the Performance panel.

## Outcome

Built as planned, in `components/ui/lcd-meter.tsx`, with these departures:

- **The `audio` taper by default**, not `LevelMeter`'s `linear`. On a linear −60..0 scale, −60 to −24 took half the track: music sat pinned at the right end and the scale crowded (`12 6 0`, with `3` thinned out). The audio taper spreads `48 24 12 6 3 0` across the track, as in the chosen preview. `taper="linear"` is still a prop.
- **Thickness 5, 7 and 10 dots** (`sm`, `default`, `lg`), not 3, 4 and 6. A row is one line of text (14, 18 or 24 px), and the track is centred in it, so `(line − thickness) / 2` must be a whole number of dots. These are the values that fill the line and meet that.
- **The grid nudge is always on**, not a fallback. The CSS-only alignment held for the default rows (checked at 1× and 2×: each segment is exactly 3 × 7 dots), but a label wider than the default column (`MIC`, `OUT`) left the tracks 1.1 px off, and a row of odd height above the meter left them 1 px off. `useDotGrid` measures the first track against the screen's content box after every render and on resize, and writes `--lcd-meter-nudge-x` and `--lcd-meter-nudge-y`. It reads the pitch from the screen's computed `mask-size`, so with the mask off it does nothing.
- **The scale is `DbScale` directly**, not `LevelMeterScale`, which positions itself absolutely under the channels. Its tick marks are hidden: a 1 px mark at an arbitrary position cuts dots. Its height is rounded up to whole dots, so rows under the meter stay on the grid.
- **`useLcdDisplay()` returns the whole context** (`appearance`, `matrix`, `size`, `variant`, `scanlines`, `rootRef`), or `null`.
- **The readout's width** is `DbReadout`'s `ch` width × 1.2, for the screen's 0.12em tracking on a 0.6em mono digit. Without it, the column (and so the track) changed width between `−∞` and `−12.4`.
- **The demo** has an input select (Line In, USB Mic, Test Tone) that switches the demo signal, to show the meter sharing a screen with a select row. **The segments example** feeds one signal to all three meters, so the looks compare like for like, and the repo's lint wants static class strings.

Checks: typecheck, `ultracite check`, 460 unit tests, the registry build, React Doctor (no new findings), and the e2e specs that cover the new page and its neighbours. In the theme spec, 5 of 24 runs at 1440 px timed out on `data-theme` while the machine was saturated (load average 24, with a 9.5 s font request); all 5 passed when rerun serially. A visual check at 1× and 2× in all six site themes, light and dark, plus the four hardware inks, a pinned dark screen, `sm` and `lg`, Doto with `matrix={false}`, a range up to +6 and a layered RMS bar.

The social card was captured with `build-social-images.mjs`'s own capture code for that one card. The full `pnpm og:build` timed out waiting for its first card (`home`) three times, twice with nothing else running and once beside a warm server; the cause was not found. The same capture code, run against a capture server started by hand, renders every card it was given and reproduces the committed `home` card byte for byte. It also shows that the committed LCD Display card is stale: the screen matches, but its caption is the page's old description ("A dark dot-matrix screen…"), from before that PR made the screen backlit in the light theme. That card is left for its own change.
