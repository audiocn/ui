# 014 — Knob: metal mini knobs

Status: IMPLEMENTED (2026-10-03).

The volume dial's brushed aluminium cap (plan 010), at everyday knob sizes: the standard track and range around a metal cap, for rows of small controls such as gain, pan and filter knobs.

## Approach: a `variant` on `KnobCap`

The default cap is sized to sit inside `KnobScale`, and its dot is too small to read on a 48px knob. `KnobCap` takes `variant="mini"`, which swaps `KnobPointer` in the standard anatomy:

```tsx
<KnobDial>
  <KnobTrack />
  <KnobRange />
  <KnobCap variant="mini" />
</KnobDial>
```

- The cap fills the inside of `KnobTrack`, with the same halo, bezel, rim, conic face and grain as the default cap.
- The value is marked with an engraved line instead of a dot: a dark groove with a light lip below it, lit from above, so the lip stays put while the line turns.
- The brushed rings are coarser (`--knob-cap-pitch`, 0.9px instead of 0.3px) so they stay a texture, not moiré, at small sizes.
- The variant classes are a `cva`, exported as `knobCapVariants`; the cap carries `data-variant`. New slot: `knob-cap-pointer`.

## Delivery

1. `components/ui/knob.tsx`: the `mini` variant.
2. A unit test in `components/ui/controls.test.tsx`: the line turns with the value and there is no dot.
3. Example `components/examples/knob-metal.tsx` and a "Metal knobs" section in the knob docs, with the `KnobCap` API row and `--knob-cap-pitch`.
4. Registry description mentions the mini cap.
5. Typecheck, lint, unit tests, and a visual check at `sm`, `default` and 3× in both themes.
