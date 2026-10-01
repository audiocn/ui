# 011 — React Doctor cleanup

Status: PLANNED (2026-10-01).

`npx react-doctor@latest --verbose -y --no-score` (`--no-score` keeps the private repo off the score API) found **59 findings in 285 files: 5 errors and 54 warnings**. After reading the code behind each one, 17 get fixed in source and 42 are false positives or deliberate patterns, which get waived in config with a reason. One real bug the scan missed (hardcoded DOM ids in blocks) is added.

## The rule for this cleanup

Registry files are copied into users' repos. A waiver in our local config only hides a finding here. A user who runs React Doctor on their app still sees it, coming from our component. So:

1. If a clean, idiomatic fix exists for a registry file, fix it in source, even when the finding is a false positive (for example the 5 hook "errors").
2. Waive in `doctor.config.ts` only when the pattern is deliberate (the conventions in plan 002) or the file is vendored upstream code.
3. Never put `react-doctor-disable` comments in registry source. They would ship to users.

## Triage

| Rule | # | Verdict | Action |
| --- | --- | --- | --- |
| `effect-needs-cleanup` (error) | 5 | False positive: every listener is released, but the matcher can't see it | Fix anyway (registry hooks), batch 1 |
| `no-create-object-url-without-revoke`, soundboard | 1 | **Real leak** | Fix, batch 2 |
| `no-create-object-url-without-revoke`, `lib/docs/demo-audio.ts` | 1 | Deliberate page-lifetime cache for a fixed set of demo tracks | Waive |
| `no-high-complexity-react-function`, MicSetup | 1 | Real (maintainability) | Extract hooks, batch 4 |
| `rerender-lazy-state-init` | 4 | No measurable impact (`clamp` is O(1)), but it ships to users | Fix, batch 5 |
| `no-spread-props-over-defaults-clobbers-with-undefined` | 1 | Real but low impact (an explicit `undefined` gives NaN uniforms) | Fix, batch 5 |
| `js-hoist-intl` | 2 | Negligible | Fix, batch 5 |
| `require-pnpm-hardening` | 2 | Real | Fix, batch 6 |
| `only-export-components`, `theme-picker.tsx` `THEMES` | 1 | Real, local file | Fix, batch 5 |
| `only-export-components`, everything else | 31 | By design or a false positive (see below) | Waive |
| `use-lazy-motion` | 2 | Real ~30 kB, but in vendored ncdai files | **Decision 1** |
| `no-pass-data-to-parent`, system-audio-mixer | 1 | Documented exception: the block owns the Web Audio graph, so the parent can't produce the stream itself | Waive |
| `no-fetch-in-effect`, use-sound | 1 | False positive: client-only decode into an `AudioContext`, with a cancel flag and a shared promise cache | Waive |
| `prefer-use-effect-event`, sound-pad | 1 | False positive: `handleDown` deps ⊆ effect deps, and it's also used in a JSX handler | Waive |
| `prefer-tag-over-role`, level-meter | 1 | False positive: `<meter>` can't hold the custom channel/bar children | Waive |
| `js-index-maps`, electric-bar-visualizer | 1 | False positive: fixed pool of 3 arcs, 2 attempts per frame | Waive |
| `no-array-index-as-key`, `field.tsx` | 1 | Stock shadcn file, static error list | Waive (upstream parity) |
| `duplicate-jsx-subtree`, examples | 2 | Docs examples are standalone on purpose | Waive |

The 32 `only-export-components` findings break down as:

- 9 are exported `cva` variants (`faderVariants`, …), required by convention 7.
- 8 are components that return `useRender(...)` (`AudioPlayerPlay`, `FaderReset`, `ParameterSliderReset`, …). That's a detector bug, worth reporting upstream.
- 8 are public helpers that the docs or tests use (`formatPan`, `parsePan`, `describePan`, `parseKnobValue`, `thinDbScaleLabels`, `layoutElectricBars`, `createElectricScene`, `createElectricTrace`). They stay in the file under convention 1.
- 4 are stock shadcn files (badge, button, tabs, toggle).
- 1 is `getSharedAudioContext` (registry hook API) and 1 is `convertNpmCommand` (vendored).
- 1 is `THEMES`, which gets fixed.

## Batch 1: hook listener cleanup (5 errors, registry hooks)

These are behavior-preserving rewrites. A scratch test confirmed that the current add/remove loop is flagged and both of these idioms pass.

- `hooks/use-audio-player.ts:132`: create one `AbortController`, pass `{ signal }` in the `addEventListener` loop, and make the cleanup `controller.abort()`. The second loop goes away.
- `hooks/use-audio-devices.ts:132`: put `{ signal }` on `devicechange` and on the `PermissionStatus` `change` listener. Check `signal.aborted` after the `await`. This replaces the `disposed` flag and the mutable `status` variable.
- `hooks/use-microphone.ts:122`: put `{ signal }` on the track `ended` listeners. `cancelled` becomes `signal.aborted`. Cleanup is `controller.abort(); stopStream(acquired)`.
- `hooks/use-sound.ts:210`: add `{ once: true }`, since a source node ends exactly once.
- `hooks/use-system-audio.ts:121`: add `{ once: true }`, since a track ends once.

Verify with `hooks/hooks.test.tsx` and `e2e/mic-setup.spec.ts`.

## Batch 2: soundboard object URL leak (real bug)

`components/blocks/soundboard/soundboard.tsx:175`: dropped files get `blob:` URLs that are never revoked.

- Track the URLs this board created in a `useRef(new Set<string>())`.
- When a newer removal replaces `removedSound`, revoke the previous one's `src`, because that removal can no longer be undone.
- On unmount, revoke what's left (see **Decision 2** for controlled mode).
- Never revoke URLs the board didn't create (`defaultSounds`, URLs the user passes in).
- Add a test: add a file, remove it, remove another, and assert that `revokeObjectURL` was called once with the first URL.

## Batch 3: hardcoded DOM ids (not flagged by React Doctor)

Fixed `id`s on switch/label pairs collide when two instances render on one page, so clicking a label toggles the wrong switch. Replace them with `useId()`:

- `components/blocks/mic-setup/mic-setup.tsx:285` (`mic-setup-mute`)
- `components/blocks/music-player/music-player.tsx:335` (`music-ducking`)
- `components/blocks/system-audio-settings/system-audio-settings.tsx:170` (`system-audio-enabled`)
- `components/blocks/quick-audio-popover/quick-audio-popover.tsx:119` (`quick-system-audio`)

Check e2e specs for selectors that use these ids. The `id="microphone"` props in system-audio-mixer are channel ids, not DOM ids, so they stay.

## Batch 4: MicSetup complexity

`components/blocks/mic-setup/mic-setup.tsx:109`: cyclomatic complexity 15, cognitive 16. Extract two hooks in the same file (a block is one file):

- `useMicInput(context, stream, gainDb, muted)`: the gain node, the source connect effect and the gain ramp effect (lines 130–155).
- `useLevelCheck(meter)`: `checking`, `result`, `peakRef`, the timer effect and a `start()` (lines 134–175 and 291–297).

The JSX stays as it is. Re-run the scan to confirm the function drops below the threshold.

## Batch 5: small fixes

- `rerender-lazy-state-init`: use `useState(() => clamp(...))` in
  - `knob.tsx:1065`
  - `pan-control.tsx:108`
  - `parameter-slider.tsx:156`
  - `volume-control.tsx:261`
- `components/home/web-threads.tsx:299`: drop `undefined` keys from `props` before merging over `DEFAULTS`, so an explicit `undefined` doesn't clobber a default.
- `components/docs/theme-picker.tsx:18`: move `THEMES`, `ThemeName` and `isTheme` to `lib/docs/themes.ts`. Update `theme-swatches.tsx` and `search-dialog.tsx`.
- `components/docs/github-stars.tsx:29,35`: hoist the `en-US` formatters to module scope, and build new ones only when `locales` differs.

## Batch 6: pnpm hardening

Add to `pnpm-workspace.yaml` (pnpm 10.33 supports both):

```yaml
minimumReleaseAge: 10080 # 7 days
trustPolicy: no-downgrade
```

Then run `pnpm install` to confirm the lockfile still resolves. Expect future `pnpm up` to skip releases younger than 7 days.

## Batch 7: waivers in `doctor.config.ts`

A local, non-shipped config with `ignore.overrides` for the 42 waived findings. Group them by reason, each with a comment, and reuse the `SHADCN_FILES` / `NCDAI_FILES` lists from `oxlint.config.ts`. Then re-run `npx react-doctor@latest --verbose -y --no-score`. The target is 0 errors and 0 unwaived warnings.

## Verification, after each batch

`pnpm test`, `pnpm typecheck`, `pnpm exec ultracite check`, `npx react-doctor@latest --verbose -y --no-score --scope changed`, and `pnpm test:e2e` after batches 1, 3 and 4.

## Decisions

1. **LazyMotion in vendored ncdai files** (`copy-button.tsx`, `icon-swap.tsx`). Switching to `LazyMotion` + `m` saves about 30 kB on every docs page with a code block, but it breaks the "stay as upstream writes them" rule in `oxlint.config.ts`. Recommendation: keep upstream parity and waive it.
2. **Soundboard in controlled mode, on unmount.** Revoking would break a parent that remounts the board with the same `sounds`. Recommendation: revoke on unmount only in uncontrolled mode, and document that in controlled mode the parent owns the URLs.
3. **Install React Doctor as a dev dependency** (`npx react-doctor install --yes`, which adds skill files and a `doctor` script). Recommendation: yes, so the repo stays clean before launch, run with `--no-score` while the repo is private.
