# 019 — Knob: the knurled cap

Status: IMPLEMENTED (2026-10-12). Stacked on PR #22 (plan 018, rotary selector). The LCD display from the same reference is plan 020.

The stepped "effort" knob from a hardware panel reference: a dark cap with a ridged grip and a white painted line, surrounded by four labelled positions (LOW · MED · HIGH · ULTRA). The positions, snapping, label clicks and typing come from `RotarySelector` (plan 018). This plan adds only the cap and an example that composes the print.

The first draft of this plan added a `KnobDetents` part and a stepped `fineStep` default to `Knob`. Both were dropped: the rotary selector already selects from a fixed list, with typed values and clickable labels.

## Approach: a third `KnobCap` variant

```tsx
<RotarySelectorDial>
  {/* print: marks, dots, labels */}
  <KnobCap variant="knurled" />
</RotarySelectorDial>
```

It works in any dial, through the dial context from plan 017, so `Knob` takes it too (`<KnobScale />` + `<KnobCap variant="knurled" />`).

- **Layers**, from the outside in: halo, dark bezel (radius 33, reported as the cap radius, so the selector's drag circle matches), a knurled grip of 60 ridges (a lit wall and a shaded wall each) from 27 to 31.4, a groove, and a smooth dished face lit from the upper left.
- **Lighting stays still, the grip and line turn.** The grip and the pointer turn by `rotate: var(--knob-cap-turn)`, so the turn is a CSS transition: 150 ms with a slight overshoot, off while `data-dragging` is set on the dial, off under `prefers-reduced-motion`.
- **The short way round.** The cap carries the angle across whole turns (`useContinuousAngle`), so a selector that wraps from its last position to its first animates forward, not back across the dial. On a `Knob`, a Home or End jump of more than 180° turns the short way through the gap for the 150 ms of the snap and lands in the right place.
- **Tokens:** the variant sets `--knob-cap-metal` to `zinc-700` and adds `--knob-cap-pointer` (white). It is dark in both themes; the print follows the theme. The `default` and `mini` variants now set their own white metal, so the variants never fight over `--knob-cap-metal`.
- **Slots:** `knob-cap` (`data-variant="knurled"`), `knob-cap-turn`, `knob-cap-knurl`, `knob-cap-face`, `knob-cap-pointer`.

## Effort selector example

`components/examples/rotary-selector-effort.tsx`, in the rotary selector docs:

- Four values over 240° (`startAngle={-120}`, `stepAngle={80}`), with three short ticks between positions as plain `<line>`s.
- Each position: a long `RotarySelectorPositionMark`, and a hollow dot that fills when selected plus an upright mono label, both inside one `RotarySelectorPositionLabel` with a click box. Labels hang off the side of their dot that faces away from the dial.
- An "Auto effort" lamp under the knob, on a short lead that lights with it. On, it disables the selector.

Taken from the reference and left out: the ▲ beside MED (the selected position already lights its mark, dot and label), the brushed plate with screws, and the LCD beside the knob (plan 020, a separate PR). A full panel combining both can follow once both land.

`components/examples/knob-knurled.tsx` shows the cap on a plain `Knob` with a 0–10 scale, in the knob docs.

## Delivery

1. `components/ui/knob.tsx`: the `knurled` variant.
2. Unit tests in `components/ui/controls.test.tsx`: the grip and line turn with the value, the cap reports radius 33, a wrap turns forward.
3. Examples, knob docs (Knurled cap section, theming row, `KnobCap` API), rotary selector docs (Effort selector section), registry description.
4. Typecheck, lint, unit tests, registry build, and a headless visual check in both themes.
