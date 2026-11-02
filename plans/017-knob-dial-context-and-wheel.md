# 017 — Knob: shared dial context and wheel

Status: IMPLEMENTED (2026-10-08). Part 1 is PR #20, part 2 is PR #21. Prerequisite for plan 018 (rotary selector). Follows PR #18, which limited Knob's drag area to the dial circle.

Two Knob changes found while designing the rotary selector. Each is its own PR.

---

## 1. Shared dial context and cap radius

### Why

The rotary selector reuses `KnobCap` and `KnobPointer`. The cap is the physical part, and it should not depend on what turns under it: a potentiometer (Knob) or a switch (RotarySelector). Today both parts read Knob's own context (`useKnob`), so they only work inside a Knob. The prototype got around this by wrapping the selector in a hidden `<Knob>`. That is not suitable for production.

The selector's drag circle should also hug the cap, so the cap needs to tell the dial its size.

### Design

- A small dial context that any dial provides: `{ angle, disabled, registerCapRadius }`. `KnobDial` provides it now, and `RotarySelectorDial` will provide it in plan 018.
- `KnobCap` and `KnobPointer` read their angle and disabled state from it instead of from `useKnob`.
- `KnobCap` reports its outer radius: 25.5 for the default cap and 32 for `mini`. `KnobCap` gets no new prop. A dial that needs a larger drag circle (for example a ring drawn around the cap) sets its own `hitRadius` (plan 018).
- Knob keeps its full-disc drag circle (radius 50). It does not use the reported radius.

### Delivery

One PR, with no visible change:

1. The dial context in `components/ui/knob.tsx`, exported for other dial components.
2. `KnobCap` and `KnobPointer` move to it. They keep a clear error when used outside any dial.
3. Unit tests: the cap follows the angle, and the reported radius for both variants.

---

## 2. Knob wheel

### Why

The rotary selector work found that the wheel needs to tell a mouse wheel from a trackpad, and must ignore momentum. One threshold cannot serve both. Knob should feel the same as the selector, and the wheel is safe to have on by default, because it only acts while the dial has focus.

### Design

- `allowWheel` defaults to `true`.
- The wheel rules live in one shared module, so Knob and RotarySelector behave the same. Knob steps by `step` per notch. The selector steps by one position.
- **Notch input** (a mouse wheel) gives one step per event. An event is a notch when:
  - `deltaMode` is line or page (Firefox). Read it before `deltaY`, because Firefox reports 0 otherwise;
  - or the legacy `wheelDeltaY` is a non-zero multiple of 120, except when it equals −3 × `deltaY` (macOS trackpads in Chrome and Safari);
  - or |delta| is 50 px or more and comes 40 ms or more after the previous event.
- **Smooth input** (a trackpad) adds up pixel deltas and steps when the sum reaches 30 px. It takes at most one step per event and keeps the remainder below the threshold. It resets after 150 ms idle and on a reversal.
- **Momentum and free spin are ignored.** Only dense events can lock: smooth events under 40 ms apart, notches under 25 ms apart. A stream locks when any of these holds:
  - 3 shrinking deltas in a row, the current one under 0.8 × the gesture's peak;
  - over a window of 4, the deltas never grow, their mean is under 0.85 × the mean of the 4 before, and the current delta is at or below 0.5 × peak;
  - 4 or more deltas in a row at or below 0.5 × peak;
  - for notches, 3 in a row under 25 ms apart (a free-spinning wheel).
- **Unlock:** after 120 ms of silence, on a reversal, on a jump above max(1.5 × previous, previous + 4 px), or on a rising delta of at least 4 px (a steady drag).
- **The peak** is measured for each gesture. It resets after 120 ms of silence, on a reversal, on unlock, and on a sharp drop (a delta under half the previous one), so a hand that slows down is not read as momentum.
- Shift+wheel and horizontal wheels use the same rules on their own axis.

These values come from the rotary selector prototype: one real MacBook trackpad trace plus simulations, and a Logitech MX Anywhere 3S on stock macOS. Before this PR merges, recheck them against real readouts: a hard and a gentle trackpad swipe, notch-by-notch scrolling, fast spinning, and a free spin.

### Delivery

One PR:

1. The shared wheel module, with unit tests built from recorded traces, including the real plateaued trackpad tail (4, 4, 4, 3×11, 2×6 at about 8 ms).
2. Knob uses it, and `allowWheel` defaults to `true`.
3. Knob docs: the `allowWheel` default, and a sentence on how the wheel behaves.
4. e2e: a notch steps once, a dense decaying stream stops early, and an unfocused dial lets the page scroll.

## As built

Differences from the plan above.

Dial context (PR #20):

- The context is `DialProvider({ angle, onCapRadiusChange })`, not `{ angle, disabled, registerCapRadius }`. No part read `disabled`, so it went.
- `KnobCap` reports its radius before paint, and `null` when it unmounts. It calls the dial through `useEffectEvent`, so a dial that passes an inline callback does not loop.

Wheel (PR #21):

- The module is `lib/audio/wheel.ts`. `createWheelStepper()` returns a function that turns one wheel event into −1, 0 or 1. Each dial keeps its own stepper.
- The "jump" unlock rule went. A rise of 4 px or more already covers it.
- The listener must be non-passive to call `preventDefault()`, and a non-passive listener on every knob slows page scrolling. So the dial adds it only while it has focus and is enabled.
- Knob turns by `step` for each wheel step, and by `fineStep` with Shift or Alt.
- Unit tests check each limit on both sides of its edge, so a retune shows as a failing test. The e2e test sets `timeStamp` on its wheel events, so the result does not depend on the speed of the machine.
- Still open: the recheck on real devices before merge.
