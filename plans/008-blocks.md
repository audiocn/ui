# 008 — Blocks

Status: PLANNED (2026-09-30). Follows plan 002; composes plans 003–007.

Blocks are complete, working assemblies, like shadcn's blocks. A block is code the user owns after installing: it has few props, and is customised by editing it. Each block wires `ui` components to the Web Audio hooks and installs everything it needs in one command.

Blocks may import Phosphor icons and shadcn components freely.

Six blocks: `system-audio-mixer`, `mic-setup`, `system-audio-settings`, `quick-audio-popover`, `soundboard`, `music-player`.

---

## `system-audio-mixer`

The complete mixer. The flagship block.

### What it contains

- A `Mixer` with one `ChannelStrip` per source:
  - **Microphone:** device name, level meter, fader, mute, solo.
  - **System audio:** on/off switch, level meter, fader, mute. Shows a notice when the browser cannot capture system audio.
  - **Music:** level meter, fader, mute, solo, and the current track title.
  - **Sounds:** level meter, fader, mute.
- A master strip: stereo meter, master fader, clip indicator with count.
- A header with a layout switch (rows or console) and a reset action.

### Hooks used

`use-mixer`, `use-web-audio-mixer`, `use-microphone`, `use-system-audio`, `use-audio-devices`, `use-audio-analyser`.

### Props

| Prop | Type | Default | Notes |
| --- | --- | --- | --- |
| `defaultOrientation` | `"horizontal" \| "vertical"` | `"horizontal"` |  |
| `sources` | `("microphone" \| "system" \| "music" \| "sounds")[]` | all four | Which strips to show |
| `persistKey` | `string` | — | Saves mixer state to `localStorage` |
| `onOutputChange` | `(stream: MediaStream \| null) => void` | — | The mixed output, for recording or streaming |

### States it handles

Microphone permission prompt, permission denied, no devices, device disconnected, system audio unsupported, system audio ended by the user, audio context suspended until the first click.

### Files

`system-audio-mixer.tsx` (the mixer), `mixer-source-strip.tsx` (one strip), `use-mixer-sources.ts` (wires the hooks together).

---

## `mic-setup`

Choose and check a microphone.

- `AudioDeviceSelect` with a `LiveWaveform` preview.
- A `LevelMeter` with a `DbReadout` and `ClipIndicator`.
- A gain `ParameterSlider` (−24..+24 dB).
- A mute switch.
- A "Check level" action that listens for a few seconds and reports "Good", "Too quiet", "Too loud" or "No signal".
- Permission prompt and denied states.

| Prop | Type | Notes |
| --- | --- | --- |
| `deviceId`, `onDeviceChange` | `string \| null`, callback | Optional control of the selection |
| `gainDb`, `onGainChange` | `number`, callback |  |
| `muted`, `onMutedChange` | `boolean`, callback |  |

Hooks: `use-audio-devices`, `use-microphone`, `use-audio-analyser`.

---

## `system-audio-settings`

Turn system audio capture on and set its level.

- An on/off switch with a state label (Off, On, Needs permission, Unsupported, Ended).
- A level `ParameterSlider` (−24..+12 dB, default −6).
- A `LevelMeter` shown while capture is on.
- A notice explaining what the browser will capture, and a fallback message where capture is unsupported.

| Prop                         | Type                | Notes |
| ---------------------------- | ------------------- | ----- |
| `enabled`, `onEnabledChange` | `boolean`, callback |       |
| `gainDb`, `onGainChange`     | `number`, callback  |       |

Hooks: `use-system-audio`, `use-audio-analyser`.

---

## `quick-audio-popover`

Compact audio controls behind one button, for a toolbar.

- Trigger: a button showing a small `BarVisualizer` of the microphone, which changes to a muted state when muted.
- Content: microphone select, mute button, system audio switch, and a link slot for "More audio settings".

| Prop | Type | Notes |
| --- | --- | --- |
| `children` | `ReactNode` | Extra content at the bottom of the popover |
| `side`, `align` | popover placement |  |

Hooks: `use-audio-devices`, `use-microphone`, `use-audio-analyser`, `use-system-audio`. Uses shadcn `popover`.

---

## `soundboard`

A grid of sound pads.

- `SoundPadGrid` with hotkeys.
- A header with a master `VolumeControl`, a "Stop all" button and a hotkeys on/off switch.
- Each pad: label, hotkey, progress, and a context menu with per-pad volume, mode and remove.
- An "Add sound" pad that accepts a file, and drag-and-drop of audio files onto the grid.
- Empty state.

| Prop | Type | Notes |
| --- | --- | --- |
| `sounds`, `defaultSounds` | `{ id, label, src, hotkey?, mode?, volume? }[]` |  |
| `onSoundsChange` | callback |  |
| `output` | `AudioNode` | Route into a mixer instead of the speakers |

Hooks: `use-sound`, `use-audio-context`. Uses shadcn `context-menu`.

---

## `music-player`

A playlist player with ducking.

- `AudioPlayer` with artwork, title, transport, a `Waveform` seek bar, time and volume.
- A `TrackList` below it; selecting a track plays it; next and previous follow the list; the list advances automatically.
- Shuffle and repeat toggles.
- A ducking section: an on/off switch and an amount `ParameterSlider`, which lowers the music while the microphone is active.
- Empty state.

| Prop | Type | Notes |
| --- | --- | --- |
| `tracks`, `defaultTracks` | `{ id, title, artist?, src, artwork?, duration? }[]` |  |
| `onTrackChange` | `(track) => void` |  |
| `duckingSource` | `FrameSource<MeterFrame>` | The signal that triggers ducking |
| `output` | `AudioNode` | Route into a mixer |

Hooks: `use-audio-player`, `use-waveform-data`, `use-audio-analyser`.

---

## Registry items

| Block | Registry dependencies |
| --- | --- |
| `system-audio-mixer` | `button`, `switch`, `tooltip`, `@audiocn/mixer`, `@audiocn/channel-strip`, `@audiocn/level-meter`, `@audiocn/fader`, `@audiocn/channel-toggle`, `@audiocn/db-readout`, `@audiocn/clip-indicator`, and the hooks listed above |
| `mic-setup` | `button`, `switch`, `field`, `@audiocn/audio-device-select`, `@audiocn/live-waveform`, `@audiocn/level-meter`, `@audiocn/parameter-slider`, and its hooks |
| `system-audio-settings` | `switch`, `field`, `alert`, `@audiocn/level-meter`, `@audiocn/parameter-slider`, and its hooks |
| `quick-audio-popover` | `button`, `popover`, `switch`, `@audiocn/audio-device-select`, `@audiocn/bar-visualizer`, and its hooks |
| `soundboard` | `button`, `switch`, `context-menu`, `empty`, `@audiocn/sound-pad`, `@audiocn/volume-control`, and its hooks |
| `music-player` | `button`, `switch`, `empty`, `@audiocn/audio-player`, `@audiocn/track-list`, `@audiocn/waveform`, `@audiocn/parameter-slider`, and its hooks |

## Docs

- Each block has a full-width live preview, the install command, and a file tree with the code.
- A "Blocks" index page shows all six.
- Previews run on bundled demo audio and `use-demo-signal`, with an opt-in switch to use the real microphone.

## Tests

- Each block renders its empty, loading, permission and active states against mocked hooks.
- `system-audio-mixer`: mute, solo and master changes reach the graph; unsupported system audio shows the notice instead of failing.
- `soundboard`: adding and removing sounds, hotkeys, stop all.
- `music-player`: track advance, shuffle, repeat, ducking on and off.

---

## Later candidates (not specified yet)

Components that fit the library and would each get a spec when scheduled:

- `loudness-meter`: LUFS (momentary, short-term, integrated) with a target line.
- `gain-reduction-meter`: how much a compressor or limiter is reducing.
- `correlation-meter`: stereo phase correlation.
- `oscilloscope` and `spectrogram`.
- Processing panels on Web Audio nodes: noise gate, compressor, limiter, EQ.
- `audio-recorder`: outside the current non-goals; needs a decision first.
