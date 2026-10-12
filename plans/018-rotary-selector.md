# 018 — Rotary selector

Status: IMPLEMENTED (2026-10-08), PR #22. Follows plans 002 and 005. Needs plan 016 (click sound helper) and plan 017 (shared dial context and wheel).

A rotary switch that selects one of a fixed list of positions, like the waveform switch on a synth VCO or the TR-808 instrument select. It behaves like a `<select>`: one value at a time, never empty.

The panel print (marks, labels, leaders) is not laid out by the component. The component gives typed geometry and parts, and the user composes the print. An automatic label layout was tried in a prototype and dropped: it had too many edge cases and blocked custom designs.

The design was settled in a brainstorm and several prototype rounds. The prototype answered the keyboard, drag, wheel, leader and print-placement questions. It is kept on the branch `prototype/rotary-selector` (to be pushed when the prototype is captured), not on `main`.

---

## Anatomy

```tsx
const WAVES = ["sine", "triangle", "saw", "square"] as const;

<RotarySelector
  values={WAVES}
  value={wave}
  onValueChange={setWave}
  startAngle={-45}
  stepAngle={-30}
  format={(value) => value.toUpperCase()}
>
  {({ positions }) => (
    <>
      <RotarySelectorDial>
        {positions.map((position) => (
          <Fragment key={position.value}>
            <RotarySelectorPositionMark position={position} />
            <RotarySelectorPositionLeader
              position={position}
              from={53}
              ray={8}
              to={{ x: -18 }}
            >
              <RotarySelectorPositionLabel position={position}>
                <WaveIcon wave={position.value} />
              </RotarySelectorPositionLabel>
            </RotarySelectorPositionLeader>
          </Fragment>
        ))}
        <KnobCap />
      </RotarySelectorDial>
      <RotarySelectorValue />
      <RotarySelectorLabel>Waveform</RotarySelectorLabel>
    </>
  )}
</RotarySelector>;
```

- **Once per control:** `RotarySelector`, `RotarySelectorDial`, `RotarySelectorLabel` (the control's name, like `KnobLabel`), `RotarySelectorValue` (read-only readout of `format(value)`).
- **Once per position**, each taking the `position` object: `RotarySelectorPositionLabel`, `RotarySelectorPositionMark`, `RotarySelectorPositionLeader`.
- **Shared with Knob:** `KnobCap` (both variants) and `KnobPointer`, through the dial context from plan 017. The cap is the physical part. It does not depend on what turns under it.

## Typing

The children of the root are a function. The root infers the value union from `values` with a const generic, so the render function gets typed positions with no factory and no explicit type argument.

```ts
children: (selector: {
  value: Value;
  positions: readonly RotarySelectorPosition<Value>[];
  position: (value: Value) => RotarySelectorPosition<Value>;
}) => ReactNode;

interface RotarySelectorPosition<Value> {
  value: Value;
  index: number;
  angle: number; // degrees, 0 = 12 o'clock, clockwise positive
  selected: boolean;
  pointAt: (radius: number) => { x: number; y: number };
}
```

- Parts take `position`, not `value`, so the union flows into every part without generics.
- `position("saw")` is a typed getter for one-off print. `position("sawtooth")` is a type error.
- `value`, `defaultValue`, `resetValue`, `onValueChange`, `onValueCommitted` and `format` are all typed against the union.

## Geometry

- `startAngle` is the angle of the first value. `stepAngle` is the angle between values (30° is the hardware default). A negative step runs anticlockwise. Nothing is centred automatically: to centre the positions, offset `startAngle` by half the spread.
- The number of positions is `values.length`.
- **Wrap:** the selector wraps from the last value to the first only when `values.length × |stepAngle| = 360°` (with a rounding tolerance, so `360 / 7` works). Otherwise it stops at both ends. There is no prop.
- **Overflow:** a total above 360° is invalid.
  - With a literal `values` tuple and a literal `stepAngle`, it is a type error on `stepAngle` ("13 values × 30° = 390°, max 360°").
  - When the types cannot see it (an array built at runtime, a `number` variable), it throws in development. In production it logs an error and renders.
- **Coordinates:** the dial uses Knob's view box: 0–100, centre (50, 50), dial edge at radius 50. The layout box is the dial square only. Print outside it uses values below 0 or above 100, paints outside the box and takes no layout space. The user leaves room for it with margin or grid gaps.

## Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `RotarySelector` | `values` | `readonly Value[]` | — | Order is index order |
|  | `value`, `defaultValue` | `Value` | `values[0]` | Never empty |
|  | `onValueChange` | `(value, details) => void` | — | `details.reason`: drag, keyboard, wheel, reset, label |
|  | `onValueCommitted` | `(value) => void` | — | At the end of every interaction, as Knob, even with no change |
|  | `resetValue` | `Value` | `defaultValue` | Double-click and Alt+click |
|  | `startAngle`, `stepAngle` | `number` | 0, 30 | Degrees |
|  | `format` | `(value) => string` | `String` | `aria-valuetext` and `RotarySelectorValue` |
|  | `dragDirection` | `"vertical" \| "horizontal" \| "circular"` | `"vertical"` | As Knob |
|  | `allowWheel` | `boolean` | `true` | Only while the dial has focus (plan 017) |
|  | `clickSound` | `boolean \| ClickSound` | `false` | `true` plays the selector's own click (plan 016) |
|  | `size` | `"sm" \| "default" \| "lg"` | `"default"` | Same steps and `--knob-size` as Knob |
|  | `disabled` | `boolean` | `false` | Whole control only. Also from `AudioConfigProvider` |
| `RotarySelectorDial` | `hitRadius` | `number` | the cap's radius | Size of the drag circle |
| `RotarySelectorPositionLeader` | `from` | `number` | — | Start radius |
|  | `ray` | `number` | — | Length of the radial segment |
|  | `to` | `{ x: number } \| { y: number }` | — | End of the run: a column `x` or a row `y` |
|  | `children` | `ReactNode \| ((leader) => ReactNode)` | — | Plain children sit at the leader's end. A function gets `{ end, bend, path }` |

There is no per-position `disabled`. A position that is not available is removed from `values`.

## Interaction

- **Keyboard:** only the dial is a tab stop. Arrow Up and Right turn clockwise, Down and Left anticlockwise, as Knob. With a negative `stepAngle`, clockwise means a lower index. Home and End go to the first and last values. Page Up and Page Down are not bound.
- **Drag:** only a circle around the cap starts a drag, sized by the cap's reported radius (plan 017) or by `hitRadius`. The dial box corners and all print do not.
  - The pattern is Knob's since PR #18: the dial element ignores the pointer except while dragging, and a transparent circle is the drag surface. The drag cursor and pointer capture stay on the dial during the drag.
  - Vertical and horizontal drags step one position per fixed pointer distance (24 px in the prototype) and follow the keyboard rule. Circular drag snaps to the nearest position. On a selector that does not wrap, the pointer can pass through the gap and the value stays at the end it reached.
- **Position labels:** clicking one selects its value. Labels are not tab stops and are `aria-hidden`.
- **Wheel:** the shared wheel rules from plan 017, one position per step: a hard detent, with no free spin.
- **Reset:** double-click or Alt+click on the drag circle.
- **Click sound:** with `clickSound`, every position change clicks, whatever its source. The selector's own sound:

  ```ts
  const rotarySelectorClick = createClickSound({
    tone: { hz: 460, decayMs: 1.3, gain: 1 },
    noise: { decayMs: 0.75, gain: 0.21 },
    pitchSpread: 0.03,
    volume: 0.18,
  });
  // played with minIntervalMs: 33
  ```

## Print and styling

- The print is nested inside `RotarySelectorDial` but does not respond to the pointer. Only the drag circle and the position labels do.
- Data attributes: `data-dragging`, `data-disabled` and `data-wraps` on the dial. `data-selected="true"` on position parts. This is an exception to the present-or-absent rule in plan 002: shadcn's `data-selected:` variant only matches `"true"`.
- CSS variables: `--knob-size` and `--knob-angle`, shared with Knob. A Knob and a selector of the same `size` line up centre to centre and take the same caps.
- Slots: `rotary-selector`, `rotary-selector-dial`, `rotary-selector-hit-area`, `rotary-selector-label`, `rotary-selector-value`, `rotary-selector-position-label`, `rotary-selector-position-mark`, `rotary-selector-position-leader`.

## Leaders

`RotarySelectorPositionLeader` draws two segments: a radial ray from `from` for `ray` units, then a horizontal run to `to.x` or a vertical run to `to.y`.

Impossible geometry has defined behaviour: a ray that points away from its column, a bend inside the dial, or a ray too short to clear it. The leader still draws its raw path, styled as invalid (red dashed in the docs theme), and logs one `console.warn` per problem in development. It does not throw and does not clamp.

## Accessibility

Same pattern as Knob, Fader and PanControl:

- The dial is `role="slider"`, with `aria-valuenow` as the index, `aria-valuemin` 0, `aria-valuemax` `values.length − 1`, and `aria-valuetext` from `format`.
- The name comes from `RotarySelectorLabel` through `aria-labelledby`, or from `aria-label` or `aria-labelledby` on the dial.
- The print SVG is `aria-hidden`.
- Disabled: `aria-disabled`, `tabIndex={-1}`, `data-disabled`.

## Delivery

One PR, after plans 016 and 017 are merged:

1. `components/ui/rotary-selector.tsx` and its `registry.json` item.
2. Unit tests: the keyboard rule for positive and negative steps, the wrap rule, the overflow error, label clicks, commit events, `format` in `aria-valuetext`, clicks on position changes. Type tests for the inferred union, `position()` and the overflow error on `stepAngle`.
3. e2e: the drag circle (corners and print do not drag), drag directions, label click, the wheel.
4. Docs page `content/docs/components/rotary-selector.mdx` with examples: VCO waveform with leaders, 808 instrument select (12 × 30°, wraps, numbers plus boxed names), 808 measures (no wrap, bracket line), a compact icon-only selector next to a Knob, and sizes.

Separate fix, not part of this plan: ChannelStrip's `data-selected:` ring probably never shows, because Base UI writes `data-selected=""`. Check it in a browser first.

## Out of scope

- Per-position `disabled`.
- Automatic label layouts, and automatic centring of the positions.
- An editable value readout. Keys are faster than typing a name, and the name may only be an icon.

## As built

Differences from the plan above:

- The prototype is not on a `prototype/rotary-selector` branch. It stays as files outside git in the planning worktree (`components/prototype/` and `app/(home)/prototype/`).
- The selector shares more with Knob than the cap. `knob.tsx` exports `pointAt`, `pointerAngle`, `turnBetween`, `useDialWheel` and `useDialClick`, and both dials use them.
  - `useDialWheel` returns a callback ref that also sets the user's `ref`. Before, a `ref` on `KnobDial` replaced the dial's own, and the wheel never attached.
  - `turnBetween` now works for angles more than one turn apart.
- Circular drag on a selector that does not wrap follows the pointer's unwrapped angle. On press, the angle moves to within half a turn of the middle of the positions. Each move then adds the short way round. The index is that angle, rounded to a step and clamped. So the ends are hard stops: a pointer that goes on around past an end must come back the way it went. A state machine for the gap was tried first, and each fix showed a new case.
- On a selector that wraps, circular drag goes to the nearest position by circular distance. A tie goes to the lower index. Rounding by the step drifted one position each turn when the step is rounded, such as 51.4° for 7 values.
- More props are checked. An empty `values`, or a `stepAngle` of 0 or not finite, throws in development and logs an error in production, as overflow does. A `value`, `defaultValue` or `resetValue` that is not in `values` logs a warning in development. The type error on `stepAngle` applies only to a literal tuple with a whole-number literal step.
- Only changes from the user click: drag, keys, wheel, label and reset. A controlled `value` changed from outside does not click, as with Knob.
- Leaders warn only about real geometry errors: a ray that points away from its column or row, and a bend past it. A bend inside the dial and a short ray can look right, as in the waveform demo, so they do not warn.
- The focus ring is not on the dial box. It is an SVG circle 2 units outside the drag circle (`hitRadius`, else the cap's radius, else 50), 3 px wide at every size, with the slot `rotary-selector-focus-ring`. Around the box, it stood far from the cap. Knob still has no focus ring.
- The docs also show the drag directions, and a selector with `KnobPointer` in place of the metal cap. The page has its own social image.
- Still open: the ChannelStrip `data-selected:` check.
