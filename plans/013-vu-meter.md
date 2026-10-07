# 013 — VU meter

Status: IMPLEMENTED (2026-10-02). See [Outcome](#outcome). Follows plan 002; reuses the meter plumbing from plan 004.

An analog VU meter: a backlit face in a dark bezel, a printed scale from −10 to +3 VU with a red section above 0, and a needle that swings with real VU ballistics. One meter shows one channel; a stereo pair is two meters side by side, labelled L and R.

## The reference

What the design shows, and what the component has to match:

- **The scale is a smile, not a frown.** The arc dips in the middle, and every tick points outward from a centre _above_ the face. The needle hangs from a pivot above the window, hidden behind the badge, and swings across the bottom. Most VU meters pivot from below; this one doesn't, and the geometry follows it.
- **The scale is linear in amplitude, not in dB.** −10 to −7 is squeezed at the left, −3 to 0 is wide, and 0 VU sits about 62% along the arc. That is how a real VU movement reads, because the needle follows voltage.
- **Marks:** numbers at −10, −7, −5, −3, −2, −1, 0, +1, +2, +3; unnumbered ticks at −6 and −4. Ticks hang below the arc, with numbers below them, upright (not turned to follow the arc). The ends of the arc finish in a short outward stroke.
- **The red section** runs from 0 to +3: the arc there is thicker, and its ticks and numbers are red.
- **The face** is backlit: bright near the top centre, falling to grey at the corners, with a soft inner shadow under the bezel lip and a faint glass sheen.
- **The needle** is a thin dark line with a faint shadow beside it, cast on the face by the backlight.
- **Corner text:** a legend at the bottom left ("RMS" plus a red figure) and a channel letter at the bottom right ("L").
- **The badge** at the top centre covers the top of the needle. The badge in the reference is another company's logo; we ship an empty slot, not that mark.

## Approach

A new component, `vu-meter`, not a `variant` of `LevelMeter`. A needle meter has a different scale law, different ballistics, a different shape and different parts. It reuses everything below the parts: `FrameSource`, `createFrameEmitter`, `subscribeFrame`, `useFrameSource`, `useVisibility`, `useReducedMotion`, `formatDb` and the zone tokens.

Two additions to `lib/audio`, both useful outside this component:

1. A `vu` taper, so the scale law can be reused by `DbScale` and `LevelMeter`.
2. A second-order needle integrator, because the existing `vu` ballistics preset is a first-order smoother and a needle without inertia and overshoot looks wrong.

## Anatomy

```tsx
<VuMeter source={analyser.meter} channel={0} aria-label="Left channel">
  <VuMeterFace>
    <VuMeterScale />
    <VuMeterNeedle />
    <VuMeterBadge>{/* your mark */}</VuMeterBadge>
    <VuMeterLegend>VU</VuMeterLegend>
    <VuMeterLabel>L</VuMeterLabel>
  </VuMeterFace>
</VuMeter>
```

With no children, `<VuMeter rmsDb={-18} />` renders the face, the scale, the needle and a "VU" legend. That is a complete meter.

Every part is HTML on the outside. `VuMeterScale` and `VuMeterNeedle` each draw their own absolutely positioned `<svg>` with the same 200 × 100 view box, so SVG and HTML parts stack in DOM order and any part can be left out, reordered or replaced. The badge comes after the needle so it covers the needle's top end, as in the reference.

## Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `VuMeter` | `source` | `FrameSource<MeterFrame>` | — | Subscribes and paints without React renders |
|  | `peakDb`, `rmsDb` | `number` | — | Mono declarative input, dBFS |
|  | `channels` | `ChannelLevel[]` | — | Declarative multi-channel input |
|  | `channel` | `number \| "max"` | `"max"` | Which channel the needle shows, as in `DbReadout` |
|  | `measure` | `"rms" \| "peak"` | `"rms"` | Falls back to peak when a frame has no RMS |
|  | `referenceDb` | `number` | −18 | The dBFS level that reads 0 VU (EBU R68). −20 for SMPTE |
|  | `minDb`, `maxDb` | `number` | −10, +3 | The printed scale, in VU |
|  | `zones` | `MeterZone[]` | ok, then clip from 0 | In VU. Colours the arc, ticks and numbers |
|  | `ballistics` | `"vu" \| "instant" \| Partial<NeedleOptions>` | `"vu"` | How the needle moves |
|  | `variant` | `"classic" \| "flat"` | `"classic"` | See [Materials](#materials) |
|  | `actionsRef` | `Ref<VuMeterActions>` | — | `paint(frame)`, `reset()`, as on `LevelMeter` |
| `VuMeterScale` | `ticks` | `number[]` | −10, −7, −5, −3, −2, −1, 0, 1, 2, 3 | Numbered ticks, filtered to the range |
|  | `minorTicks` | `number[]` | −6, −4 | Unnumbered ticks |
|  | `format` | `(vu: number) => string` | `"+3"`, `"0"`, `"−7"` | Text for each number |
| `VuMeterNeedle` | — |  |  | The needle and its shadow |
| `VuMeterBadge`, `VuMeterLegend`, `VuMeterLabel` | `ComponentProps<"div">` |  |  | Top centre, bottom left, bottom right |

`minDb` and `maxDb` are the numbers printed on the scale, relative to `referenceDb`, not dBFS. `<VuMeter rmsDb={-18} />` reads 0, and so does `<VuMeter rmsDb={-20} referenceDb={-20} />`. For the broadcast scale, set `minDb={-20}` and add −20 to `ticks`.

Inside a `ChannelStrip` or `Mixer`, `VuMeter` takes `dimmed` from the audio config but ignores its `minDb`, `maxDb`, `zones` and `ballistics`. Those are dBFS settings for bar meters and would put the needle on the wrong scale.

## The scale law

A VU needle's swing is proportional to signal amplitude. Over a printed range `minDb..maxDb`:

```ts
position =
  (dbToGain(vu) - dbToGain(minDb)) / (dbToGain(maxDb) - dbToGain(minDb));
```

Over −10..+3 this puts the marks where the reference has them:

| VU       | −10 | −7   | −5   | −3   | −2   | −1   | 0    | +1   | +2   | +3  |
| -------- | --- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | ---- | --- |
| Position | 0   | 0.12 | 0.22 | 0.36 | 0.44 | 0.53 | 0.62 | 0.74 | 0.86 | 1   |

Silence maps below 0 (−0.29 over this range), so the needle rests against the left pin, just past the end of the arc, as a real meter does. It is added to `lib/audio/taper.ts` as `vuTaper(min, max)` and as `"vu"` in `TaperInput`, so `<DbScale taper="vu" />` and `<LevelMeter taper="vu" />` work too.

## Needle ballistics

A real VU meter is a damped spring: it reaches 99% of a step in 300 ms and overshoots by 1 to 1.5% (IEC 60268-17). The existing `BALLISTICS.vu` preset can't overshoot, so a new integrator lives in `lib/audio/needle.ts`:

```ts
interface NeedleOptions {
  /** Time to reach 99% of a step. Default 300. */
  riseMs: number;
  /** Overshoot past a step, 0..1. Default 0.015. */
  overshoot: number;
}

const needle = createNeedle({ maxPosition: 1.04, minPosition: -0.04 }, "vu");
needle.step(targetPosition, nowMs); // the position to draw
needle.reset();
```

- It integrates scale position, which is linear in amplitude, so the physics are those of the real movement.
- The damping ratio comes from the overshoot: `ζ = −ln(os) / √(π² + ln²(os))`, 0.80 for 1.5%. The natural frequency is solved once so the first crossing of 99% lands on `riseMs`: 13.1 rad/s for 300 ms. A check with 1 ms steps gives 99% at 301 ms and a 1.41% overshoot.
- Semi-implicit Euler in sub-steps of at most 2 ms, so uneven frames don't change how it moves. A gap over 250 ms (a hidden tab) snaps to the target instead of replaying the swing.
- Pins at both ends: the needle stops at `minPosition` and `maxPosition` and loses its speed there, so a loud signal pegs it against the right pin.
- `"instant"` follows the target exactly.

## Painting

The painter follows `LevelMeter`'s: one `subscribeFrame` listener per meter, nothing in React state.

- Each frame: read the chosen channel and measure, convert dBFS to VU with `referenceDb`, map it through the taper, step the needle, and write `--vu-level` on the root when it moves more than an epsilon.
- `VuMeterNeedle` turns with CSS, from `--vu-level`: `rotate(calc(var(--vu-from) + var(--vu-level) * var(--vu-sweep)))` around the pivot, with `transform-box: view-box`. A user can build their own needle, or a whole custom meter, from `--vu-level` alone.
- `data-zone` on the root follows the needle's zone; `data-active` is set while the needle is off the left pin; `data-pinned` is set while it rests against the right pin.
- Skips frames while off screen (`useVisibility`).
- Reduced motion: `instant` ballistics, painted four times a second, as on `LevelMeter`.

## Geometry

Starting values in the 200 × 100 view box, measured from the reference. Tune them in the browser.

| Constant      | Value        | Meaning                                     |
| ------------- | ------------ | ------------------------------------------- |
| Pivot         | (100, −108)  | Above the window, behind the badge          |
| Arc radius    | 170          | The arc's lowest point sits at y ≈ 62       |
| Sweep         | 58°          | ±29° around straight down                   |
| Tick length   | 5, minor 3.5 | Outward from the arc                        |
| Number radius | arc + 12     | Numbers upright, centred on the tick's line |
| Red arc width | 2.5          | The rest of the arc is 0.8                  |
| Pin overshoot | 0.04         | Of the sweep, past each end                 |

The needle runs from the pivot to just short of the bottom edge. The face clips it, so only the part below the top edge shows.

## Materials

### `classic` (default)

The reference look, the same in light and dark themes, like `KnobCap`'s aluminium.

- **Bezel** (`VuMeter`): a near-black frame with a lighter top edge, padding for the frame width, and `rounded-lg` from the theme's radius.
- **Face** (`VuMeterFace`): a `rounded-md` window with a fixed `aspect-[2/1]`. Background: a radial backlight from the top centre (`--vu-face`) to the corners (`--vu-face-shade`). An `after:` layer on top of everything adds the inner shadow under the bezel lip and a faint diagonal glass sheen, with `pointer-events-none`.
- **Ink:** the arc, ticks, numbers and corner text in `--vu-ink`, a soft charcoal rather than pure black.
- **Needle:** `--vu-needle`, about 0.7 units wide, plus a blurred copy offset 1.5 units sideways at 30% opacity as the backlight shadow.

### `flat`

The same parts on theme tokens, for apps that want the meter to sit in a shadcn UI: `bg-card` face, `border` bezel, `foreground` ink, no glow or glass.

### Sizing

The meter fills its container's width and keeps its shape. `VuMeterFace` is a size container, and the HTML corner text is sized in `cqw`, so a 12rem meter and a 40rem meter look the same, just larger.

## Theming

| Variable or attribute | On | Meaning |
| --- | --- | --- |
| `--vu-face`, `--vu-face-shade` | root | Backlight centre and corners. Set an amber `--vu-face` for a warm lamp |
| `--vu-ink` | root | Scale and text |
| `--vu-needle` | root | Needle |
| `--vu-bezel` | root | Frame |
| `--vu-zone-ok`, `--vu-zone-warn`, `--vu-zone-clip` | root | Zone colours. `ok` defaults to `--vu-ink`, because a VU scale prints its normal range in ink; `warn` and `clip` default to `--meter-warn` and `--meter-clip` |
| `--vu-level` | root | Live needle position, written by the painter |
| `data-variant` | root | `classic` or `flat` |
| `data-zone` | root | `ok`, `warn` or `clip`, at the needle |
| `data-active` | root | The needle has left the left pin |
| `data-pinned` | root | The needle rests against the right pin |
| `data-slot` | every part | `vu-meter`, `vu-meter-face`, `vu-meter-scale`, `vu-meter-tick` (`data-major`, `data-zone`), `vu-meter-scale-label`, `vu-meter-needle`, `vu-meter-badge`, `vu-meter-legend`, `vu-meter-label` |

## Accessibility

- The root is `role="meter"`, with `aria-valuemin` and `aria-valuemax` from the printed scale, `aria-valuenow` in VU and `aria-valuetext` such as "−3.0 VU", updated at most four times a second.
- It needs a label: `aria-label` or `aria-labelledby`.
- The SVG layers and the badge are `aria-hidden`. The legend and channel letter are decorative; the label carries the channel name.

## Delivery

1. **Core:** `vuTaper` and `"vu"` in `lib/audio/taper.ts`; `createNeedle` and `NEEDLE_BALLISTICS` in `lib/audio/needle.ts`, added to the `core` registry item's files.
2. **Component:** `components/ui/vu-meter.tsx` with `VuMeter`, `VuMeterFace`, `VuMeterScale`, `VuMeterNeedle`, `VuMeterBadge`, `VuMeterLegend`, `VuMeterLabel`, and the exported `vuMeterVariants`.
3. **Examples** in `components/examples/`:
   - `vu-meter-demo`: one meter on a `useDemoSignal` music source, styled after the reference, with a legend in the reference's style ("RMS" and the calibration in red).
   - `vu-meter-stereo`: an L and R pair on one stereo source.
   - `vu-meter-calibration`: −18 against −20 reference, and the −20 to +3 broadcast scale.
   - `vu-meter-variants`: `classic`, `flat`, and a warm amber lamp done with `--vu-face`.
   - `vu-meter-microphone`: a live microphone through `useAudioAnalyser`.
4. **Docs:** `content/docs/components/vu-meter.mdx` with the plan 002 sections, a "Calibration" section on `referenceDb`, and "Feeding data". Add it to `meta.json` under meters and to the table in `components/index.mdx`.
5. **Site wiring:** `components/docs/example-registry.tsx`, `registry.json` (deps: `class-variance-authority`; registry deps: `@audiocn/core`, `@audiocn/use-audio-config`, `@audiocn/use-frame-source`, `@audiocn/use-reduced-motion`, `@audiocn/use-visibility`), the component name pattern in `oxlint.config.ts`, the route in `e2e/themes.spec.ts`, and a social preview in `components/social/social-previews.tsx` and `lib/social-catalog.ts` (the catalog test requires one per component). Then `pnpm og:build`.
6. **Checks:** typecheck, `pnpm check`, unit tests, e2e themes, and a visual check in the browser next to the reference in both themes.

## Tests

- `taper.test.ts`: `vuTaper` puts the marks at the positions in the table, round-trips through `toValue`, and maps silence below 0.
- `needle.test.ts`: a step reaches 99% in 300 ms ± 5% and overshoots between 1 and 1.5%; the result is the same with 8 ms and 33 ms frames; the needle stops at both pins and loses its speed there; a long gap snaps; `instant` follows exactly.
- `vu-meter.test.tsx`:
  - Source, values and `actionsRef.paint` give the same `--vu-level`.
  - `referenceDb` shifts the reading: −18 dBFS reads 0 VU by default, −20 dBFS reads 0 VU with `referenceDb={-20}`.
  - `channel` and `measure` pick the right reading, and RMS falls back to peak.
  - `data-zone` switches at 0 VU; `data-pinned` is set at the right pin.
  - Tick and number counts follow `ticks`, `minorTicks` and the range; ticks in the clip zone carry `data-zone="clip"`.
  - ARIA values are in VU and throttled.
  - Reduced motion paints without inertia.
  - The painter unsubscribes on unmount.

## Decisions

All four were taken as proposed; the pivot was revised after review.

- **The needle pivots from above**, as in the reference. A classic bottom pivot (a frown-shaped arc) could be a later `pivot` prop; the parts don't change, only the geometry. _Revised after review: see [Pivot](#pivot)._
- **The default range is −10 to +3**, as in the reference, not the −20 to +3 broadcast scale.
- **The default `referenceDb` is −18 dBFS** (EBU R68, common in plugins). SMPTE users set −20.
- **No peak LED** in this plan. The reference has none; one could come later as `VuMeterPeak`, built on `ClipIndicator`.

## Outcome

Everything in [Delivery](#delivery) shipped: `vuTaper` and `"vu"` in `lib/audio/taper.ts`, `createNeedle` in `lib/audio/needle.ts`, `components/ui/vu-meter.tsx`, five examples, the docs page, the registry item and the site wiring.

Verified with:

- unit tests: 329 pass, 30 of them new (taper, needle, component);
- typecheck;
- `ultracite check`;
- `pnpm react-doctor` (no issues);
- a production build;
- the full e2e suite (656 pass), with the VU Meter page in the themes spec;
- a visual check in the browser against the reference, in both themes.

Where the work departed from the plan:

- **Geometry.** The pivot sits at (100, −104), not −108, so the arc's lowest point is at y 66 and its ends at y 45, as measured from the reference.
- **Classic red.** `classic` uses a fixed red, `oklch(0.64 0.19 24)`, instead of `--meter-clip`. The dark theme's destructive token is lighter for dark backgrounds and washed out on the lit face. `flat` still uses `--meter-clip`.
- **Needle rotation.** A `rotate-[…]` class on the needle group instead of inline `--vu-from` and `--vu-sweep` variables, because lint forbids inline styles in `ui` files. The pivot and sweep in the class match `PIVOT` and `SWEEP`.
- **Needle shadow.** Its blur filter is in user units, because a vertical line has a zero-width bounding box and a relative filter region drew nothing. `flat` hides the shadow.
- **`createNeedle(ballistics, stops)`.** With stops, the needle starts at rest against the low pin, so a meter swings up when it mounts.
- **The taper.** `vuTaper` isn't clamped, so silence maps below 0 and the needle can rest past the end of the scale. The named `"vu"` taper clamps to 0..1 for `LevelMeter` and `DbScale`.
- **Channel reading.** The meter reuses `readChannel` from `db-readout`, so the registry item depends on `@audiocn/db-readout`.
- **Arc slot.** The scale's arcs are `vu-meter-arc` parts with `data-zone`, which the plan didn't list.
- **The demo badge** uses theme radius and spacing tokens, because examples can't use arbitrary values. Only its icon scales with the meter.
- **React Doctor.** `vu-meter.tsx` joins `level-meter.tsx` in the `prefer-tag-over-role` waiver: a `<meter>` element can't hold the face, scale and needle.
- **Social images.** `og:build` re-encoded five unrelated images with new hashes; only the new VU Meter image was kept.
- **Not verified:** `pnpm test:install`. Scaffolding the fresh Next.js fixture failed on pnpm's trust check (`ERR_PNPM_TRUST_DOWNGRADE` for `undici-types@6.21.0`) before any audiocn item was added. The built `public/r/vu-meter.json` and `core.json` include the new files.

## Pivot

Added after review (2026-10-02). Most VU meters pivot from below, so `pivot="bottom"` is the default and the reference's hanging needle is `pivot="top"`.

|  | `bottom` (default) | `top` |
| --- | --- | --- |
| Pivot | (100, 156), below the window | (100, −104), above the window |
| Scale | An arc over the pivot, radius 125, 80° | An upturned arc, radius 170, 58° |
| Numbers | Above the ticks | Below the ticks |
| Needle base | Hidden by a bezel lip concentric with the pivot (`vu-meter-cover`, radius 66) | Covered by the badge |
| Badge | Mid-face, under the needle | Top centre, over the needle |

- Ticks point away from the pivot in both, so one geometry table (`GEOMETRY`) drives the scale, the needle and the lip. `--vu-level` means the same on both.
- The needle sits at `z-10`; the badge goes over it only on a top pivot, so the order of the parts doesn't matter. The glass layer is `z-30`.
- `data-pivot` is on the root, the scale and the needle.
- A new example, `vu-meter-pivot`, shows both, and the social image now shows the bottom pivot.
