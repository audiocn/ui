# 024: LCD VU Meter

Status: IMPLEMENTED (2026-10-23). Follows plans 002, 004 and 020.

A dot-matrix bar-graph level meter for the LCD Display (plan 020), so a screen can show a level like a hardware panel.

```tsx
<LcdDisplay>
  <LcdDisplayRow>
    <LcdDisplayLabel>Out</LcdDisplayLabel>
    <LcdVuMeter source={analyser.meter}>
      <LcdVuMeterChannels>
        <LcdVuMeterScale />
      </LcdVuMeterChannels>
      <LcdVuMeterValue />
      <LcdVuMeterClip />
    </LcdVuMeter>
  </LcdDisplayRow>
</LcdDisplay>
```

| Part | Built on | Notes |
| --- | --- | --- |
| `LcdVuMeter` | `LevelMeter` | All Level Meter props but `variant`. `segments` (24, like the Level Meter), `mode` (`bar`, `dot`). Named by the row's label |
| `LcdVuMeterChannels` | `LevelMeterChannels` | With no children, one channel per channel in the data |
| `LcdVuMeterChannel` | `LevelMeterChannel` | With no children, a track with a peak bar and the hold |
| `LcdVuMeterTrack` | `LevelMeterTrack` | The unlit segments |
| `LcdVuMeterBar` | `LevelMeterBar` | Lit segments up to `measure`: `peak` or `rms` |
| `LcdVuMeterHold` | `LevelMeterHold` | The peak-hold segment |
| `LcdVuMeterScale` | `LevelMeterScale` | Dim ink |
| `LcdVuMeterValue` | `LevelMeterValue` | Lit ink, the display's font |
| `LcdVuMeterClip` | `LevelMeterClip` | A clip dot like the Level Meter's, unlit until clipping |

- **Composed, not forked.** The meter is a Level Meter with LCD parts, so sources, ballistics, zones, ARIA throttling, visibility sleep and reduced motion are the Level Meter's. Two small exports made that possible: `useLevelMeterLayout()` (orientation and the channel count the default anatomy uses) in `level-meter.tsx`, and `useLcdDisplayRowLabelId()` in `lcd-display.tsx`.
- **Whole segments.** The painter keeps writing a continuous `--meter-level` and `--meter-hold`; each channel snaps them down to whole segments with CSS `round()` into `--lcd-vumeter-lit` and `--lcd-vumeter-hold`, so no extra JavaScript runs per frame.
- **Layers.** Three layers share one segment mask (`--lcd-vumeter-mask`, a repeating gradient with a `--lcd-vumeter-gap` gap, the display's dot pitch by default): unlit segments at 14% of the ink, lit segments clipped to the level, and one hold segment. `dot` mode clips the lit layer to its top segment.
- **Inks.** The meter reads the display's variables, so it follows its `variant` (theme primary, amber, green, cyan, red), its appearance and every site theme, with no colour props of their own.
- **Monochrome.** An LCD has one ink, so zones are not coloured. `data-zone` and `data-clipping` stay on the meter for styling, and the clip dot lights instead.

## Delivery

1. `components/ui/lcd-vumeter.tsx` and `components/ui/lcd-vumeter.test.tsx`.
2. Examples `lcd-vumeter-demo`, `lcd-vumeter-modes`, `lcd-vumeter-inks` and `lcd-vumeter-vertical`, and `content/docs/components/lcd-vumeter.mdx`.
3. Docs nav (Meters and visualizers, after Spectrum), the components index, the README table and count, a `registry.json` item and a social card.
4. Typecheck, lint, unit tests, registry build, production build, and a visual check in light and dark across every site theme and ink.
