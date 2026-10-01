# 010 — Knob: the volume dial variant

Status: IMPLEMENTED (2026-10-01).

A hi-fi volume dial built from the existing `Knob`: a brushed aluminium cap with an indicator dot, in a dark bezel, surrounded by a numbered tick scale (0 to 100, a tick per unit, a long tick every 5, a number every 10, numbers turned to follow the arc).

## Approach: new parts, not a `variant` prop

The knob's look comes from the parts inside `KnobDial`, not from classes on the root, so a `variant` prop on `Knob` would have nothing to switch. The volume dial is two new parts that compose with the rest, the same way `KnobTrack`, `KnobRange` and `KnobPointer` do:

```tsx
<Knob className="[--knob-size:14rem]" defaultValue={33}>
  <KnobDial>
    <KnobScale labelEvery={10} majorEvery={5} ticks={100} />
    <KnobCap />
  </KnobDial>
  <KnobLabel className="sr-only">Volume</KnobLabel>
</Knob>
```

Everything else (circular drag, Shift for fine, keyboard, wheel, reset, typed values, log scale, bipolar origin) is unchanged and works with the new parts.

## `KnobScale`

Tick marks and numbers around the outside of the 100 × 100 view box.

| Prop | Default | Meaning |
| --- | --- | --- |
| `ticks` | 50 | Divisions across the arc; draws `ticks + 1` marks |
| `majorEvery` | 5 | Every nth tick is long |
| `labelEvery` | 10 | Every nth tick is numbered; 0 hides the numbers |
| `format` | the knob's `format` | Text for each number |

- Ticks are evenly spaced along the arc, so they follow `scale="log"` too.
- Ticks between `origin` and the value carry `data-active` and light up in `foreground`; the rest are `muted-foreground`. Bipolar knobs light from the centre.
- Numbers are rotated to follow the arc, as on the reference dial.
- Slots: `knob-scale`, `knob-tick` (`data-major`, `data-active`), `knob-scale-label`.

## `KnobCap`

The aluminium cap, drawn inside the scale.

- A soft shadow well, a dark bezel and a bright rim, as SVG gradients.
- The face is a CSS conic gradient with fine concentric brushing, in a `foreignObject`, since SVG has no conic gradient. The highlights stay still while the knob turns, like light on a real cap.
- An indicator dot near the edge turns with the value.
- The cap is aluminium in both themes; the scale follows the theme tokens.
- Slots: `knob-cap`, `knob-cap-face`, `knob-cap-dot`.

## Sizing

The scale's numbers need room, so the volume dial is used at a large custom size (`[--knob-size:14rem]`) rather than adding a size to the shared `AudioSize` scale.

## Delivery

1. `components/ui/knob.tsx`: `KnobScale` and `KnobCap`.
2. Unit tests in `components/ui/controls.test.tsx`: tick and label counts, lit ticks from the origin, the dot following the value.
3. Example `components/examples/knob-volume.tsx` and a "Volume dial" section in the knob docs, with anatomy, theming and API rows.
4. Registry description mentions the scale and cap.
5. Typecheck, lint, unit tests, a visual check in the browser, then push to `main`.
