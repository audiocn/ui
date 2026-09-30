# 001 — audiocn: a shadcn audio component library

Status: IMPLEMENTED (2026-09-30). See "Changes made while building" at the end.

## Summary

audiocn is a standalone, shadcn-based React library of audio components: meters, visualizers, volume faders, channel strips, a complete mixer, and sound and music playback. It is distributed as a shadcn registry and documented with Fumadocs.

It is built and works entirely on its own in the browser, on the Web Audio API. Videorc (`~/projects/videorc`) is the first planned consumer and adopts it later; nothing in this repo depends on Videorc.

## Detailed specs

This file is the overview. Every component, hook and block is specified in its own plan:

| Plan | Covers |
| --- | --- |
| `002-conventions.md` | The shadcn way: authoring rules, shared props, data attributes, theming, registry and docs conventions |
| `003-core-lib-and-hooks.md` | `lib/audio` and every hook |
| `004-meters-and-visualizers.md` | `level-meter`, `db-scale`, `db-readout`, `clip-indicator`, `bar-visualizer`, `live-waveform`, `waveform`, `spectrum` |
| `005-controls.md` | `fader`, `parameter-slider`, `knob`, `pan-control`, `channel-toggle`, `volume-control`, `audio-device-select` |
| `006-mixer.md` | `channel-strip`, `mixer` |
| `007-sounds-and-music.md` | `audio-player`, `track-list`, `sound-pad` |
| `008-blocks.md` | The six blocks, and later candidates |

Where this file and a spec disagree, the spec wins.

## Decisions

| Decision | Choice |
| --- | --- |
| Primitive library | Base UI (`base-rhea`, the repo's current shadcn setup) |
| Relationship to Videorc | Fully separate library; Videorc adopts it later |
| "Web components" | React components for the web, not Custom Elements |
| Audio backend | Web Audio API, shipped as optional hooks; components stay source-agnostic |
| Distribution | shadcn registry, namespace `@audiocn`, served from `https://audiocn.dev/r/{name}.json` |
| Docs | Fumadocs, in this same Next.js app, at `https://audiocn.dev` |
| Licence | MIT |
| Openness | Fully open source. The repo stays private until the library is built; nothing is announced or published before then |
| Repo shape | Single Next.js app (site, docs and registry), no monorepo |
| Component style | The shadcn way (plan 002): compound parts, `data-slot`, `className` everywhere, `cva` variants, semantic tokens, built directly on Base UI primitives |
| Units | dB and dBFS for mixer components. 0..1 only in `volume-control` and `audio-player`, matching `HTMLMediaElement.volume` |

## Goals

- Every component needed for a complete system audio mixer: microphone, system audio, music, sound effects and a master output.
- Components that run at 60 fps with many channels on screen, without a React render per audio frame.
- A working mixer in the browser with no native backend.
- Components that can later be fed from any other source (Videorc's Rust backend over WebSocket) without changes.

## Non-goals

- Recording, encoding or streaming audio.
- A DAW: no timeline, multitrack editing or automation.
- Custom Elements or non-React frameworks.
- Any change to Videorc as part of this plan.

## Architecture

### Layers

Each layer only depends on the layers above it in this list.

1. `lib/audio` — pure TypeScript, no React, no DOM. Decibel maths, meter ballistics, fader taper, band resampling, shared types.
2. `hooks` — React hooks. Two groups:
   - presentation hooks (`use-frame-source`, `use-level`, `use-clip-hold`, `use-mixer`, `use-demo-signal`)
   - Web Audio hooks (`use-audio-context`, `use-audio-analyser`, `use-audio-devices`, `use-microphone`, `use-system-audio`, `use-audio-player`, `use-sound`, `use-waveform-data`, `use-web-audio-mixer`)
3. `ui` — primitives and composites. They never call Web Audio, open a device or fetch anything. They receive values and frames.
4. `blocks` — full assemblies that wire `ui` to the Web Audio hooks.
5. `examples` — the demos rendered in the docs.

### Folder layout

```
registry/audiocn/
  lib/audio/       types, decibels, ballistics, zones, taper, bands, time, frame-loop
  hooks/           use-*.ts
  ui/              one file per component
  blocks/          one folder per block
  examples/        one file per demo
registry.json      source of truth, built to public/r/*.json
content/docs/      Fumadocs MDX
app/docs/          docs routes
app/(home)/        landing page
plans/             this file and the specs
```

### The data contract

All levels are dBFS. Components map dB to position over a configurable range (default −60..0 dBFS). `lib/audio` exports `levelToDb` and `dbToLevel` for consumers that hold a 0..1 level.

```ts
interface ChannelLevel {
  peakDb: number;
  rmsDb?: number;
}

interface MeterFrame {
  channels: ChannelLevel[]; // 1 = mono, 2 = stereo
}

interface VisualFrame {
  bands: Float32Array; // 0..1 per band
  history: Float32Array; // ring of 0..1 levels
  historyStart: number;
  historyLength: number;
  peakDb: number;
}
```

Every meter and visualizer accepts data three ways:

```tsx
// A source: the component subscribes and paints itself. No React renders.
<LevelMeter source={analyser.meter} />;

// Values: for slow or occasional data. The component applies ballistics on its
// own animation frame, so a 1 Hz source still moves smoothly.
<LevelMeter peakDb={-9.1} rmsDb={-18} />;

// A ref handle: for callers that already own a frame loop.
meterRef.current?.paint(frame);
```

A source is anything with `subscribe(callback): unsubscribe`. `use-audio-analyser` returns one, and so can a WebSocket client or a test.

### Mixer state

- All controls are controlled components (`value` + `onValueChange`, plus `onValueCommitted` for persistence), matching shadcn conventions.
- `use-mixer` is an optional reducer hook for the state of a mixer: channels with `id`, `gainDb`, `muted`, `solo`, `pan`, plus master gain.
- `use-web-audio-mixer` binds that state to a Web Audio graph: a `GainNode` and `StereoPannerNode` per channel, solo and mute logic, a master gain, and an `AnalyserNode` tap per channel and on the master for meters.
- A consumer with its own engine keeps `use-mixer` (or its own state) and skips `use-web-audio-mixer`.

### Rendering

- Meters: DOM elements moved with `transform`, so updates stay on the compositor. Level changes are not CSS-transitioned; ballistics are computed.
- Waveform and spectrum: Canvas 2D, scaled for device pixel ratio through a `ResizeObserver`.
- One shared animation-frame loop per page, not one per component.
- Loops stop when the component is off screen or the document is hidden.

### Theming

- Semantic tokens only; no raw colour values in components.
- Visualizers paint with `currentColor`, so the parent's text colour sets the tone.
- Meter zones use three CSS variables shipped through the registry's `cssVars`: `--meter-ok`, `--meter-warn`, `--meter-clip`, with light and dark defaults.
- Primitives import no icons; icons are passed as children. Blocks and examples use Phosphor, the repo's icon library.
- No `backdrop-filter` and no toasts from passive components.

### Accessibility

- Meters: `role="meter"` with `aria-valuenow` in dB, updated at most 4 times a second, and `aria-valuetext` such as "−12 dB".
- Faders: slider semantics, `aria-valuetext` in dB, arrow keys for one step, Shift+arrow for a large step, Home/End, and a reset to the default value.
- Mute, solo and monitor: toggle buttons with `aria-pressed`.
- `prefers-reduced-motion`: visualizers show a static level instead of animating.
- Colour is never the only signal: clip and mute states also carry text or shape.

### Distribution

- `registry.json` at the repo root; `shadcn build` writes `public/r/*.json`.
- Item types: `registry:lib`, `registry:hook`, `registry:ui`, `registry:block`.
- Components are built directly on Base UI primitives (`@base-ui/react`). Official shadcn items are only dependencies where a component composes one (for example `select`, `badge`, `kbd`); those are listed by bare name in `registryDependencies`. audiocn items reference each other through the `@audiocn` namespace.
- Consumers add `"@audiocn": "https://audiocn.dev/r/{name}.json"` to `registries` in `components.json` and run `shadcn add @audiocn/mixer`.
- Until launch the registry is only used locally; the site is not deployed publicly and the repo is not made public.

## Components

One line each; the full specs are in plans 003–008.

### Meters and visualizers (plan 004)

| Item | What it is |
| --- | --- |
| `level-meter` | Peak and RMS meter; mono or stereo; solid, segmented or gradient; zones; peak hold; optional scale, readout and clip light |
| `db-scale` | dB tick marks and labels, shared by meters and faders |
| `db-readout` | Throttled numeric label such as "−12.3 dB" |
| `clip-indicator` | Clip light with hold time, count and click to reset |
| `bar-visualizer` | Frequency-band bars, any bar count |
| `live-waveform` | Scrolling or static canvas waveform of a live signal |
| `waveform` | Static waveform of a clip, with playhead, seeking, regions and markers |
| `spectrum` | Spectrum analyser with frequency and level axes (phase 6) |

### Controls (plan 005)

| Item | What it is |
| --- | --- |
| `fader` | dB volume fader: taper, detents, scale, reset, horizontal or vertical |
| `parameter-slider` | Labelled slider with numeric input, unit and reset |
| `knob` | Rotary control |
| `pan-control` | Left and right balance with a centre detent |
| `channel-toggle` | Mute, solo and monitor toggles |
| `volume-control` | Simple 0..1 volume slider with a mute button, for players |
| `audio-device-select` | Device picker with permission, loading and missing-device states |

### Mixer (plan 006)

| Item | What it is |
| --- | --- |
| `channel-strip` | Compound strip: header, status, meter, fader, controls, notice. Row and console layouts. `variant="master"` is the output strip |
| `mixer` | Strip container: shared meter configuration, keyboard navigation, master area, empty state |

### Sounds and music (plan 007)

| Item | What it is |
| --- | --- |
| `audio-player` | Composable player: transport, seek, time, volume, rate, loop |
| `track-list` | Track list with active and playing states |
| `sound-pad` | Trigger pad with modes, hotkey and progress, plus a pad grid |

### Blocks (plan 008)

| Block | What it assembles |
| --- | --- |
| `system-audio-mixer` | The complete mixer: microphone, system audio, music, sounds and master, running on Web Audio |
| `mic-setup` | Device select, live preview, meter, gain, mute and a level check |
| `system-audio-settings` | On/off switch, level, meter and status notices |
| `quick-audio-popover` | Compact mic and system audio controls in a popover |
| `soundboard` | Grid of sound pads with master volume, hotkeys and stop-all |
| `music-player` | Player, track list, shuffle, repeat and ducking |

### Hooks and lib (plan 003)

| Item | Purpose |
| --- | --- |
| `core` (`lib/audio`) | dB conversion, ballistics, zones, taper, band resampling, time formatting, shared frame loop, types, and the audio theme tokens |
| `use-frame-source` | Subscribe a callback to a frame source |
| `use-level` | Low-rate React state from a frame source |
| `use-clip-hold` | Clip detection with a hold time |
| `use-mixer` | Mixer state: gain, mute, solo, pan |
| `use-demo-signal` | Deterministic synthetic signal for previews and tests |
| `use-audio-context` | One shared `AudioContext` |
| `use-audio-analyser` | Frames from a `MediaStream`, media element or `AudioNode` |
| `use-audio-devices` | Device list, permission state, device-change events |
| `use-microphone` | Microphone stream with processing off by default |
| `use-system-audio` | System or tab audio through `getDisplayMedia` |
| `use-audio-player` | Media-element playback state |
| `use-sound` | Low-latency buffer playback for pads |
| `use-waveform-data` | Decode a file into waveform peaks |
| `use-web-audio-mixer` | Binds mixer state to a Web Audio graph, with ducking and a limiter |

## Documentation

Fumadocs (`fumadocs-ui` and `fumadocs-core` 16.x, `fumadocs-mdx` 15.x), following the layout already used in `~/projects/8bitcn`.

- Sections: Getting started, Concepts, Components, Hooks, Blocks.
- Concepts pages: decibels and levels, feeding data (source, values, ref), theming, accessibility, using your own audio engine.
- Every component page follows the template in plan 002.
- A theme picker switches shadcn presets live, so every component can be seen under different themes.
- Every component page has a live preview, a code tab, the install command, a props table generated from the types, and accessibility notes.
- Previews run on `use-demo-signal`, a deterministic synthetic signal, so they animate on load with no microphone prompt. Each preview has an opt-in "use my microphone" switch.
- `llms.txt` and `llms-full.txt` routes.

## Testing and quality gates

- `lib/audio`: vitest unit tests for dB conversion, ballistics timing, taper round-trips and band resampling.
- Components: Testing Library tests for keyboard control, ARIA values and controlled-state behaviour.
- Web Audio hooks: tested against a mocked `AudioContext`.
- Registry: `shadcn build` must succeed, and an install smoke test adds every item into a clean Base UI fixture project and type-checks it.
- Every change passes `pnpm exec ultracite check`, `pnpm typecheck` and `pnpm build`.
- Performance check: the full mixer block with 16 strips holds 60 fps with no React commits during steady-state metering.

## Phases

Each phase ends with docs pages, tests and registry entries for what it adds.

### Phase 0 — Foundations

- Read `node_modules/next/dist/docs/` for the Next.js 16 conventions the repo requires before writing app code.
- Add Fumadocs: `source.config.ts`, `lib/source.ts`, `app/docs/[[...slug]]`, `mdx-components.tsx`, the docs layout.
- Add `registry.json`, the `registry/audiocn` tree and a `registry:build` script.
- Build the docs preview component (preview, code, install command) and the props table.
- Add vitest and Testing Library.
- Add the audio tokens (plan 002) to `app/globals.css`.
- Landing page and site navigation.
- Add the MIT `license.md` and set `"license": "MIT"` in `package.json`.

Done when: the docs site builds with one placeholder component page, and that component installs from the local registry into a fixture project.

### Phase 1 — Audio core

- `lib/audio` with full tests.
- `use-frame-source`, `use-level`, `use-clip-hold`, `use-demo-signal`.
- `use-audio-context`, `use-audio-analyser`, `use-audio-devices`, `use-microphone`.

Done when: a docs page shows raw frames from both the synthetic signal and a real microphone.

### Phase 2 — Meters and visualizers

- `db-scale`, `db-readout`, `clip-indicator`, `level-meter`, `bar-visualizer`, `live-waveform`.

Done when: all six render from all three inputs (source, values, ref), and the reduced-motion behaviour is verified.

### Phase 3 — Controls

- `fader`, `parameter-slider`, `channel-toggle`, `volume-control`, `audio-device-select`, `pan-control`, `knob`.
- Add the shadcn items they compose (`select`).

Done when: every control is fully keyboard operable and announces dB values.

### Phase 4 — Mixer

- `use-mixer`, `use-web-audio-mixer`, `use-system-audio`.
- `channel-strip`, `mixer`.
- Blocks: `system-audio-mixer`, `mic-setup`, `system-audio-settings`, `quick-audio-popover`.

Done when: the `system-audio-mixer` block mixes a live microphone, captured system audio and a test tone in the browser, with working mute, solo, pan and master, and passes the 16-strip performance check.

### Phase 5 — Sounds and music

- `use-audio-player`, `use-sound`, `use-waveform-data`.
- `waveform`, `audio-player`, `track-list`, `sound-pad`.
- Blocks: `soundboard`, `music-player`; both added as channels to `system-audio-mixer`.

Done when: music and pads play through the mixer and show on its meters, and music ducks under the microphone.

### Phase 6 — Analysis and processing (later)

- `spectrum`.
- Candidates, listed at the end of plan 008 and to be specified separately: loudness meter, gain-reduction meter, correlation meter, oscilloscope, spectrogram, and noise gate, compressor and EQ panels.

## Videorc adoption (later, not part of this plan)

Recorded here so the library is designed with it in mind.

- Videorc's shadcn setup is `radix-rhea`; audiocn is Base UI. Most audiocn components are built directly on `@base-ui/react`, which Videorc can install alongside Radix, so they need no change. Only the items that compose a shadcn styled component (`audio-device-select`, and the blocks) hit the Radix and Base API differences and would need a Radix variant or local edits.
- Videorc's local checkout was 173 commits behind `origin/main` on 2026-09-30. Adoption work must start from `origin/main`, which has system audio and the v2 design rules.
- Components it would replace: `AudioMixer`, `SystemAudioMixerRow`, `BarVisualizer`, `LiveWaveform`, `PowerSlider` (audio uses), `SourceSelect`, `MicPickerPreview`, and the maths in `lib/mic-meter.ts`.
- Videorc's backend sends levels at 1 Hz, mono, peak only. The library smooths any rate, but a real mixer there needs the backend to emit per-source, per-channel peak and RMS at about 30 Hz.
- Videorc's backend has no music or sound sources, solo, pan, monitoring or master control yet.
- Videorc bans direct icon-package imports, `bg-popover`, `cursor-pointer` and raw colours. The theming rules above already satisfy these for primitives; blocks would need their icons swapped.

## Known gaps to settle in Phase 0

The specs are designs on paper. These points are unverified and are settled by a short spike at the start of Phase 0, before any component is built:

- **Import paths.** Source lives under `registry/audiocn/`, and items import each other. It is unverified how the docs app should import them and whether the shadcn CLI rewrites those imports correctly on install. Spike: one `ui` item that imports a `lib`, a `hook` and another `ui` item, built and installed into a fixture project. Fallback: the `8bitcn` layout (files under `components/ui/` with explicit targets).
- **Token delivery.** It is unverified that a registry item's `cssVars` can both add the audio tokens and map them into Tailwind (`bg-meter-ok`). Same spike.
- **Local registry URL.** `audiocn.dev` is not deployed before launch, so local work and the install smoke test point `@audiocn` at `http://localhost:3000/r/{name}.json`.
- **Demo audio.** The docs and blocks need short music and sound clips under a licence that allows redistribution in an MIT repo (CC0, or generated in code). None are chosen yet.
- **Browser support for system audio capture** is stated from memory and must be checked against current browser documentation before the docs state it.
- **Default numbers** (meter zones, ballistics, clip threshold, fader range) are starting values and may change once they are seen and heard.
- **Prove the conventions first.** `level-meter` and `fader` are built and reviewed before the rest, and plan 002 is corrected from what they teach.

Not planned yet, and not blocking Phase 0: landing page design, CI, versioning and changelog, and a contributing guide.

## Open questions

- Whether to ship Radix variants of `audio-device-select` and the blocks before Videorc adoption.

## Changes made while building

The specs in plans 002–008 were written before any code. These are the places where the build departs from them, and why.

- **Layout.** Source files live at the paths a consumer gets (`components/ui`, `hooks`, `lib/audio`, `components/blocks`) instead of `registry/audiocn/`. Imports are then identical in the docs app and after install, and the CLI's standard alias rewriting is all that is needed.
- **Props tables** are written in MDX with a `PropsTable` component. `fumadocs-typescript` needs the TypeScript 7 native API, which this repo does not use.
- **Imperative handles** use an `actionsRef` prop, following Base UI's naming. `ref` stays a plain DOM ref, so components still work as Base UI `render` targets.
- **Tapers** expose `toPosition` and `toValue` (not `toDb`), because the same interface serves log scales for frequency and time.
- **Master strip** is `ChannelStrip variant="master"`, as planned in plan 006.
- **Extra parts** that the specs lacked but the layouts needed: `LevelMeterChannels`, `KnobDial`, `ChannelStripText`, `useAudioConfig` (the context strips and mixers pass down), `useReducedMotion`, `createFrameRelay` and `createAnalyserTap`.
- **Level meter semantics** set `role="meter"` and its ARIA values directly on the DOM, instead of using Base UI `Meter`, so updates skip React renders.
- **Mixer channels** gained a `monitor` field: a per-channel send to the speakers, off by default for the microphone to avoid feedback.
- **Demo audio** is synthesised in the browser with `OfflineAudioContext`, so no audio files ship. Blocks default to empty track and sound lists; the docs pass the generated audio in.
- **Theme picker** offers four colour themes defined in `app/globals.css` rather than switching shadcn presets.
- **Performance gate** runs in e2e: a 16-strip console example on the Mixer page is measured for two seconds and must hold over 50 fps with zero React commits (counted through a stand-in React DevTools hook).
- **Install smoke test** (`pnpm test:install`) creates a Next.js app with the shadcn RC CLI (`--base base --preset nova`), installs all 43 items from a local registry server and type-checks the app.

## Verification (2026-09-30)

- `pnpm exec ultracite check`: clean.
- `pnpm typecheck`: clean.
- `pnpm test`: 99 unit tests pass.
- `pnpm build`: registry and site build with no warnings (57 static pages).
- `pnpm test:e2e`: 59 tests pass, including every docs page with no console errors and the performance gate.
- `pnpm test:install`: 43 items (67 files) install into a fresh Base UI app and type-check.
