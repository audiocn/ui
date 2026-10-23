# 022 — LCD Number Field

Status: IMPLEMENTED (2026-10-23). Follows plans 002, 005 and 020.

An editable number for the LCD Display (plan 020), so a screen can set a value like a hardware panel: type it, step it, or scrub it like an encoder.

```tsx
<LcdDisplayRow>
  <LcdNumberField defaultValue={128} max={300} min={20}>
    <LcdNumberFieldScrubArea>
      <LcdDisplayLabel>Tempo</LcdDisplayLabel>
    </LcdNumberFieldScrubArea>
    <LcdNumberFieldGroup>
      <LcdNumberFieldInput />
      <LcdNumberFieldUnit>Bpm</LcdNumberFieldUnit>
      <LcdNumberFieldStepper />
    </LcdNumberFieldGroup>
  </LcdNumberField>
</LcdDisplayRow>
```

| Part | Built on | Notes |
| --- | --- | --- |
| `LcdNumberField` | Base UI `NumberField.Root` | `display: contents`, so its parts sit in the row. `allowWheelScrub` on by default |
| `LcdNumberFieldScrubArea` | `NumberField.ScrubArea` | Wraps the label, like an encoder's legend. A pixel scrub cursor |
| `LcdNumberFieldGroup` | `NumberField.Group` | Value, unit and steppers on one line |
| `LcdNumberFieldInput` | `NumberField.Input` | Lit ink, inverted selection. Named by the row's label, described by the unit |
| `LcdNumberFieldUnit` | `span` | Dim ink |
| `LcdNumberFieldIncrement`, `Decrement` | `NumberField.Increment`, `Decrement` | 5 × 3 pixel chevrons |
| `LcdNumberFieldStepper` | `span` | The increment over the decrement |
| `LcdNumberFieldReset` | `button` | Returns to `resetValue` (default `defaultValue ?? min`), like the Parameter Slider's and the Fader's reset. Hidden while unmodified |

- The scrub area wraps the label rather than the value, because Base UI's scrub area takes pointer lock on press, which would stop the value from being clicked into and typed.
- **Inks.** The field reads the display's variables, so it follows its `variant` (theme primary, amber, green, cyan, red), its appearance and every site theme, with no colour props of its own.
- The input sizes to its text with `field-sizing: content`, between 3 and 12 characters.

- `useLcdDisplayRowLabelId()`, exported from `lcd-display.tsx`, names the input by its row's label, the way the display's own select is named.

## Delivery

1. `components/ui/lcd-number-field.tsx` and `components/ui/lcd-number-field.test.tsx`.
2. Examples `lcd-number-field-demo` and `lcd-number-field-inks`, and `content/docs/components/lcd-number-field.mdx`.
3. Docs nav (Controls, after LCD Display), the components index, the README table and count, a `registry.json` item and a social card.
4. Typecheck, lint, unit tests, registry build, production build, and a visual check in light and dark across every site theme and ink.
