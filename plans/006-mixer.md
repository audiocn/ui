# 006 — Mixer

Status: PLANNED (2026-09-30). Follows plan 002; builds on plans 004 and 005.

Two components: `channel-strip` and `mixer`. Both are layout and context only. They hold no audio and no state; the meters, faders and toggles from plans 004 and 005 are composed inside them, the way a shadcn `Card` holds its content.

A master or output strip is a `ChannelStrip` with `variant="master"`, not a separate component.

---

## `channel-strip`

One channel of a mixer: a source with its meter and controls.

### Anatomy

```tsx
<ChannelStrip muted={muted}>
  <ChannelStripHeader>
    <ChannelStripIcon>
      <MicrophoneIcon />
    </ChannelStripIcon>
    <ChannelStripTitle>Microphone</ChannelStripTitle>
    <ChannelStripDescription>MacBook Pro Microphone</ChannelStripDescription>
    <ChannelStripStatus>Live</ChannelStripStatus>
    <ChannelStripActions>
      <Button size="icon" variant="ghost" />
    </ChannelStripActions>
  </ChannelStripHeader>
  <ChannelStripMeter>
    <LevelMeter source={graph.meters.mic} />
  </ChannelStripMeter>
  <ChannelStripFader>
    <Fader value={gainDb} onValueChange={setGainDb} />
  </ChannelStripFader>
  <ChannelStripValue>
    <DbReadout source={graph.meters.mic} />
  </ChannelStripValue>
  <ChannelStripControls>
    <MuteToggle pressed={muted} onPressedChange={setMuted}>
      M
    </MuteToggle>
    <SoloToggle pressed={solo} onPressedChange={setSolo}>
      S
    </SoloToggle>
  </ChannelStripControls>
  <ChannelStripNotice variant="warning">
    Microphone access is needed.
  </ChannelStripNotice>
</ChannelStrip>
```

Every part is optional. The smallest useful strip is a title, a meter and a mute toggle.

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `ChannelStrip` | `orientation` | `"horizontal" \| "vertical"` | from `Mixer`, else `"horizontal"` | Horizontal is a row; vertical is a console strip |
|  | `variant` | `"default" \| "card" \| "ghost" \| "master"` | `"default"` |  |
|  | `size` | `"sm" \| "default" \| "lg"` | from `Mixer` |  |
|  | `muted` | `boolean` | `false` | Styling and context only |
|  | `solo` | `boolean` | `false` |  |
|  | `dimmed` | `boolean` | `false` | Silenced by another channel's solo |
|  | `selected` | `boolean` | `false` |  |
|  | `disabled` | `boolean` | `false` | Disables every control inside |
|  | `accent` | `string` | — | CSS colour for the channel's colour tag |
|  | `render` | element | `<div>` |  |
| `ChannelStripStatus` | `tone` | `"default" \| "live" \| "muted" \| "warning" \| "error"` | `"default"` | Rendered with shadcn `Badge` |
| `ChannelStripNotice` | `variant` | `"default" \| "warning" \| "destructive"` | `"default"` | A compact inline alert; children can include an action button |

### Context

`ChannelStrip` provides `orientation`, `size`, `disabled` and `muted` to the audiocn components inside it:

- `LevelMeter` and `Fader` take the strip's orientation and size.
- Meters and visualizers render dimmed when the strip is muted or dimmed.
- `useChannelStrip()` is exported for custom parts.

### Layout

- The strip is a CSS grid with named areas (`header`, `meter`, `fader`, `value`, `controls`, `notice`). Each part places itself in its area.
- Horizontal: header on the left, meter and fader filling the middle, controls on the right, notice on a second row.
- Vertical: header on top, meter and fader side by side in the middle, controls and value at the bottom.
- Users rearrange by overriding `grid-template-areas` with a class.

### Styling

- Data attributes: `data-orientation`, `data-variant`, `data-size`, `data-muted`, `data-solo`, `data-dimmed`, `data-selected`, `data-disabled`, `data-clipping` (set when a meter inside is clipping).
- CSS variables: `--channel-accent`, `--channel-strip-width` (vertical), `--channel-strip-height` (horizontal).

### Accessibility

- `role="group"` labelled by `ChannelStripTitle`.
- `ChannelStripNotice` is `role="status"`, or `role="alert"` for the `destructive` variant.
- State is never colour only: muted strips also show the pressed toggle and a status label.

### Docs examples

Minimal strip, full row strip, console strip, master strip with stereo meter and clip count, strip with a bar visualizer in place of a meter, strip with a switch in place of a fader (on/off source), notice states (permission needed, device lost, paused), accent colours, sizes.

---

## `mixer`

The container for a set of channel strips.

### Anatomy

```tsx
<Mixer orientation="vertical">
  <MixerHeader>
    <MixerTitle>Audio mixer</MixerTitle>
    <MixerActions>
      <Button size="sm" variant="ghost">
        Reset
      </Button>
    </MixerActions>
  </MixerHeader>
  <MixerChannels>
    {channels.map((channel) => (
      <ChannelStrip key={channel.id}>{/* … */}</ChannelStrip>
    ))}
  </MixerChannels>
  <MixerSeparator />
  <MixerMaster>
    <ChannelStrip variant="master">{/* … */}</ChannelStrip>
  </MixerMaster>
  <MixerEmpty>No audio sources yet.</MixerEmpty>
</Mixer>
```

### Props

| Part | Prop | Type | Default | Notes |
| --- | --- | --- | --- | --- |
| `Mixer` | `orientation` | `"horizontal" \| "vertical"` | `"horizontal"` | Horizontal stacks row strips; vertical lays console strips side by side |
|  | `size` | `"sm" \| "default" \| "lg"` | `"default"` | Inherited by every strip and control |
|  | `minDb`, `maxDb` | `number` | −60, 0 | Shared meter range |
|  | `zones` | `MeterZone[]` | `DEFAULT_ZONES` | Shared meter zones |
|  | `ballistics` | see `level-meter` | `"peak"` | Shared meter movement |
|  | `disabled` | `boolean` | `false` |  |
| `MixerChannels` | `scrollable` | `boolean` | `true` | Scrolls along the strip axis when strips overflow |
| `MixerEmpty` | — |  |  | Shown when `MixerChannels` has no children |

### Behaviour

- Provides the shared meter configuration through context, so every meter in the mixer uses the same scale and movement.
- Keyboard navigation between strips: with focus on a control, Ctrl+Left and Ctrl+Right (or Ctrl+Up and Ctrl+Down in the horizontal layout) move to the same control on the neighbouring strip.
- The master area stays in place while the channels scroll.
- `useMixerContext()` is exported for custom parts.

### Styling

- Data attributes: `data-orientation`, `data-size`, `data-empty`.
- CSS variables: `--mixer-gap`.

### Accessibility

- `role="group"` with a label from `MixerTitle` or `aria-label`.
- Strips are reachable in DOM order with Tab; the Ctrl+arrow shortcut is an addition, not a replacement.

### Docs examples

Row mixer, console mixer, mixer with a master strip, scrolling mixer with many channels, empty state, mixer driven by `use-mixer` and `use-demo-signal`, and a live Web Audio mixer driven by `use-web-audio-mixer`.

---

## Registry items

| Item | npm dependencies | Registry dependencies |
| --- | --- | --- |
| `channel-strip` | `@base-ui/react`, `class-variance-authority` | `badge`, `@audiocn/core` |
| `mixer` | `class-variance-authority` | `@audiocn/core`, `@audiocn/channel-strip` |

Meters, faders and toggles are not dependencies of these two: the user installs the parts they compose. The blocks in plan 008 pull everything in together.

## Tests

- Context inheritance: orientation, size, disabled and meter configuration reach nested components, and explicit props override them.
- Data attributes reflect `muted`, `solo`, `dimmed`, `selected`.
- Ctrl+arrow navigation moves between strips and stops at the ends.
- `MixerEmpty` appears only with no channels.
- Performance: 16 strips with live meters, no React commits in steady state.
