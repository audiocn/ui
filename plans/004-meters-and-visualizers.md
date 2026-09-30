# 004 — Meters and visualizers

Status: PLANNED (2026-09-30). Follows plan 002; data types are in plan 003.

Eight components: `level-meter`, `db-scale`, `db-readout`, `clip-indicator`, `bar-visualizer`, `live-waveform`, `waveform`, `spectrum`.

Every component here accepts data as a `source`, as values, or through a ref handle (plan 002). All honour `prefers-reduced-motion` by showing a static level.

---

## `level-meter`

A peak and RMS level meter. The centre of the library.

Built on: `@base-ui/react/meter` for semantics. The bars are moved with transforms, outside React.

### Anatomy

```tsx
<LevelMeter source={analyser.meter} orientation="vertical">
  <LevelMeterScale />
  <LevelMeterChannel index={0}>
    <LevelMeterTrack>
      <LevelMeterBar measure="rms" />
      <LevelMeterBar measure="peak" />
      <LevelMeterHold />
    </LevelMeterTrack>
  </LevelMeterChannel>
  <LevelMeterValue />
  <LevelMeterClip />
</LevelMeter>
```

With no children, `<LevelMeter />` renders a default anatomy: one track per channel with a peak bar and a hold tick. `<LevelMeter peakDb={-12} />` is a complete meter.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `LevelMeter` | `source` | `FrameSource<MeterFrame>` | — | Subscribes and paints |
|  | `peakDb`, `rmsDb` | `number` | — | Mono declarative input |
|  | `channels` | `ChannelLevel[]` | — | Multi-channel declarative input |
|  | `channelCount` | `number` | 1 | Tracks to render before data arrives |
|  | `minDb`, `maxDb` | `number` | −60, 0 | Displayed range |
|  | `zones` | `MeterZone[]` | `DEFAULT_ZONES` | Colour thresholds |
|  | `ballistics` | `Partial<BallisticsOptions> \| "peak" \| "vu" \| "instant"` | `"peak"` | Movement |
|  | `taper` | `Taper` | linear | Scale law |
|  | `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` |  |
|  | `variant` | `"solid" \| "segmented" \| "gradient"` | `"solid"` | Segmented is an LED ladder |
|  | `segments` | `number` | 24 | For `segmented` |
|  | `size` | `"sm" \| "default" \| "lg"` | `"default"` | Track thickness |
|  | `ref` | `LevelMeterHandle` | — | `paint(frame)`, `reset()` |
| `LevelMeterChannel` | `index` | `number` | 0 | Which channel to show |
| `LevelMeterBar` | `measure` | `"peak" \| "rms"` | `"peak"` | Layer both for a dual meter |
| `LevelMeterHold` | — |  |  | Peak-hold tick |
| `LevelMeterScale` | `ticks`, `side` | see `db-scale` |  | Inherits range and taper |
| `LevelMeterValue` | `measure`, `intervalMs` | see `db-readout` |  | Numeric readout |
| `LevelMeterClip` | `thresholdDb`, `holdMs` | see `clip-indicator` |  | Clip light |

Inside a `ChannelStrip` or `Mixer`, `orientation`, `size`, `minDb`, `maxDb`, `zones` and `ballistics` are inherited from context unless set.

### Styling

- Data attributes: `data-orientation`, `data-variant`, `data-zone`, `data-clipping`, `data-active`; on each bar, `data-measure`.
- CSS variables: `--meter-thickness`, `--meter-gap` (between channels), `--meter-ok`, `--meter-warn`, `--meter-clip`, and the live `--meter-level` (0..1) on each channel.
- Zone colours stay fixed to their position on the track; the bar reveals them as the level rises.

### Accessibility

- `role="meter"`, `aria-valuemin`, `aria-valuemax`, `aria-valuenow` in dB, `aria-valuetext` such as "−12 dB". Updated at most four times a second.
- Requires a label (`aria-label` or `aria-labelledby`).

### Docs examples

Mono, stereo, vertical, segmented, gradient, peak and RMS layered, with scale, with readout and clip light, VU ballistics, custom zones, custom colours, a CSS-only custom visual built on `--meter-level`.

---

## `db-scale`

Tick marks and labels for a dB range. Used by `level-meter` and `fader`, and usable alone.

### Anatomy

```tsx
<DbScale minDb={-60} maxDb={6} orientation="vertical">
  <DbScaleTick value={0} />
  <DbScaleTick value={-12} />
</DbScale>
```

With no children it renders ticks from the `ticks` prop.

### Props

| Part | Prop | Type | Default |
| --- | --- | --- | --- |
| `DbScale` | `minDb`, `maxDb` | `number` | −60, 0 |
|  | `ticks` | `number[]` | `[0, -6, -12, -18, -24, -36, -48, -60]`, filtered to the range |
|  | `taper` | `Taper` | linear |
|  | `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` |
|  | `side` | `"start" \| "end"` | `"end"` |
|  | `labels` | `boolean` | `true` |
|  | `format` | `(db: number) => string` | digits only, no unit |
| `DbScaleTick` | `value` | `number` | — |
|  | `major` | `boolean` | `true` |

Decorative for assistive technology (`aria-hidden`), since the meter or fader it belongs to already reports the value.

---

## `db-readout`

A numeric level label that does not jitter.

```tsx
<DbReadout source={analyser.meter} />
```

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `value` | `number` | — | dB, declarative |
| `source` | `FrameSource<MeterFrame>` | — |  |
| `measure` | `"peak" \| "rms"` | `"peak"` |  |
| `channel` | `number \| "max"` | `"max"` |  |
| `intervalMs` | `number` | 250 | How often the text changes |
| `holdMs` | `number` | 0 | Show the highest value in this window |
| `decimals` | `number` | 1 |  |
| `unit` | `boolean` | `true` | Append " dB" |
| `floorDb` | `number` | −60 | At or below this, show "−∞" |
| `format` | `(db: number) => string` | — | Overrides all formatting |

- Renders a `span` with `tabular-nums` and a reserved width, so the layout never shifts.
- Data attributes: `data-zone`, `data-silent`.

---

## `clip-indicator`

A light that turns on when the signal clips, and holds.

```tsx
<ClipIndicator source={analyser.meter} />
```

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `clipping` | `boolean` | — | Controlled state |
| `source` | `FrameSource<MeterFrame>` | — | Detects clipping itself |
| `thresholdDb` | `number` | −1 |  |
| `holdMs` | `number` | 1500 | `Infinity` latches until reset |
| `onClippingChange` | `(clipping: boolean) => void` | — |  |
| `showCount` | `boolean` | `false` | Shows the number of clips |
| `render` | element | `<button>` | Click resets |
| `ref` | `ClipIndicatorHandle` | — | `report(db)`, `reset()` |

- Children replace the default dot (for example the word "Clip").
- Its width is always reserved, so turning on never moves the layout.
- Data attributes: `data-clipping`.
- Announces "Clipping" through `aria-live="polite"` once per hold.

---

## `bar-visualizer`

A row of bars driven by frequency bands. The voice-activity look.

```tsx
<BarVisualizer source={analyser.visual} barCount={24} />
```

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `source` | `FrameSource<VisualFrame>` | — |  |
| `levels` | `number[] \| Float32Array` | — | 0..1, declarative |
| `barCount` | `number` | 24 | Bands are resampled to fit |
| `align` | `"center" \| "start" \| "end"` | `"center"` | Where bars grow from |
| `mirrored` | `boolean` | `false` | Symmetric around the middle bar |
| `minLevel` | `number` | 0.08 | Resting bar size |
| `idle` | `"static" \| "pulse" \| "wave"` | `"static"` | Behaviour with no signal |
| `loading` | `boolean` | `false` | Runs a sweep animation |
| `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` |  |
| `ref` | `BarVisualizerHandle` | — | `paint(levels)` |

- DOM bars, each `data-slot="bar-visualizer-bar"`, painted without React.
- Colour is `currentColor`; set it with a text colour class.
- CSS variables: `--bar-width`, `--bar-gap`, `--bar-radius`.
- Data attributes: `data-active`, `data-loading`; on each bar, `data-index`.
- `role="img"` with a label; purely decorative uses set `aria-hidden`.

---

## `live-waveform`

A canvas waveform of a live signal.

```tsx
<LiveWaveform source={analyser.visual} mode="scrolling" className="h-16" />
```

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `source` | `FrameSource<VisualFrame>` | — |  |
| `mode` | `"scrolling" \| "static"` | `"static"` | Scrolling shows level history |
| `variant` | `"bars" \| "line" \| "mirror"` | `"bars"` | `line` draws the time-domain trace |
| `barWidth`, `barGap`, `barRadius` | `number` | 3, 1, 1.5 | Pixels |
| `minBarHeight` | `number` | 4 | Pixels |
| `lineWidth` | `number` | 1.5 | For `line` |
| `fadeEdges` | `boolean` | `true` |  |
| `fadeWidth` | `number` | 24 | Pixels |
| `active` | `boolean` | `true` | `false` shows the idle dotted line |
| `sensitivity` | `number` | 1 | Visual gain |
| `ref` | `LiveWaveformHandle` | — | `paint(frame)`, `clear()` |

- Fills its container; size it with `className`.
- CSS variables: `--waveform` (defaults to `currentColor`).
- Redraws on resize and on theme change.
- `role="img"` with a label.

---

## `waveform`

A static waveform of a clip, with a playhead, seeking, regions and markers.

Built on: a canvas for the waveform, DOM for the interactive parts.

### Anatomy

```tsx
<Waveform peaks={peaks} duration={duration} currentTime={time} onSeek={seek}>
  <WaveformCanvas />
  <WaveformCursor />
  <WaveformHover />
  <WaveformRegion start={4} end={9} onValueChange={setRegion} />
  <WaveformMarker time={12}>Drop</WaveformMarker>
</Waveform>
```

With no children it renders the canvas and the cursor.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `Waveform` | `peaks` | `Float32Array \| number[]` | — | From `use-waveform-data` |
|  | `duration` | `number` | — | Seconds |
|  | `currentTime`, `defaultCurrentTime` | `number` | — | Seconds |
|  | `time` | `FrameSource<number>` | — | Smooth playhead with no renders |
|  | `onSeek` | `(time: number) => void` | — | While dragging |
|  | `onSeekCommitted` | `(time: number) => void` | — | On release |
|  | `step`, `largeStep` | `number` | 5, 15 | Seconds, for the keyboard |
|  | `variant` | `"bars" \| "line" \| "mirror"` | `"bars"` |  |
|  | `barWidth`, `barGap`, `barRadius` | `number` | 2, 1, 1 |  |
|  | `interactive` | `boolean` | `true` | `false` is display only |
|  | `loading` | `boolean` | `false` | Skeleton state |
|  | `disabled` | `boolean` | `false` |  |
| `WaveformHover` | `format` | `(time: number) => string` | `formatTime` | Time under the pointer |
| `WaveformRegion` | `start`, `end` | `number` | — | Seconds |
|  | `onValueChange` | `({ start, end }) => void` | — | Drag or resize |
|  | `resizable`, `draggable` | `boolean` | `true` |  |
|  | `minLength` | `number` | 0.1 | Seconds |
| `WaveformMarker` | `time` | `number` | — |  |

### Styling

- CSS variables: `--waveform` (unplayed), `--waveform-progress` (played, defaults to `var(--primary)`), `--waveform-cursor`.
- Data attributes: `data-loading`, `data-dragging`, `data-disabled`.

### Accessibility

- When interactive, the root is a slider: `aria-valuenow` in seconds, `aria-valuetext` such as "1:24 of 3:40".
- Keyboard: Left/Right seek by `step`, Shift for `largeStep`, Home and End.
- Region handles are separate sliders with their own labels.

---

## `spectrum`

A frequency spectrum analyser. Phase 6.

```tsx
<Spectrum source={analyser.visual} className="h-40">
  <SpectrumCanvas />
  <SpectrumFrequencyAxis />
  <SpectrumLevelAxis />
</Spectrum>
```

| Part | Prop | Type | Default |
| --- | --- | --- | --- |
| `Spectrum` | `source` | `FrameSource<VisualFrame>` | — |
|  | `variant` | `"bars" \| "line" \| "area"` | `"bars"` |
|  | `minDb`, `maxDb` | `number` | −90, 0 |
|  | `minHz`, `maxHz` | `number` | 20, 20000 |
|  | `scale` | `"log" \| "linear"` | `"log"` |
|  | `peakHold` | `boolean` | `false` |
|  | `grid` | `boolean` | `true` |
| `SpectrumFrequencyAxis` | `ticks` | `number[]` | `[100, 1000, 10000]` |
| `SpectrumLevelAxis` | `ticks` | `number[]` | every 12 dB |

CSS variables: `--spectrum`, `--spectrum-peak`, `--spectrum-grid`.

---

## Registry items

| Item | npm dependencies | Registry dependencies |
| --- | --- | --- |
| `level-meter` | `@base-ui/react`, `class-variance-authority` | `@audiocn/core`, `@audiocn/use-frame-source`, `@audiocn/db-scale`, `@audiocn/db-readout`, `@audiocn/clip-indicator` |
| `db-scale` | — | `@audiocn/core` |
| `db-readout` | — | `@audiocn/core`, `@audiocn/use-frame-source` |
| `clip-indicator` | `@base-ui/react` | `@audiocn/core`, `@audiocn/use-clip-hold`, `@audiocn/use-frame-source` |
| `bar-visualizer` | — | `@audiocn/core`, `@audiocn/use-frame-source` |
| `live-waveform` | — | `@audiocn/core`, `@audiocn/use-frame-source` |
| `waveform` | `@base-ui/react` | `@audiocn/core`, `@audiocn/use-frame-source` |
| `spectrum` | — | `@audiocn/core`, `@audiocn/use-frame-source` |

## Tests

- Each component renders from all three inputs (source, values, ref) and shows the same result.
- Meter ARIA values are correct and throttled.
- Zone and clip attributes change at the right thresholds.
- Reduced motion produces a static render.
- Canvas components redraw on resize and unsubscribe on unmount.
- `waveform`: keyboard seeking, region drag and resize limits.
