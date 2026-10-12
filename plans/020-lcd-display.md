# 020 — LCD Display

Status: IMPLEMENTED (2026-10-12).

A dot-matrix screen from a hardware panel reference: rows of dim labels and glowing amber values (`MODEL  OPUS 5.5 ⌄`, `EFFORT  HIGH ⌄`), dotted dividers and a blinking status pixel. A value can be a dropdown. It stands alone: no audio core, no dependency on the knob. The selector knob from the same reference is plan 019.

## Anatomy

```tsx
<LcdDisplay>
  <LcdDisplayRow>
    <LcdDisplayLabel>Model</LcdDisplayLabel>
    <LcdDisplaySelect items={models} value={model} onValueChange={setModel}>
      <LcdDisplaySelectTrigger />
      <LcdDisplaySelectContent>
        <LcdDisplaySelectItem value="opus">Opus 5.5</LcdDisplaySelectItem>
      </LcdDisplaySelectContent>
    </LcdDisplaySelect>
  </LcdDisplayRow>
  <LcdDisplaySeparator />
  <LcdDisplayRow>
    <LcdDisplayLabel>Effort</LcdDisplayLabel>
    <LcdDisplayValue>High</LcdDisplayValue>
  </LcdDisplayRow>
  <LcdDisplayStatus />
</LcdDisplay>
```

| Part | Element | Notes |
| --- | --- | --- |
| `LcdDisplay` | `div` | The bezel, the glass and the dot grid. `role="group"`. `variant`, `size`, `matrix`, `scanlines` |
| `LcdDisplayRow` | `div` | Label left, value right. Gives the label an id through a row context |
| `LcdDisplayLabel` | `span` | Dim ink |
| `LcdDisplayValue` | `span` | Bright ink. `live` opts into `aria-live="polite"`. Flickers on change |
| `LcdDisplaySeparator` | `div` | A dotted rule, `role="separator"` |
| `LcdDisplayStatus` | `span` | The corner pixel. `state`: `idle` blinks slowly, `busy` quickly, `off` is unlit |
| `LcdDisplaySelect` + `Trigger`, `Content`, `Item` | Base UI `Select` | Trigger named by the row's label, with a pixel chevron. The popup is a small screen in the same ink. Highlighted items invert |

## Look

- **Follows the theme** (`appearance`, default `auto`). Light is a backlit LCD: pale glass tinted by the ink (`--lcd-display-glass`, the ink's hue at a fixed 0.93 lightness and 35% of its chroma, so a dark theme primary doesn't dim the glass) with dark pixels (`--lcd-display-lit`, the ink mixed 62% toward black) and a faint shadow instead of a glow. A grey-green reflective glass with only 8% of the ink was tried and dropped: too dull. Dark is near-black glass where the ink itself glows. `appearance="light"` or `"dark"` pins one look; `auto` writes the dark set out again behind `dark:` so Tailwind sees it. The popup carries the same appearance through context.
- The hardware colours and shadows are component variables (`--lcd-display-bezel`, `--lcd-display-bezel-edge`, `--lcd-display-bezel-ring`, `--lcd-display-bezel-shadow`, `--lcd-display-recess`, `--lcd-display-sheen`, `--lcd-display-scanline`), not the raw palette.
- **Ink follows the theme**: the default `variant="primary"` sets `--lcd-display-ink` to the theme's `--primary`, so the screen takes every theme's colour in light and dark. `amber`, `green`, `cyan` and `red` are fixed hardware inks; the social card keeps amber. `--lcd-display-lit` (lit pixels), `--lcd-display-ink-dim` (labels), `--lcd-display-ink-unlit` (background dots), the light glass and `--lcd-display-glow` are mixed from it, so a custom colour is one variable.
- **Dot-matrix without shipping a font**: the content is masked with a dot grid (`--lcd-display-pitch`, 2px), so any mono font breaks into dots. The glass tiles the same grid faintly, like unlit pixels. The glow is a `drop-shadow` on a wrapper outside the mask, so it bleeds past the dots.
- **A real dot-matrix font** is opt-in: set `--lcd-display-font` (default `var(--font-mono)`) and turn the mask off with `matrix={false}`. The docs site loads Google's Doto at weight 900, not preloaded, for its font example.
- Scanlines and a glass sheen over the top, off with `scanlines={false}`.
- `size` (`sm`, `default`, `lg`) sets the text size, since the repo's lint keeps typography classes off the root.
- No icon imports: the chevron is an inline 5 × 3 pixel SVG, and the selected item's mark is a lit pixel (Base UI's default check is replaced).
- The change flicker writes `data-changed` to the DOM for two frames, without a render. Blinking and flicker stop under `prefers-reduced-motion`.

## Delivery

1. `components/ui/lcd-display.tsx` and `components/ui/lcd-display.test.tsx`.
2. Examples `lcd-display-demo`, `lcd-display-variants`, `lcd-display-font`, and `content/docs/components/lcd-display.mdx`.
3. Docs nav (Controls, after Knob), the components index, the README table, a `registry.json` item and a social card.
4. Typecheck, lint, unit tests, registry build, and a visual check at 1× and 2× in both themes.
