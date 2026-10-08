# 016 — Click sound helper

Status: IMPLEMENTED (2026-10-08), PR #19. Prerequisite for plan 018 (rotary selector).

Today, Knob's `clickSound` is a fixed tick: its numbers are module constants in `components/ui/knob.tsx`, and the only public setting is on or off. The rotary selector needs a different sound (a low thud, tuned by ear), and Knob keeps its own. A shared helper builds both from the same model, so each component tunes its own sound and users can make variants.

The helper also fixes a bug: Knob's `playClick` always uses the shared page context and plays straight to the speakers. It ignores `AudioContextProvider` and cannot be routed into a bus.

## The sound model

The same synthesis as Knob's `clickBuffer`: a short mono buffer that is the sum of two parts, each `gain × wave × e^(−t / decay)`.

- **Noise:** `wave = Math.random() * 2 − 1`, a snap.
- **Tone:** `wave = sin(2π · hz · t)`, a ring.

At play time, `playbackRate` moves at random within `±pitchSpread`, which shifts pitch and length together like a real detent, and the output goes through a gain of `volume`.

## API

`lib/audio/click.ts`:

```ts
const tick = createClickSound(); // defaults: Knob's current tick
const thud = createClickSound({
  tone: { hz: 460, decayMs: 1.3, gain: 1 },
  noise: { decayMs: 0.75, gain: 0.21 },
  pitchSpread: 0.03,
  volume: 0.18,
});
const brighter = thud.with({ tone: { hz: 900 } }); // deep partial merge over thud

thud.play({ context, destination, when, minIntervalMs: 33 });
```

```ts
interface ClickSound {
  /** Plays once. Resumes a suspended context (needs a user gesture). */
  play: (options?: ClickPlayOptions) => void;
  /** A new sound with these parameters deep-merged over this one's. */
  with: (changes: ClickSoundChanges) => ClickSound;
  /** The full parameters after merging, for docs or a tuning UI. */
  readonly params: Readonly<ClickSoundParams>;
}
```

- **Sound parameters:** `tone: { hz, decayMs, gain }`, `noise: { decayMs, gain }`, `lengthMs`, `pitchSpread` (a ratio, 0.03 = ±3%), `volume` (linear, like `useSound`). Times carry the `Ms` suffix, as the meters' `attackMs` and `releaseMs` do.
- **Play options:** `context` (default: the shared page context), `destination` (default: the context's speakers), `when` (context time, for scheduled clicks), `minIntervalMs` (rate limit).
- **Defaults** are Knob's current tick: tone 4200 Hz, 1.2 ms, gain 0.4; noise 0.4 ms, gain 0.6; length 6 ms; spread ±4%; volume 0.12.
- `createClickSound` and `.with` take the same deep partial. Missing fields come from the base sound: the defaults for `createClickSound`, the sound itself for `.with`.
- The buffer is built once per context and per sound.

## Rate limit

The rate limit is a playback concern, so it is a play option, not a sound parameter.

- Each sound keeps its own timer. Today one timer is shared by the whole page. In practice the result is the same, because clicks only come from user input and a user moves one control at a time.
- Scheduled playback (`when`) leaves `minIntervalMs` out, so a metronome is never throttled by wall-clock time.

## Use in components

- A component keeps its own sound as a constant (`knobClick`, `rotarySelectorClick`). Users who want a different default edit that constant in their copy of the source.
- The `clickSound` prop is `boolean | ClickSound`. `true` plays the component's own sound, and a `ClickSound` replaces it. Passing the same sound to a Knob and a selector gives a panel one consistent click.
- Components play through the `useAudioContext()` context, so `AudioContextProvider` is honoured.
- Whether a change clicks stays the component's job. Knob clicks when a change reaches or crosses a detent (`KnobScale`'s long ticks, or each `largeStep`).

## Delivery

One PR:

1. `lib/audio/click.ts`, added to the `core` registry item.
2. Unit tests: the deep merge, `.with` on a derived sound, the per-sound rate limit, `when` not throttled, the buffer cached per context and sound.
3. Knob moves to `createClickSound()` with the same sound, plays through the `useAudioContext()` context, and its `clickSound` prop accepts `boolean | ClickSound`. The existing click tests in `components/ui/controls.test.tsx` still pass.
4. Knob docs: the `clickSound` row shows the new type, plus a short example of a custom sound.

No audible change for existing Knob users.

## Out of scope

- Named presets (`clickSound="tick"`).
- A panel-wide sound through `AudioConfigProvider`. It can be added later on top of `ClickSound`.
- Changing Knob's sound.

## As built

Differences from the plan above:

- Knob gets its context from the new `useProvidedAudioContext()`. It returns the provider's context, or `null`, and never creates one. Before, every Knob created and resumed the shared page context, even with no click sound. Now a silent Knob touches no AudioContext, and `play()` uses the shared page context only when it plays.
- A suspended context needs more care than the plan said:
  - `play()` asks the context to resume every time, because only a resume inside a user gesture starts it. A later gesture can start a context whose first resume hangs.
  - While the context resumes, it holds at most one click. Dropping all clicks lost the first real click on Firefox, Safari and iOS. Holding all of them played a burst of stale clicks later.
  - If the browser refuses the resume, the held click stops and disconnects.
  - A click that the queue drops does not restart the rate limit.
- Each sound caches its buffer in a `WeakMap` keyed by context.
- `test/fake-audio.ts` records the buffers and sources it creates, so tests can check what plays.
- The `core` registry item lists `lib/audio/click.ts`, and now depends on `use-audio-context`.
