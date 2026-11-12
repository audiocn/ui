# Porting audiocn to GPUI — rules for every contributor (human or agent)

The goal is an **exact** GPUI copy of audiocn: same components, same props, same
defaults, same visuals (sizes, colours, radii, spacing, fonts), same behaviour
(drag, keyboard, ballistics, animation timing). The React source in the repo root
is the spec. When in doubt, read the `.tsx`, its tests (`*.test.tsx`) and its docs
page (`content/docs/**/<name>.mdx`), and copy what it does.

The house style follows the Evil Buttons GPUI port
(`~/.cargo/git/checkouts/evil-buttons-*/*/gpui/crates/evilbuttons/src`, e.g. `hold.rs`,
`motion.rs`, `surface.rs`): read one or two of those files before you start.

## Layout

```
gpui/crates/audiocn/src/
  lib.rs            crate root: re-exports components, blocks, theme, types
  theme.rs          shadcn + audiocn CSS variables as a GPUI global (DONE)
  config.rs         AudioConfig (use-audio-config) (DONE)
  motion.rs         Easing / Transition / Spring / Timeline (DONE, from Evil Buttons)
  audio/            lib/audio/*.ts + lib/electric.ts     (pure maths, unit-tested)
  signal/           hooks/* that are pure state (demo signal, clip hold, tap tempo, mixer, level)
  ui/               shadcn primitives (button, select, popover, switch, tabs, ...)
  components/       components/ui/<audio component>.tsx, one module each
  blocks/           components/blocks/<block>/, one module each
  io/               native audio I/O behind the `device` feature (cpal), not wasm
gpui/crates/gallery/src/demos/<slug>.rs   one docs page per file, one variant per example
```

`mod.rs` files already declare every module (`mod x; pub use x::*;`). **Only edit
the files you own.** If you truly need something in a file you don't own, write it
in your own file, or note it in your final report.

## Naming (so modules can be written in parallel against each other)

Mechanical, no exceptions:

- TS file `lib/audio/bar-levels.ts` → `crate::audio::bar_levels`. Hook
  `hooks/use-clip-hold.ts` → `crate::signal::clip_hold`.
- Exported functions keep their name in snake_case: `dbToGain` → `db_to_gain`,
  `createBallistics` → `create_ballistics`. Constants: `DEFAULT_MIN_DB` stays.
  Types/interfaces keep PascalCase: `BallisticsInput`, `MeterZone`.
- Numbers are `f32` (dB, positions, levels, pixels) unless the TS clearly needs
  `f64` (timestamps in ms → `f64`). `-Infinity` → `f32::NEG_INFINITY`.
- TS options objects / optional params → a struct with `Default` (and builder-ish
  `..Default::default()` usage), or `Option<T>` params when there are one or two.
- String unions → enums (`"horizontal" | "vertical"` → `Orientation`).
- Components: `<LevelMeter>` → `LevelMeter`. Sub-parts keep their names:
  `<ChannelStripHeader>` → `ChannelStripHeader`. Props → builder methods with the
  same snake_case name (`peakHold` → `.peak_hold(bool)`), defaults identical.
  Callbacks: `onValueChange` → `.on_value_change(impl Fn(f32, &mut Window, &mut App) + 'static)`.
  `className` / `style` → the builder implements `gpui::Styled` where it makes
  sense (forward a `StyleRefinement` onto the root div) so users can tweak layout.

## Components

- A component is a `#[derive(IntoElement)] pub struct X { id: ElementId, .. }` with
  `impl RenderOnce`. Interactive or animated state lives in
  `window.use_keyed_state(id, cx, |_, cx| State { .. })` (an `Entity<State>`),
  exactly like Evil Buttons. Give each instance a unique id.
- **Controlled and uncontrolled**, like React: `.value(v)` makes it controlled
  (state follows the prop each render, changes are only reported through
  `on_value_change`); without `.value`, `.default_value(v)` seeds internal state.
- **Theme**: `use crate::theme::ActiveTheme; cx.theme().muted_foreground` etc.
  Tailwind → GPUI: `size-4` = 16px, `gap-2` = 8px (Tailwind unit = 4px),
  `text-xs` 12px/16px, `text-sm` 14px/20px, `rounded-md` = `theme.radius_md()`,
  `ring-1 ring-foreground/10` = 1px border of `foreground.opacity(0.1)`.
  `color-mix(in oklab, A x%, B)` → `crate::theme::mix(B, A, x/100)`;
  `bg-foo/40` → `.opacity(0.4)` on the colour. `font-mono` → `theme.font_mono`,
  `tabular-nums` → use the mono font or fixed-width layout.
- **Painting**: meters, knobs, waveforms, visualizers draw in `canvas(prepaint,
  paint)` with `window.paint_quad(fill(...))`, `PathBuilder` + `window.paint_path`,
  gradients via `linear_gradient`. SVG in React → paths here.
- **Animation**: anything that moves calls `window.request_animation_frame()` during
  render/paint *while* it is moving (meters: while a `FrameSource` is attached and
  not idle, or while ballistics are still decaying). Time comes from
  `crate::motion::Instant` (`web_time`), never `std::time::Instant` (wasm).
- **Frame sources**: React components take `source?: FrameSource<MeterFrame>` or a
  static `value/level` prop. Here: `.source(FrameSource<MeterFrame>)` /
  `.source(FrameSource<VisualFrame>)` and the same static props. Read with
  `source.latest()` / `source.with_latest(..)` in paint.
- **AudioConfig**: React reads `useAudioConfig()` from context. Here every component
  that does has `.config(AudioConfig)`; resolve `prop → config → default`.
  Containers (ChannelStrip, Mixer, blocks) pass config down explicitly.
- **Keyboard & focus**: same keys as React (arrows, PageUp/Down, Home/End, shift for
  fine steps, double-click / alt-click reset, Enter to edit value...). Use a
  `FocusHandle` stored in state with `.track_focus(&focus)` and `on_key_down`. Paint
  the focus ring like `focus-visible:ring-3 ring-ring/50` when
  `window.last_input_was_keyboard()` and focused.
- **Reduced motion**: `crate::signal::reduced_motion` (a global flag the app can set).
- Doc comment on every public type: one line describing it, like the docs page
  description, plus a short usage example (`ignore` code block).

## Gallery demos

`gpui/crates/gallery/src/demos/<slug>.rs` already lists the variants (one per file
in `components/examples/<slug>-*.tsx`; `-demo` = `default`). Port each example
*exactly*: same data, labels, sizes, layout. Replace `pending(..)` with the real
element. Live data in examples comes from `useDemoSignal` → use
`audiocn::signal::demo_signal` (a `DemoSignal` you drive from a keyed state entity
on animation frames). Microphone examples: render the same UI fed by the demo
signal with the "microphone" affordance (the gallery has no mic access by default).

## Verification (required before you finish)

From `gpui/`:

```sh
cargo check -p audiocn -p audiocn-gallery     # whole workspace must compile
cargo test -p audiocn <your module>           # port the TS unit tests for your files
cargo clippy -p audiocn -- -D warnings        # at the end, for your files
cargo fmt --all
```

Other agents edit other files at the same time. If the build fails **only** in
files you don't own, ignore those errors (filter `cargo check` output by your
paths) and keep going; never "fix" someone else's file. Don't run `cargo clean`.
Never commit or push; the coordinator does that.

Screenshots: `scripts/shot.sh` is being fixed (GPUI windows from agent shells
currently don't appear). Until it works, verify by reading the React source
carefully, not by eye.
