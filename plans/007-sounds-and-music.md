# 007 — Sounds and music

Status: PLANNED (2026-09-30). Follows plan 002; hooks are in plan 003.

Three components: `audio-player`, `track-list`, `sound-pad`. The `waveform` component they often pair with is in plan 004.

---

## `audio-player`

A composable audio player. The root owns the playback state; the parts are buttons, sliders and labels bound to it.

Built on: an `<audio>` element through `use-audio-player`, Base UI `slider` for seeking, and `volume-control` for volume.

### Anatomy

```tsx
<AudioPlayer src="/track.mp3">
  <AudioPlayerArtwork src="/cover.jpg" alt="" />
  <AudioPlayerTitle>Night Drive</AudioPlayerTitle>
  <AudioPlayerDescription>Orc Beats</AudioPlayerDescription>
  <AudioPlayerControls>
    <AudioPlayerPrevious />
    <AudioPlayerSkipBack seconds={10} />
    <AudioPlayerPlay />
    <AudioPlayerSkipForward seconds={10} />
    <AudioPlayerNext />
  </AudioPlayerControls>
  <AudioPlayerTime type="current" />
  <AudioPlayerSeek />
  <AudioPlayerTime type="remaining" />
  <AudioPlayerVolume />
  <AudioPlayerRate />
  <AudioPlayerLoop />
</AudioPlayer>
```

Every part is optional. A play button and a seek bar is a complete player.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `AudioPlayer` | `src` | `string` | — |  |
|  | `player` | result of `useAudioPlayer` | — | Use an external player instead of an internal one |
|  | `autoPlay`, `loop` | `boolean` | `false` |  |
|  | `preload` | `"none" \| "metadata" \| "auto"` | `"metadata"` |  |
|  | `volume`, `defaultVolume` | `number` | 1 | 0..1 |
|  | `muted`, `defaultMuted` | `boolean` | `false` |  |
|  | `playbackRate`, `defaultPlaybackRate` | `number` | 1 |  |
|  | `onPlay`, `onPause`, `onEnded` | `() => void` | — |  |
|  | `onTimeUpdate` | `(time: number) => void` | — |  |
|  | `onVolumeChange`, `onMutedChange`, `onPlaybackRateChange` | callbacks | — |  |
|  | `onPrevious`, `onNext` | `() => void` | — | Enables the previous and next buttons |
|  | `onError` | `(error: MediaError) => void` | — |  |
|  | `shortcuts` | `boolean` | `true` | Keyboard shortcuts while focus is inside |
|  | `crossOrigin` | `string` | — | Needed to analyse remote audio |
| `AudioPlayerPlay` | `render` | element | `<button>` | Children can be a function of `{ playing, loading }` |
| `AudioPlayerSkipBack`, `AudioPlayerSkipForward` | `seconds` | `number` | 10 |  |
| `AudioPlayerSeek` | `step`, `largeStep` | `number` | 5, 15 | Seconds |
| `AudioPlayerTime` | `type` | `"current" \| "remaining" \| "duration"` | `"current"` |  |
|  | `format` | `(seconds: number) => string` | `formatTime` |  |
| `AudioPlayerVolume` | `orientation` |  |  | A bound `VolumeControl` |
| `AudioPlayerRate` | `rates` | `number[]` | `[0.5, 0.75, 1, 1.25, 1.5, 2]` | Cycles on click |
| `AudioPlayerLoop` | — |  |  | A bound toggle |

### Behaviour

- `useAudioPlayerContext()` is exported, so any component can bind to the player. The documented recipe for a waveform seek bar is a `Waveform` reading `time`, `duration` and `seek` from that context.
- The seek bar and playhead move every frame without React renders; the time text updates about four times a second.
- The seek bar shows the buffered range.
- Buttons render no icon by default when children are given; with no children they show a text label, so the component has no icon dependency.
- Exposes `player.ref`, so the audio can be routed into `use-audio-analyser` or `use-web-audio-mixer`.

### Keyboard (while focus is inside, when `shortcuts` is on)

| Key                      | Action                   |
| ------------------------ | ------------------------ |
| Space or K               | Play or pause            |
| Left / Right             | Seek back or forward 5 s |
| Shift+Left / Shift+Right | Seek 15 s                |
| Up / Down                | Volume up or down        |
| M                        | Mute                     |
| Home / End               | Start or end             |

### Styling

- Data attributes on the root: `data-playing`, `data-paused`, `data-loading`, `data-ended`, `data-error`, `data-muted`.
- The seek bar uses the same slider tokens as `fader`.

### Accessibility

- `role="group"` labelled by `AudioPlayerTitle`.
- The play button's label switches between "Play" and "Pause".
- The seek slider reports `aria-valuetext` such as "1:24 of 3:40".

### Docs examples

Minimal player, full player, compact one-line player, player with a waveform seek bar, player with a live visualizer, player with artwork, playback rate, external `useAudioPlayer`, error state.

---

## `track-list`

A list of tracks with the current one marked.

### Anatomy

```tsx
<TrackList>
  <TrackListItem active playing onSelect={play}>
    <TrackListItemIndex>1</TrackListItemIndex>
    <TrackListItemArtwork src="/cover.jpg" alt="" />
    <TrackListItemContent>
      <TrackListItemTitle>Night Drive</TrackListItemTitle>
      <TrackListItemDescription>Orc Beats</TrackListItemDescription>
    </TrackListItemContent>
    <TrackListItemDuration>3:40</TrackListItemDuration>
    <TrackListItemActions>
      <Button size="icon" variant="ghost" />
    </TrackListItemActions>
  </TrackListItem>
</TrackList>
```

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `TrackList` | `size` | `"sm" \| "default" \| "lg"` | `"default"` |  |
|  | `variant` | `"default" \| "outline"` | `"default"` |  |
| `TrackListItem` | `active` | `boolean` | `false` | The current track |
|  | `playing` | `boolean` | `false` | Current and playing |
|  | `disabled` | `boolean` | `false` |  |
|  | `onSelect` | `() => void` | — | Click, Enter or Space |
|  | `render` | element | `<li>` |  |
| `TrackListItemIndex` | — |  |  | Shows its children, or an animated playing indicator when the item is playing |

- Keyboard: Up and Down move between items, Enter or Space selects, Home and End jump.
- Actions inside an item stay individually focusable.
- Data attributes: `data-active`, `data-playing`, `data-disabled`.
- Reordering is a documented recipe with a drag-and-drop library, not built in.

---

## `sound-pad`

A button that triggers a sound, with a hotkey and playback progress.

Built on: a `<button>`. The pad plays nothing itself; it reports triggers, and `use-sound` does the playback.

### Anatomy

```tsx
<SoundPadGrid columns={4} hotkeys>
  <SoundPad
    hotkey="1"
    playing={airhorn.isPlaying}
    onTrigger={airhorn.play}
    onStop={airhorn.stop}
  >
    <SoundPadIcon>
      <MegaphoneIcon />
    </SoundPadIcon>
    <SoundPadLabel>Airhorn</SoundPadLabel>
    <SoundPadShortcut />
    <SoundPadProgress source={airhorn.progress} />
  </SoundPad>
</SoundPadGrid>
```

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `SoundPad` | `onTrigger` | `() => void` | — |  |
|  | `onStop` | `() => void` | — |  |
|  | `playing` | `boolean` | `false` |  |
|  | `mode` | `"one-shot" \| "toggle" \| "hold" \| "loop"` | `"one-shot"` | How presses map to trigger and stop |
|  | `hotkey` | `string` | — | Active when the grid enables hotkeys |
|  | `loading` | `boolean` | `false` | Sound not decoded yet |
|  | `disabled` | `boolean` | `false` |  |
|  | `accent` | `string` | — | CSS colour for the pad |
|  | `variant` | `"default" \| "outline" \| "ghost"` | `"default"` |  |
|  | `size` | `"sm" \| "default" \| "lg"` | `"default"` |  |
| `SoundPadProgress` | `value` | `number` | — | 0..1, declarative |
|  | `source` | `FrameSource<number>` | — | Smooth progress with no renders |
|  | `variant` | `"bar" \| "fill" \| "ring"` | `"bar"` |  |
| `SoundPadShortcut` | — |  |  | Shows the pad's `hotkey` with shadcn `Kbd` |
| `SoundPadGrid` | `columns` | `number` | 4 | Also settable with a class |
|  | `hotkeys` | `boolean` | `false` | Listens for pad hotkeys |
|  | `hotkeyScope` | `"focus" \| "global"` | `"focus"` | `global` listens on the document, never while typing in a field |

Modes:

- `one-shot`: every press triggers; pressing again restarts.
- `toggle`: press to start, press again to stop.
- `hold`: plays while pressed.
- `loop`: like toggle, and marks the pad as looping.

### Styling

- Data attributes: `data-playing`, `data-pressed`, `data-mode`, `data-loading`, `data-disabled`.
- CSS variables: `--pad-accent`, `--pad-progress` (live, 0..1).

### Accessibility

- A real `<button>`. In `toggle` and `loop` modes it has `aria-pressed`.
- The grid uses arrow keys in two dimensions between pads.
- Hotkeys are announced through `aria-keyshortcuts`.

### Docs examples

Single pad, the four modes, progress variants, grid with hotkeys, pad with a context menu for volume, accent colours, loading state.

---

## Registry items

| Item | npm dependencies | Registry dependencies |
| --- | --- | --- |
| `audio-player` | `@base-ui/react` | `@audiocn/core`, `@audiocn/use-audio-player`, `@audiocn/volume-control` |
| `track-list` | `class-variance-authority` | `@audiocn/core` |
| `sound-pad` | `class-variance-authority` | `kbd`, `@audiocn/core`, `@audiocn/use-frame-source` |

## Tests

- `audio-player`: play and pause state, seeking, volume and mute, rate, the keyboard table, external `player`, error state, against a mocked media element.
- `track-list`: selection, arrow navigation, nested action focus.
- `sound-pad`: the four modes, hotkeys in both scopes, no hotkey while typing, grid navigation.
