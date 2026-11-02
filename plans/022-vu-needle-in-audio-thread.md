# 022 — VU needle in the audio thread

Status: IMPLEMENTED (2026-11-02). See [Outcome](#outcome). Follows plan 013. Replaces its [Needle ballistics](013-vu-meter.md#needle-ballistics) and [Painting](013-vu-meter.md#painting) sections, and its data props.

The needle felt slow and too regular. Two smoothing stages added up (an RMS detector, then a spring stepped on animation frames), and the demo fed it made-up levels. This plan moves the whole needle into an AudioWorklet that runs on every sample: a rectifier, a resonant low-pass and two stops. The worklet sends the needle position to the main thread, which only paints it.

The settings come from a spike on the docs bench (local branch `prototype/vu-ballistics`, Codex worktree `ac8c`), tuned by ear and eye with a test tone and a microphone.

## Decisions

- **Audio signals only.** `VuMeter` takes an audio `input`. `source`, `peakDb`, `rmsDb`, `channels`, `measure` and `actionsRef.paint` go. A needle needs the signal itself; dB values give it nothing to rectify or integrate.
- **`|x|`, not an RMS detector.** The coil of a real meter gets the full-wave rectified voltage, and the movement does the averaging. The scale is calibrated so that a sine at `referenceDb` (RMS) reads 0 VU. On the bench, `x²` needed a square root after the filter and made the fall slower than the rise; `|x|` felt right.
- **The ballistics are one 2nd-order low-pass.** Two props, `lowPassCutoffHz` (default 2) and `lowPassQ` (default 0.6). At these values a step reaches 99% in about 340 ms and overshoots by about 1%, close to the VU standard (300 ms, 1–1.5%). Q 0.7071 overshoots 4.3%; only Q ≤ 0.5 never overshoots.
- **The filter runs as a mass on a spring.** Position and velocity are its state, so the stops can act on them. Away from the stops it is the same filter as an RBJ low-pass at that cutoff and Q.
- **Stops act on the state, not on the output.** Upper stop at 1.04, just past the top of the scale: the velocity flips and keeps `rebound` of its speed (default 0.5). The spring keeps pulling, so the bounces die out and the needle rests on the stop. Lower stop at −0.04, just under the `minDb` mark: the needle stops there with speed 0. A memoryless wavefolder was tried and rejected: it reads a steady overload as a lower level, and a hot input either falls back to rest or pins at the fold floor.
- **Every sample, on the audio clock.** Frame rate, hidden tabs and main-thread stalls do not change how the needle moves.
- **Position out, not angle.** The worklet sends the scale position. The face parts keep the angle (sweep, pivot).
- **Numeric props are AudioParams.** `lowPassCutoffHz`, `lowPassQ`, `rebound`, `referenceDb`, `minDb`, `maxDb`, all k-rate. A prop change sets a param; no message protocol.
- **Pure physics, tested directly.** `lib/audio/needle.ts` holds two self-contained functions with no imports: `needleConstants` (props to coefficients) and `stepNeedle` (one sample). Unit tests call them; the worklet source includes them with `Function.prototype.toString`.
- **No "instant" ballistics.** Instant movement on a rectified signal follows each half-cycle. With reduced motion the physics stay the same and the meter paints four times a second.
- **Static needles use CSS.** A page with no audio (social previews) sets `--vu-level` and `aria-valuenow` on the root. The meter leaves `--vu-level` alone until it has had an input; when an input goes away, it paints rest.
- **Failures are visible.** `useVuNeedle` reports `status` (`idle`, `loading`, `running`, `suspended`, `error`) and an `error` that says why: no Web Audio, a closed context, a processor that could not load, or one that stopped on the audio thread (`processorerror`). `VuMeter` shows the status as `data-status`, so a broken meter no longer looks like silence.
- **`reset()` paints rest at once.** It resets the worklet and paints the rest position on the main thread, because a suspended context sends no more positions.
- **Docs play the "work work" sample.** Examples loop `/sounds/peon-work-work-loop.wav` (the sample with 1 s of silence after it) through `useSound` and feed `sound.output` to the meter, behind a play button.

## Signal chain

```
input ─▶ AudioWorkletNode (silent tap, 2 channels, speakers mix) ─▶ destination
           per sample:
             drive    = |x[channel]|, or the larger of |L| and |R|
             target   = drive × gain + offset            (calibrated scale position)
             a        = ω²(target − y) − 2ζω·v           (ω = 2π·lowPassCutoffHz, ζ = 1 / (2·lowPassQ))
             v += a·dt;  y += v·dt                       (semi-implicit Euler)
             y > 1.04  → reflect: v = −v × rebound, or rest on the stop once the bounce is shorter than a step
             y < −0.04 → y = −0.04, v = 0
           every intervalMs: postMessage(y)
main thread: onmessage ─▶ skip if off screen ─▶ --vu-level, data-zone, data-active, data-pinned, ARIA
```

### Calibration

The scale law of plan 013 is affine in amplitude: `position = (amplitude − dbToGain(minDb)) / span`, with `span = dbToGain(maxDb) − dbToGain(minDb)` and amplitude 1 at 0 VU. The mean of a full-wave rectified sine is `2√2/π` times its RMS, so:

```ts
const referenceMean = ((2 * Math.SQRT2) / Math.PI) * dbToGain(referenceDb);
const gain = 1 / (referenceMean * span);
const offset = -dbToGain(minDb) / span;
```

The low-pass is linear and passes DC unchanged, so filtering the position is the same as filtering the amplitude.

## API

```tsx
const sound = useSound("/sounds/peon-work-work-loop.wav", { loop: true });

<VuMeter aria-label="Program" input={sound.output} />;
```

| Prop | Type | Default |
| --- | --- | --- |
| `input` | `MediaStream \| HTMLMediaElement \| AudioNode \| null` | — |
| `channel` | `0 \| 1 \| "max"` | `"max"` |
| `referenceDb`, `minDb`, `maxDb`, `zones`, `variant`, `pivot` | as in plan 013 |  |
| `ballistics` | `Partial<{ lowPassCutoffHz; lowPassQ; rebound }>` | `{ 2, 0.6, 0.5 }` |
| `actionsRef` | `Ref<{ reset(): void }>` | — |

The root adds `data-status` to the data attributes of plan 013.

`useVuNeedle(input, onPosition, options)` owns the worklet node, for anyone building a custom meter, and returns `{ status, error, reset }`. It has its own docs page and registry item; the worklet source ships in that item, not in `core`.

## Tests

Written first.

- `lib/audio/needle.test.ts`, on `stepNeedle` with a generated sine at 48 kHz: settles on 0 VU at `referenceDb` for any cutoff and Q; 99% time and overshoot match the 2nd-order response; a steady tone over the top rests on the upper stop; bounces grow with the impact speed and die out; `rebound: 0` stops dead; falling into silence never goes below the lower stop.
- `lib/audio/vu-worklet.test.ts`: runs the module source in a fake `AudioWorkletGlobalScope`; matches `stepNeedle`, follows params, picks the channel, posts at `intervalMs`, handles `"reset"` and `"stop"`.
- `hooks/use-vu-needle.test.tsx`: connects and disconnects only its own node, survives StrictMode, sets params without rebuilding the node, starts with params changed while it loaded, moves to a new input, and reports each status and error.
- `components/ui/vu-meter.test.tsx`: drives positions through a mocked `useVuNeedle`; `--vu-level`, data attributes including `data-status`, ARIA throttling, off-screen skipping, reduced motion, `reset()`, a CSS-set needle, and everything about the face that plan 013 already tests.

## Delivery

1. Physics and its tests.
2. Worklet and its tests.
3. `useVuNeedle` and its tests.
4. `VuMeter` on `input`; the frame painter goes.
5. Examples on the sample, the social preview on `--vu-level`, docs, registry.
6. Point plan 013's status line here.

## Outcome

Implemented on PR #6 in `005e9bf` (the needle) and `f609f35` (docs and examples).

- **As planned:** the signal chain, the defaults (2 Hz, Q 0.6, rebound 0.5), the stops, the AudioParams, and the tests above, written first.
- **Added on review:** `data-status` and the error statuses; `reset()` painting rest; `VU_SCALE` as the one source of the default scale; a processor name that does not need `crypto.randomUUID` (it throws outside secure contexts and broke any page with a meter); no allocation in the worklet's render loop.
- **Checked by hand:** the needle moves in the dev server and in a minified production build, so the source text the worklet gets survives minification.
- **On a steady tone above the top**, each half-cycle tugs the needle off the upper stop by less than 0.1% of the scale. The tests allow for it; nobody can see it.
- **Social image:** the generator re-rendered every page's image with a new hash on this machine (rendering differences, not content). Only the new `use-vu-needle` image was kept.
