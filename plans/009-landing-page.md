# 009 — Landing page: a live component showcase

Status: IMPLEMENTED (2026-09-30). Replaced the split hero (`app/(home)/page.tsx` + `components/docs/hero-demo.tsx`). See "Changes made while building" at the end.

The landing page becomes two things: a centred hero that says what audiocn is, and a grid of live, playable components under it. No feature grid, no marketing sections. The components are the pitch.

References: 8bitcn.com (centred hero, a divider, then three masonry columns of live components) and ui.elevenlabs.io (the same shape with four columns, cards with light titles, a theme switch above the grid).

---

## Page structure

```
┌───────────────────────────── navbar (unchanged) ─────────────────────────────┐
│                                                                              │
│                  [ Audio components for shadcn/ui  → ]      pill             │
│                                                                              │
│                    Audio UI, mixed and mastered.            h1               │
│      Meters, faders, knobs, visualizers and a complete mixer for React.      │
│              Built the shadcn way, so you own every line.                    │
│                                                                              │
│                   [ Get started ]  [ Browse components ]                     │
│                 npx shadcn@latest add @audiocn/mixer  ⧉                      │
│                                                                              │
│ ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿ live scrolling waveform divider ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿ │
│                                                                              │
│ Everything below is live. Drag a fader,        Theme  ● Stone ● Ocean ● Rose │
│ turn a knob, hit a pad.                                                      │
│ ┌─────────┐ ┌─────────────────────────┐ ┌─────────┐                          │
│ │ VOICE   │ │ MIXER (console)         │ │ METER   │                          │
│ │ bars    │ │ Mic  Sys  Music  │ Mstr │ │ BRIDGE  │                          │
│ └─────────┘ │ ▮▮   ▮▮   ▮▮     │ ▮▮   │ │ ▮▮ ▮▮   │                          │
│ ┌─────────┐ │ ╪    ╪    ╪      │ ╪    │ │ scale   │                          │
│ │ KNOBS   │ │ M S  M S  M S    │      │ │ clip    │                          │
│ └─────────┘ └─────────────────────────┘ └─────────┘                          │
│ ┌─────────┐ ┌─────────────────────────┐ ┌─────────┐                          │
│ │SPECTRUM │ │ WAVEFORM + region       │ │ PAN+M/S │                          │
│ └─────────┘ └─────────────────────────┘ └─────────┘                          │
│ ┌─────────┐ ┌─────────────────────────┐ ┌─────────┐                          │
│ │ EQ      │ │ MUSIC PLAYER + tracks   │ │ OUTPUT  │                          │
│ │ sliders │ │                         │ │ vol+dev │                          │
│ └─────────┘ ├─────────────────────────┤ └─────────┘                          │
│             │ SOUND PADS 4×2          │ ┌─────────┐                          │
│             └─────────────────────────┘ │ PLAYER  │                          │
│                                          └─────────┘                          │
│                        [ Browse all 20 components → ]                        │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Hero

| Part | Content |
| --- | --- |
| Pill | "Audio components for shadcn/ui", links to `/docs/components` |
| h1 | **Audio UI, mixed and mastered.** `font-heading`, `text-5xl` → `lg:text-7xl`, `tracking-tight`, centred, `text-balance` |
| Subline | "Meters, faders, knobs, visualizers and a complete mixer for React. Built the shadcn way, so you own every line." `text-muted-foreground`, `max-w-2xl` |
| CTAs | "Get started" → `/docs` (primary). "Browse components" → `/docs/components` (outline) |
| Install | `npx shadcn@latest add @audiocn/mixer`, mono, with a copy button |
| Divider | A full-width `LiveWaveform` (`mode="scrolling"`, `aria-hidden`, `text-muted-foreground/40`, about `h-12`) on a demo speech signal. It stands in for 8bitcn's dashed rule. Static under reduced motion |

Other headlines, if the one above does not land:

- "Every fader, knob and meter your app needs."
- "Your app, now with a mixing desk."
- "Signal in. Interface out."

---

## The grid

### Layout

Three column stacks, not a strict bento. Our components have intrinsic heights (a mixer, a player, a pad grid); a bento with fixed row spans would force empty space or clipping. Both references use stacks for the same reason.

```tsx
<section className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
  <div className="flex flex-col gap-4 md:col-span-2 lg:order-2">
    {/* wide */}
  </div>
  <div className="flex flex-col gap-4 lg:order-1">{/* left */}</div>
  <div className="flex flex-col gap-4 lg:order-3">{/* right */}</div>
</section>
```

- **Mobile:** one column, wide stack first, so the mixer is the first thing under the hero.
- **Tablet:** the wide stack spans both columns on top; left and right stacks sit side by side below.
- **Desktop:** narrow | wide | narrow, with the wide column twice the width.
- The grid widens to `max-w-7xl`, so the narrow columns are about 296 px and the wide column about 608 px.
- Balance the column bottoms by eye once tiles render; move a tile between the narrow stacks if one runs long.

### Tiles

Every tile is live and needs no permission prompt. Signals come from `useDemoSignal`, sound from the rendered demo audio in `lib/docs/demo-audio.ts`.

| # | Stack | Tile | Built from | Links to |
| --- | --- | --- | --- | --- |
| 1 | wide | **Mixer**: Mic, System, Music strips plus Master, console orientation; mute and solo work through `useMixer` | `mixer-console`, `mixer-demo` | `/docs/components/mixer` |
| 2 | wide | **Waveform**: mirror variant, draggable region, a marker, a clip time readout | `waveform-regions` | `/docs/components/waveform` |
| 3 | wide | **Music player**: player with its track list | `music-player` block | `/docs/blocks/music-player` |
| 4 | wide | **Sound pads**: 4×2 grid, one-shot, hold and toggle modes, ring and bar progress | `sound-pad-grid` | `/docs/components/sound-pad` |
| 5 | left | **Voice**: bar visualizer and live waveform with Idle, Listening and Speaking chips (the ElevenLabs "agent orb" slot) | `bar-visualizer-states`, `live-waveform-variants` | `/docs/components/bar-visualizer` |
| 6 | left | **Knobs**: Gain, Pan, Low cut | `knob-demo` | `/docs/components/knob` |
| 7 | left | **Spectrum**: area variant on speech | `spectrum-variants` | `/docs/components/spectrum` |
| 8 | left | **EQ band**: parameter sliders for frequency, Q and gain | `parameter-slider-frequency` | `/docs/components/parameter-slider` |
| 9 | right | **Meter bridge**: two vertical stereo meters, vertical dB scale, latching clip lights, peak readouts | `level-meter-vertical`, `db-scale-vertical`, `clip-indicator-latching`, `db-readout-zones` | `/docs/components/level-meter` |
| 10 | right | **Channel**: pan control plus mute, solo and monitor toggles | `pan-control-demo`, `channel-toggle-variants` | `/docs/components/pan-control` |
| 11 | right | **Output**: volume control popover and a device select in its static states | `volume-control-popover`, `audio-device-select-states` | `/docs/components/volume-control` |
| 12 | right | **Compact player** | `audio-player-compact` | `/docs/components/audio-player` |

Tiles are composed for the landing page, not rendered from the docs examples: the examples carry `max-w-*` limits and doc-specific variants. Each tile borrows its example's composition and drops the limits.

### Tile frame

A `ShowcaseCard`: `rounded-xl border bg-card p-4`, the component centred, and a small label in the top-left styled like a console scribble strip (`font-mono text-[11px] uppercase tracking-wider text-muted-foreground`). The label is a link to the component's docs page and shows an arrow on hover and focus. The rest of the card is not a link, so every control inside stays usable.

### Above the grid

- Left: "Everything below is live. Drag a fader, turn a knob, hit a pad."
- Right: theme swatches (Stone, Ocean, Rose, Mono) that drive the existing theme store in `components/docs/theme-picker.tsx`. The navbar picker stays in sync through its change event. Retheming the whole grid at once is the strongest proof of "themed with your shadcn tokens".

### Below the grid

One centred outline button: "Browse all 20 components →".

---

## Behaviour

- **Loading:** each tile is a `dynamic(..., { ssr: false })` client component with a `Skeleton` of the tile's final height, so nothing shifts. Tiles mount when within about 200 px of the viewport (IntersectionObserver), so the demo audio (`OfflineAudioContext` renders) and signals below the fold cost nothing until scrolled to.
- **Animation:** everything paints on the shared frame loop, which already pauses off screen and on hidden tabs. Under `prefers-reduced-motion`, signals freeze on a representative frame; confirm how `use-reduced-motion` covers this today.
- **Sound:** nothing plays until the visitor clicks. Pad hotkeys stay **off** on the home page, so keys like Q or W are not taken from the page and the docs search shortcut.
- **Accessibility:** one h1. Tile labels are links with names like "Knob docs". Every meter and control keeps an `aria-label`. The divider waveform is `aria-hidden`.

---

## Files

| File | Change |
| --- | --- |
| `app/(home)/page.tsx` | Rewrite: hero, divider, grid toolbar, grid, closing button |
| `components/home/showcase-card.tsx` | New. The tile frame and label link |
| `components/home/showcase-grid.tsx` | New. The three stacks and tile order |
| `components/home/showcase-tile.tsx` | New. Client loader: dynamic import, sized skeleton, mount when near the viewport |
| `components/home/tiles/*.tsx` | New. One default-exported client tile per row in the table above |
| `components/home/hero-waveform.tsx` | New. The divider |
| `components/home/theme-swatches.tsx` | New. Swatch buttons over the shared theme store |
| `components/docs/theme-picker.tsx` | Export the theme list and store functions for the swatches |
| `components/docs/hero-demo.tsx`, `hero-demo-loader.tsx` | Delete. The mixer tile replaces them |
| `e2e/site.spec.ts` | Update the home test (see below) |

---

## Tests

- **Update** "the home page shows a live mixer": it expects the h1 to contain "mixer". Check the new h1 and keep the "Microphone level" meter assertion; the mixer tile keeps that label.
- **Add:** the home page has no horizontal overflow at 320, 768 and 1280 px, following the existing phone, tablet and desktop spec.
- **Add:** every tile renders and the page logs no console errors after scrolling to the bottom.
- **Add:** the home page to the existing frame rate and zero-commit performance gate, with every tile mounted.

---

## Decisions

1. The headline is "Audio UI, mixed and mastered."
2. Tiles carry console-style labels that link to their docs.
3. A "Use my mic" button on the Voice tile, which swaps the demo signal for the visitor's microphone on click, is left for a second pass: it adds a permission flow.

---

## Changes made while building

- **14 tiles, not 12.** With twelve, the wide column ran about 240 px past the narrow ones. A **Faders** tile (three console faders with meters in their tracks) joined the right column, and a **Live waveform** tile (an oscilloscope line over a scrolling level history) closed the left column. All three stacks now end within about 60 px of each other at 1440 px.
- **Mixer:** four channels (Mic, System, Music, Sounds) plus Master, as in the `system-audio-mixer` block; three left a gap before the master strip. The mixer is named by its card, so it takes `aria-label="Mixer"` instead of a `MixerTitle`.
- **Waveform:** plays a real demo track, with seeking, instead of static peaks.
- **Music player:** composed from `AudioPlayer` and `TrackList` in two panes (now playing beside the track list, stacked in narrow cards) rather than rendering the one-column block, which would have nested a card inside the tile's card. It uses a seek bar, so it does not repeat the waveform tile.
- **Voice:** the chips are Idle, Connecting and Speaking, matching the bar visualizer's idle, loading and live states. The live waveform moved to its own tile.
- **Channel controls** add a reverb send fader, and link to `channel-toggle`. **Output** shows the volume control inline rather than in a popover, and links to `audio-device-select`.
- **Compact player** hides its rate button below an 18rem container, where it would wrap the pill.
- **Placeholders** live beside each tile's lazy import as static `Skeleton` elements, because `shadcn/require-static-classes` cannot read a class passed through a config. Container queries size the music and pad placeholders for their stacked and side-by-side layouts.
- **Theme swatches** share the navbar picker's store through a `useSiteTheme` hook exported from `components/docs/theme-picker.tsx`, which also applies the saved theme on load. The home navbar drops the picker since the swatches cover it; docs pages keep it.
- **Divider removed.** The live waveform between the hero and the grid first spread from the centre in the mirror variant, which no audio tool does; after a pass as a left-scrolling waveform it was dropped, and WebThreads carries the hero into the grid.
- **WebThreads** (React Bits, added on request after the plan): woven threads behind the hero, full width, pinching at the install command above the grid. Dark mode uses light threads with glow. Light mode renders the same threads under `invert`, which over white is exactly the dark page's image flipped: dark threads as strong as the light ones, where drawing dark colours directly left a grey veil and the component's own `lightMode` drew neutral threads too faint to see. The component is adapted to the repo's lint rules in `components/home/web-threads.tsx`, holds still under reduced motion, pauses off screen, and adds `ogl`. It is site code under React Bits' MIT + Commons Clause licence, not a registry item.
- **Lint:** `components/home/**` joins the app code override in `oxlint.config.ts`, so tiles may pass colour and CSS variable classes to meters and visualizers.
- **Tests:** `e2e/showcase.ts` steps down the page until every tile has mounted. The responsive spec checks that no tile escapes its card at every viewport. The site spec checks that every tile loads without console errors, that soloing a mixer channel dims the others, that the swatches retheme the page and the navbar picker, and that the whole showcase runs above 50 fps. Clip lights commit React state when they latch, so the home frame test allows two commits where the console test allows none.
