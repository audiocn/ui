# 005 — Controls

Status: PLANNED (2026-09-30). Follows plan 002.

Seven components: `fader`, `parameter-slider`, `knob`, `pan-control`, `channel-toggle`, `volume-control`, `audio-device-select`.

All are controlled or uncontrolled, keyboard operable, and have no audio code inside: they report values and the consumer applies them.

---

## `fader`

A volume fader in decibels.

Built on: `@base-ui/react/slider`. The public value is always dB; the taper maps it to thumb position.

### Anatomy

```tsx
<Fader value={db} onValueChange={setDb} min={-60} max={6}>
  <FaderLabel>Microphone</FaderLabel>
  <FaderScale />
  <FaderTrack>
    <FaderRange />
    <FaderThumb />
  </FaderTrack>
  <FaderValue />
</Fader>
```

With no children, `<Fader />` renders a track, range and thumb.

`FaderTrack` accepts any children, so a `LevelMeter` can sit inside the track to make a combined fader and meter.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `Fader` | `value`, `defaultValue` | `number` | 0 | dB |
|  | `onValueChange`, `onValueCommitted` | `(db, details) => void` | — |  |
|  | `min`, `max` | `number` | −60, 6 | dB |
|  | `step` | `number` | 0.5 | Arrow keys and drag resolution |
|  | `largeStep` | `number` | 6 | Shift+arrow, Page Up/Down |
|  | `fineStep` | `number` | 0.1 | Alt while dragging or pressing arrows |
|  | `resetValue` | `number` | 0 | Double-click the thumb |
|  | `taper` | `"linear" \| "audio" \| Taper` | `"linear"` | Position law |
|  | `origin` | `number` | `min` | Where the range fill starts; set 0 for a bipolar gain |
|  | `detents` | `number[]` | `[0]` | Values the thumb snaps to |
|  | `silenceAtMin` | `boolean` | `false` | The minimum position reports `-Infinity` |
|  | `allowWheel` | `boolean` | `false` | Wheel adjusts while focused |
|  | `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` |  |
|  | `variant` | `"default" \| "console"` | `"default"` | `console` has a wide cap thumb |
|  | `size` | `"sm" \| "default" \| "lg"` | `"default"` |  |
|  | `format` | `(db: number) => string` | `formatDb` | Used for ARIA and `FaderValue` |
|  | `disabled` | `boolean` | `false` |  |
| `FaderScale` | `ticks`, `side` | see `db-scale` |  | Inherits range and taper |
| `FaderValue` | `editable` | `boolean` | `false` | Becomes a number input on click |
| `FaderReset` | `render` | element | `<button>` | Shown when value differs from `resetValue` |

Inside a `ChannelStrip`, `orientation`, `size` and `disabled` are inherited.

### Styling

- Data attributes: `data-orientation`, `data-dragging`, `data-disabled`, `data-at-detent`, `data-silent`, `data-variant`.
- CSS variables: `--fader-track-size`, `--fader-thumb-size`.

### Accessibility

- `role="slider"`, `aria-valuetext` such as "−6 dB" or "Silent".
- Keyboard: arrows by `step`, Shift+arrows and Page Up/Down by `largeStep`, Alt for `fineStep`, Home and End for the limits.
- Double-click reset also has a keyboard path through `FaderReset`.

### Docs examples

Horizontal, vertical, console variant, with scale, with editable value, bipolar gain (−24..+24, origin 0), audio taper, silence at minimum, with a meter in the track, sizes, disabled.

---

## `parameter-slider`

A labelled slider with a numeric input and a unit. For everything that is not a volume: gain trim, sync delay, thresholds, times, frequencies.

Built on: `@base-ui/react/slider` and `@base-ui/react/number-field`.

### Anatomy

```tsx
<ParameterSlider
  value={ms}
  onValueChange={setMs}
  min={-1000}
  max={1000}
  unit="ms"
>
  <ParameterSliderHeader>
    <ParameterSliderLabel>Sync offset</ParameterSliderLabel>
    <ParameterSliderInput />
    <ParameterSliderReset />
  </ParameterSliderHeader>
  <ParameterSliderControl />
  <ParameterSliderMarks />
  <ParameterSliderDescription>
    Delays the microphone.
  </ParameterSliderDescription>
</ParameterSlider>
```

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `ParameterSlider` | `value`, `defaultValue` | `number` | — |  |
|  | `onValueChange`, `onValueCommitted` | `(value, details) => void` | — |  |
|  | `min`, `max`, `step`, `largeStep` | `number` | 0, 100, 1, 10 |  |
|  | `unit` | `string` | — | Suffix such as "ms", "dB", "Hz" |
|  | `decimals` | `number` | from `step` |  |
|  | `scale` | `"linear" \| "log"` | `"linear"` | `log` for frequency and time |
|  | `origin` | `number` | `min` | Bipolar fill start |
|  | `resetValue` | `number` | `defaultValue` |  |
|  | `marks` | `{ value: number; label?: string }[]` | — |  |
|  | `format` | `(value: number) => string` | — |  |
|  | `disabled` | `boolean` | `false` |  |
| `ParameterSliderInput` | `scrub` | `boolean` | `true` | Drag the number to change it |

- The label is wired to both the slider and the input.
- Data attributes: `data-disabled`, `data-dragging`, `data-modified` (value differs from `resetValue`).

---

## `knob`

A rotary control for dense layouts.

Built on: a custom slider-role element; there is no Base UI primitive for it. Drawn in SVG.

### Anatomy

```tsx
<Knob value={pan} onValueChange={setPan} min={-1} max={1}>
  <KnobTrack />
  <KnobRange />
  <KnobPointer />
  <KnobValue />
  <KnobLabel>Pan</KnobLabel>
</Knob>
```

### Props

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `value`, `defaultValue` | `number` | — |  |
| `onValueChange`, `onValueCommitted` | `(value, details) => void` | — |  |
| `min`, `max`, `step`, `largeStep`, `fineStep` | `number` | 0, 100, 1, 10, 0.1 |  |
| `resetValue` | `number` | `defaultValue` | Double-click |
| `origin` | `number` | `min` | Arc start; the centre for bipolar knobs |
| `arc` | `number` | 270 | Sweep in degrees |
| `dragDirection` | `"vertical" \| "horizontal" \| "circular"` | `"vertical"` |  |
| `sensitivity` | `number` | 200 | Pixels of drag for the full range |
| `scale` | `"linear" \| "log"` | `"linear"` |  |
| `allowWheel` | `boolean` | `false` |  |
| `format` | `(value: number) => string` | — |  |
| `size` | `"sm" \| "default" \| "lg"` | `"default"` |  |
| `disabled` | `boolean` | `false` |  |

- CSS variables: `--knob-angle` (live, for custom skins), `--knob-size`.
- Data attributes: `data-dragging`, `data-disabled`, `data-at-origin`.
- Keyboard: the same keys as `fader`.

---

## `pan-control`

Left and right balance. A preconfigured slider with pan semantics.

```tsx
<PanControl value={pan} onValueChange={setPan} />
```

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `value`, `defaultValue` | `number` | 0 | −1 (left) to 1 (right) |
| `onValueChange`, `onValueCommitted` | `(value, details) => void` | — |  |
| `step` | `number` | 0.05 |  |
| `detent` | `boolean` | `true` | Snaps to centre |
| `format` | `(value: number) => string` | "L30", "C", "R30" |  |
| `size`, `disabled` |  |  |  |

- The fill grows from the centre.
- Double-click returns to centre.
- The docs show the same control built as a `Knob`.

---

## `channel-toggle`

Mute, solo and monitor buttons.

Built on: `@base-ui/react/toggle`.

```tsx
<MuteToggle pressed={muted} onPressedChange={setMuted}>M</MuteToggle>
<SoloToggle pressed={solo} onPressedChange={setSolo}>S</SoloToggle>
<MonitorToggle pressed={monitor} onPressedChange={setMonitor} />
```

Exports: `ChannelToggle`, `MuteToggle`, `SoloToggle`, `MonitorToggle`, `channelToggleVariants`.

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `pressed`, `defaultPressed` | `boolean` | `false` |  |
| `onPressedChange` | `(pressed: boolean) => void` | — |  |
| `tone` | `"mute" \| "solo" \| "monitor" \| "neutral"` | per export | Colour when pressed |
| `variant` | `"default" \| "outline" \| "ghost"` | `"default"` |  |
| `size` | `"sm" \| "default" \| "lg" \| "icon"` | `"default"` |  |
| `disabled` | `boolean` | `false` |  |

- Children are a letter or an icon; nothing is rendered by default.
- Default `aria-label`s are "Mute", "Solo" and "Monitor".
- Pressed colours come from `--channel-mute`, `--channel-solo`, `--channel-monitor`.
- Data attributes: `data-pressed`, `data-tone`.

---

## `volume-control`

A simple volume slider with a mute button, for players. Uses 0..1, not dB.

### Anatomy

```tsx
<VolumeControl
  value={volume}
  onValueChange={setVolume}
  muted={muted}
  onMutedChange={setMuted}
>
  <VolumeControlMute />
  <VolumeControlSlider />
  <VolumeControlValue />
</VolumeControl>
```

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `VolumeControl` | `value`, `defaultValue` | `number` | 1 | 0..1 |
|  | `onValueChange`, `onValueCommitted` | `(value, details) => void` | — |  |
|  | `muted`, `defaultMuted` | `boolean` | `false` |  |
|  | `onMutedChange` | `(muted: boolean) => void` | — |  |
|  | `step` | `number` | 0.05 |  |
|  | `curve` | `"linear" \| "perceptual"` | `"perceptual"` | How position maps to gain |
|  | `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` |  |
|  | `size`, `disabled` |  |  |  |
| `VolumeControlMute` | `render` | element | `<button>` | Children are the icon |

- `VolumeControlMute` exposes `data-level="muted" | "low" | "medium" | "high"`, so the user swaps icons with CSS or a render function.
- Unmuting at zero restores the last non-zero volume.
- The docs show a popover variant (slider opens from the mute button).

---

## `audio-device-select`

A microphone, speaker or source picker.

Built on: shadcn `select`. Devices come in as props, so it works with `use-audio-devices` or any other device list.

### Anatomy

```tsx
<AudioDeviceSelect
  devices={devices}
  value={deviceId}
  onValueChange={setDeviceId}
>
  <AudioDeviceSelectTrigger>
    <AudioDeviceSelectValue placeholder="Select a microphone" />
  </AudioDeviceSelectTrigger>
  <AudioDeviceSelectContent />
  <AudioDeviceSelectPreview>
    <LiveWaveform source={analyser.visual} />
  </AudioDeviceSelectPreview>
</AudioDeviceSelect>
```

`AudioDeviceSelectContent` with no children renders one item per device.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `AudioDeviceSelect` | `devices` | `AudioDevice[]` | — | `{ id, label, isDefault?, status?, description? }` |
|  | `value`, `defaultValue` | `string \| null` | — | Device id |
|  | `onValueChange` | `(id: string \| null) => void` | — |  |
|  | `allowNone` | `boolean` | `false` | Adds a "None" item |
|  | `noneLabel` | `string` | "None" |  |
|  | `loading` | `boolean` | `false` | "Finding devices…" |
|  | `permission` | `"granted" \| "prompt" \| "denied"` | `"granted"` |  |
|  | `onRequestPermission` | `() => void` | — |  |
|  | `disabled` | `boolean` | `false` |  |
| `AudioDeviceSelectItem` | `value`, `disabled` |  |  | For custom item rendering |
| `AudioDeviceSelectPermission` | — |  |  | Shown in place of the list when permission is needed |

Device `status` is `"available" | "unavailable" | "permission-required"`.

Behaviour:

- A selected device that is no longer in the list is kept and shown as "disconnected", disabled, so the selection is never silently lost.
- The default device is marked.
- Data attributes: `data-loading`, `data-permission`, `data-missing`.

---

## Registry items

| Item | npm dependencies | Registry dependencies |
| --- | --- | --- |
| `fader` | `@base-ui/react`, `class-variance-authority` | `@audiocn/core`, `@audiocn/db-scale` |
| `parameter-slider` | `@base-ui/react` | `@audiocn/core` |
| `knob` | `@base-ui/react`, `class-variance-authority` | `@audiocn/core` |
| `pan-control` | `@base-ui/react` | `@audiocn/core` |
| `channel-toggle` | `@base-ui/react`, `class-variance-authority` | `@audiocn/core` |
| `volume-control` | `@base-ui/react` | `@audiocn/core` |
| `audio-device-select` | — | `select`, `@audiocn/core` |

## Tests

- Every control: controlled and uncontrolled modes, `onValueChange` versus `onValueCommitted`, the full keyboard table, disabled state.
- `fader`: taper round-trips, detent snapping, double-click reset, `-Infinity` at minimum, ARIA text.
- `knob`: all three drag directions, wheel, bounds.
- `audio-device-select`: missing device, permission states, "None".
