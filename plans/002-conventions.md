# 002 — Conventions: building audiocn the shadcn way

Status: PLANNED (2026-09-30). Applies to every component in plans 004–008.

The test for every component: a developer who knows shadcn/ui should be able to guess its API, restyle it with Tailwind classes, and re-theme it with CSS variables, without reading the source.

## Authoring rules

1. **One file per component** in `registry/audiocn/ui/<name>.tsx`. The file is the unit a user copies, reads and edits.
2. **Flat named exports.** Compound parts are separate named components (`Fader`, `FaderTrack`, `FaderThumb`), never a dot namespace.
3. **Built on Base UI primitives directly**, the way shadcn's own `slider.tsx` wraps `@base-ui/react/slider`. audiocn components do not wrap shadcn's styled components unless the user would expect that exact component (`select`, `popover`, `tooltip`, `button`, `badge`).
4. **`data-slot` on every part**, named `<component>-<part>` (`data-slot="fader-thumb"`).
5. **`className` on every part**, merged last with `cn` from `@/lib/utils`, so the user's classes win.
6. **Rest props are spread** onto the underlying element. The props type is the primitive's props or `React.ComponentProps<"div">`, intersected with the component's own props. Nothing is hidden.
7. **Variants use `cva`**, and the variants object is exported (`faderVariants`). The shared axes are `variant` and `size` (`"sm" | "default" | "lg"`).
8. **State is exposed as data attributes**, so users style states with Tailwind (`data-[clipping]:ring-2`). Boolean attributes are present or absent.
9. **Controlled and uncontrolled.** Every stateful control takes `value`, `defaultValue`, `onValueChange` and `onValueCommitted`, matching Base UI's names.
10. **`render` prop for polymorphism**, through Base UI's `useRender`, on parts where swapping the element makes sense (buttons, triggers, items).
11. **React 19:** `ref` is a normal prop, no `forwardRef`.
12. **`"use client"`** at the top of every file that uses state, effects or browser APIs.
13. **No icon imports in `ui` components.** Icons are children. Blocks and examples use Phosphor.
14. **No layout opinions baked in.** Width, height and spacing come from `className`. Canvas components fill their container; there is no `height` prop.
15. **Radii and heights follow the consumer's theme**: `rounded-sm/md/lg` (derived from `--radius`) and the shadcn control heights, never fixed pixel radii.
16. Code passes `pnpm exec ultracite check`, which in this repo means arrow function components.

## Shared prop vocabulary

The same concept always has the same name.

| Prop | Type | Meaning |
| --- | --- | --- |
| `value` / `defaultValue` | `number` | Current value, in the control's unit |
| `onValueChange` | `(value, details) => void` | Fires while changing |
| `onValueCommitted` | `(value, details) => void` | Fires on release or blur |
| `min` / `max` | `number` | Range of a control |
| `step` / `largeStep` / `fineStep` | `number` | Arrow key, Shift+arrow, Alt+drag |
| `resetValue` | `number` | Value restored by double-click |
| `orientation` | `"horizontal" \| "vertical"` | Layout axis |
| `size` | `"sm" \| "default" \| "lg"` | Density |
| `disabled` | `boolean` | Disables interaction |
| `minDb` / `maxDb` | `number` | Displayed range of a meter or scale (default −60 / 0) |
| `peakDb` / `rmsDb` | `number` | Declarative level input, dBFS |
| `channels` | `ChannelLevel[]` | Declarative multi-channel level input |
| `source` | `FrameSource<T>` | A subscribable stream of frames |
| `format` | `(value: number) => string` | Custom value text |

Units:

- Mixer components use dB (gain) and dBFS (levels). Silence is `-Infinity` and renders as "−∞".
- `volume-control` and `audio-player` use 0..1, because that is `HTMLMediaElement.volume`. They are the only exception.
- Time is seconds in props, formatted as `m:ss`.

## Three ways to feed a meter or visualizer

Every meter and visualizer supports all three, in this order of preference.

```tsx
// 1. A source: the component subscribes and paints itself. No React renders.
<LevelMeter source={analyser.meter} />;

// 2. Values: for slow or occasional data. The component applies ballistics.
<LevelMeter peakDb={-9.1} rmsDb={-18} />;

// 3. A ref handle: for callers that already own a frame loop.
meterRef.current?.paint(frame);
```

`FrameSource<T>` is `{ subscribe(callback: (frame: T) => void): () => void }`. Anything can implement it: the Web Audio hooks in this library, a WebSocket, a test.

## Shared data attributes

| Attribute | On | Meaning |
| --- | --- | --- |
| `data-slot` | every part | Part name |
| `data-orientation` | roots | `horizontal` or `vertical` |
| `data-size`, `data-variant` | roots | Current variant values |
| `data-disabled` | controls | Disabled |
| `data-dragging` | sliders, knobs | Pointer is held |
| `data-zone` | meters, readouts | `ok`, `warn` or `clip` |
| `data-clipping` | meters, clip indicator, strips | Clip hold is active |
| `data-active` | meters, visualizers | Signal is above the floor |
| `data-muted`, `data-solo`, `data-dimmed` | strips | Channel state |
| `data-playing`, `data-loading` | players, pads | Playback state |

## Theming

### Semantic tokens only

Components use the standard shadcn tokens (`background`, `foreground`, `muted`, `primary`, `border`, `ring`, `destructive`, and so on), so any shadcn theme or preset restyles audiocn with no extra work.

### Audio tokens

A small set of new tokens for meanings shadcn has no token for. They ship in the `@audiocn/core` registry item through `cssVars`, with light and dark values, and are mapped into Tailwind (`bg-meter-ok`, `text-channel-solo`).

| Token               | Default              | Used for                 |
| ------------------- | -------------------- | ------------------------ |
| `--meter-ok`        | green                | Normal level zone        |
| `--meter-warn`      | amber                | Warning zone             |
| `--meter-clip`      | `var(--destructive)` | Clip zone and clip light |
| `--channel-mute`    | `var(--destructive)` | Pressed mute toggle      |
| `--channel-solo`    | amber                | Pressed solo toggle      |
| `--channel-monitor` | blue                 | Pressed monitor toggle   |

Users override them in their own CSS exactly like any shadcn token.

### Component-level CSS variables

Geometry and canvas colours are CSS variables on the component root, so they can be set from a class (`[--meter-thickness:6px]`) or globally.

- Canvas components read their colours from CSS variables at paint time, falling back to `currentColor`. A theme switch repaints without a remount.
- Meters write the live level to `--meter-level` (0..1) on each channel element, so a user can build a custom visual from it in pure CSS.

### Themes in the docs

- A theme picker on the docs site switches shadcn presets live, to show every component under different themes.
- A "Theming" page documents the audio tokens and every component variable.
- Optional `registry:theme` items can ship alternative audio token sets (for example a monochrome meter) later.

## Accessibility baseline

- Meters: Base UI `Meter` semantics, value in dB, updated at most four times a second.
- Sliders, faders, knobs: slider semantics, `aria-valuetext` in the control's unit, full keyboard control.
- Toggles: `aria-pressed` and a default `aria-label`.
- Every animated component honours `prefers-reduced-motion`.
- Focus rings use the theme's `ring` token, matching shadcn controls.

## Registry conventions

- Item names are kebab-case and match the file name.
- Types: `registry:lib`, `registry:hook`, `registry:ui`, `registry:block`.
- Dependencies on official shadcn items use bare names (`"select"`); dependencies on audiocn items use `@audiocn/<name>`.
- `@audiocn/core` holds `lib/audio` and the audio tokens. Every `ui` item depends on it.
- Each item lists its npm `dependencies` explicitly (`@base-ui/react`, `class-variance-authority`).
- Source files import only through the standard aliases (`@/lib/utils`, `@/components/ui/...`, `@/hooks/...`), so the CLI can rewrite them.

## Docs page template

Every component page has the same sections, in this order:

1. One-sentence description and a live preview.
2. Installation (CLI command, then manual steps).
3. Usage: the import and the smallest working snippet.
4. Anatomy: the full compound structure.
5. Examples: one per variant, size, orientation and notable prop.
6. Feeding data (meters and visualizers only).
7. Theming: tokens, CSS variables and data attributes.
8. Accessibility: roles and the keyboard table.
9. API reference: a generated props table per part.
