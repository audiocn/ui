# 012 — Effects audit

Status: PLANNED (2026-10-01).

Every `useEffect` and `useLayoutEffect` call in shipped and site code was reviewed: 87 call sites (78 `useEffect`, 9 `useLayoutEffect`) in 41 files. Tests and e2e specs were excluded. Five parallel reviews each read whole files against one rubric. Every bug marked ✓ below was then confirmed by reading the code, and those marked (T) were reproduced in a throwaway vitest.

## Verdict

The effects are mostly the right tool. This is an audio library: Web Audio graphs, media elements, canvases, observers and rAF loops all have to be synchronised from effects. The real problems are elsewhere:

- Cleanups that don't reset what they tear down, which breaks under `<Activity>` and StrictMode.
- Callbacks in dependency arrays, which re-run effects on every parent render.
- A few effects that should be render code or event code.

| Verdict | Count | Meaning |
| --- | --- | --- |
| Keep as is | 50 | Correct sync with an external system, with symmetric cleanup. |
| Keep, but change | 25 | Has to stay an effect, but has a bug or needless re-runs. |
| Remove | 12 | Render code, event code, a store subscription, an asymmetric cleanup, or a latest-ref that `useEffectEvent` replaces. |

Removing 12 and splitting 2 (one into three, one into two) leaves about 78 call sites. The count barely moves. The value of the work is the bug fixes, not fewer effects.

## Bugs, by impact

Each item names the file, what users see, and the fix.

1. ✓ **`hooks/use-microphone.ts:122`.** After `stop()` then `start()` with the same constraints, the hook returns the old, stopped stream as "active" until `getUserMedia` resolves again. Reproducible in quick-audio-popover (turn off, then on). Fix: the cleanup drops this run's result: `setResult((r) => (r?.key === key ? null : r))`.
2. ✓ (T) **`hooks/use-audio-player.ts:259`.** volume, muted and playbackRate share one effect, so changing any one prop overwrites the other two. That includes a volume the user set through the imperative setters. `<Activity>` show also resets them. Fix: split into three effects, each applying only when its own prop changed.
3. **`hooks/use-audio-player.ts:239`.**
   - Setting `src` to undefined removes the attribute without calling `load()`, so the old track keeps playing while status reads "idle".
   - (T) Toggling `autoPlay` or changing `preload` reloads the track from 0.
   - `crossOrigin` is never cleared.
   - Fix: a separate `preload` effect, `autoPlay` read through `useEffectEvent`, reload only on `src`/`crossOrigin`, and call `load()` when clearing.
4. ✓ (T) **`hooks/use-mixer.ts:244`.** On mount the save effect writes the defaults before the restored state lands. StrictMode's re-run of the load effect then reads them back, so saved mixer settings are lost on every dev reload. Production runs effects once and works. Fix: delete the effect and persist from `commit`, the user-action path.
5. ✓ **`components/blocks/system-audio-settings/system-audio-settings.tsx:91`.** `onStreamChange` is in the deps, so an inline parent callback tears down and rebuilds the live capture's source node on every parent render. A handler that stores a new object loops forever. Fix: `useEffectEvent` for the callback.
6. ✓ **`system-audio-settings.tsx:114` (controlled mode).** If capture ends while `enabled` stays `true` (picker cancelled, no audio ticked, or "Stop sharing"), the switch can't restart it. `setEnabled` only calls the parent, and the prop never changes. Fix: act in the handler when the prop won't change.
7. ✓ **`hooks/use-system-audio.ts:144`.** On `<Activity>` hide the cleanup stops the tracks but leaves status "active" and the dead stream in state. Also, `stop()` or an unmount while the picker is open leaks the capture that resolves afterwards. Fix: `useEffect(() => stop, [stop])`, plus a per-attempt cancel flag in `start()`.
8. ✓ **`components/ui/level-meter.tsx:718`.** A rebuilt painter starts from cached "not clipping / not active", so `data-clipping` or `data-active` left by the old painter stay stuck. Rebuilds happen on any dep change, on `<Activity>` hide/show, and on every render with inline `zones`, which also resets ballistics. Fix: seed the caches with `null`, and pass scale settings per frame so only `ballisticsKey` and `reducedMotion` rebuild.
9. ✓ **`components/ui/db-readout.tsx:115`.**
   - `<LevelMeter peakDb={-12}><LevelMeterValue /></LevelMeter>` shows "−12.0 dB" for one tick, then "−∞" for good, because a hold of 0 resets to silence and the declarative frame only re-emits on change.
   - Inline `format` or `zones` restart the interval on every render, so a parent re-rendering faster than 250 ms freezes the readout.
   - Fix: reset to the last frame's level instead of to silence, and use `useEffectEvent` for the tick.
10. ✓ **`components/ui/knob.tsx:277`.** The wheel changes a disabled knob once it has focus, which a click still gives it. Every knob also registers a non-passive wheel listener even with `allowWheel` off. Fix: check `disabled`, and only attach when `allowWheel` is set.
11. **`system-audio-mixer.tsx:415` and `components/ui/audio-player.tsx:152`.** `onOutputChange` and `onTimeUpdate` are in deps, so an inline callback re-fires on every parent render. Fix: `useEffectEvent`.
12. **`components/ui/sound-pad.tsx:146`, `:306`.** A held hotkey is never released when the subscription or registration is torn down (hotkeys toggled, pad disabled or loading, `<Activity>` hide). A hold pad's `onStop` never fires and `data-pressed` sticks. Fix: release held keys in cleanup, and in `register`'s unregister.
13. **`components/ui/parameter-slider.tsx:160`.** `latestRef` syncs only when `value` changes, so a controlled parent that clamps leaves it on the rejected value, and arrow keys step from the wrong base. Fix: drop the deps array, as fader and knob already do.
14. **`hooks/use-clip-hold.ts:86`.** An `<Activity>` hide during a hold cancels the release timer, so the clip light stays on after show. Fix: re-arm the release in setup.
15. **`components/blocks/music-player/music-player.tsx:183`.** The duck only moves when a frame arrives, so music stays lowered when the mic stops or ducking is turned off. `use-web-audio-mixer` has the same flaw with its `ducked` flag. Fix: restore gain in the off handler, and let the duck release itself.
16. **`hooks/use-sound.ts:236`.** Progress uses the current `loop` and `playbackRate` props, while the voice keeps the values it started with. Switching a looping pad's mode mid-play pins progress at 1. Fix: read the values from the voice.
17. **`components/ui/channel-strip.tsx:135`.** Unmounting a meter during its clip hold leaves the strip's `data-clipping` stuck. Fix: `childList: true`, plus an initial `update()`.

Minor ones to fix while in those files:

- `spectrum.tsx` decays peak hold per paint instead of per unit of time, and repaints twice a second with a frozen frame.
- The bar visualizers freeze when `levels` goes to undefined.
- Waveform hover sticks when interactivity turns off mid-hover.
- `use-web-audio-mixer` lets muted strips leak about 10 ms at unity gain when built.
- `use-waveform-data` reports "loading" forever without Web Audio.
- `use-audio-analyser`'s `enabled` doc comment is wrong.
- `music-player`'s `autoPlay: index > 0` stops playback when repeat-all wraps to track 0.
- The knob's detent step doesn't match its drawn long ticks for `majorEvery <= 0`.

## Effects that go away

| Effect | Replacement |
| --- | --- |
| `hooks/use-mixer.ts:244` (persist) | Write storage in `commit` (bug 4). |
| `components/blocks/soundboard/soundboard.tsx:99` (`stopSignal` counter) | Pads expose `stop` through an imperative handle, and "Stop all" calls them from its click handler. |
| `components/ui/level-meter.tsx:755` (`--meter-fill`, `--meter-mask`) | Compute in render, in `style`. They then also reach SSR HTML, and a user's `style` merges predictably. |
| `components/ui/sound-pad.tsx:461`, `:470` (`--pad-progress`) | One render-time `style` value. |
| `components/ui/waveform.tsx:274` (hover subscribe + setState) | `useSyncExternalStore`. |
| `hooks/use-sound.ts:255` (cleanup-only `output.disconnect()`) | Delete. It drops connections the consumer made under `destination: null`, and the hook's own connection is already removed at :159. |
| `system-audio-mixer.tsx:403` (cleanup-only `soundBus.disconnect()`) | Delete. The mixer graph already disposes that connection. |
| `hooks/use-frame-source.ts:23` (latest callback ref) | `useEffectEvent`. This is the most-copied hook (12 components). |
| `hooks/use-audio-player.ts:128` (callbacks ref) | `useEffectEvent` inside `usePlaybackState`. |
| `hooks/use-web-audio-mixer.ts:450` (inputs ref) | `useEffectEvent`. |
| `components/ui/sound-pad.tsx:281` (latest ref) | Plain closures for JSX handlers, plus `useEffectEvent` wrappers for the grid registry. This is optional. |

Latest-refs that must stay: `use-clip-hold:36`, `use-mixer:208`, and the value refs in `fader`, `knob` and `parameter-slider`. Each is read from JSX handlers or from functions returned to callers, where an effect event can't be called.

## Effects to keep but improve, beyond the bugs

- **`useEffectEvent` for values read only inside a listener or loop**, so effects stop re-running:
  - `components/theme-provider.tsx:22`: the D hotkey re-subscribes on every theme change.
  - `hooks/use-level.ts:74`: an inline `zones` restarts the interval, and the zone goes stale.
  - The fader and knob wheel handlers.
  - `lineWidth` and `fadeEdges` in `smooth-waveform.tsx:153` and `electric-waveform.tsx:616`, which currently reset the animation.
- **`use-audio-player.ts:132`:** resync from the element on setup, so a `pause` lost during an `<Activity>` hide doesn't leave status "playing". This is required once bug 3 stops reloading on every setup.
- **Visibility gating:** the same IntersectionObserver effect is copied in five components. `live-waveform`, `spectrum` and `waveform` don't gate at all, so they repaint while off screen. Promote `level-meter`'s `useVisibility` to `hooks/use-visibility.ts` (`registry:hook`) and use it in all seven.

## The rule behind most of these bugs

Under `<Activity>`, a hide runs cleanups and keeps state, and a show runs setup again. If a cleanup destroys something setup can't recreate (a capture, a mic stream, a pending timer, a held key), the cleanup must also reset the state that describes it. Bugs 1, 7, 12 and 14 break this rule. None of them shows up under StrictMode alone, so add an `<Activity>` hide/show test for each fix.

Related: never put a callback prop in an effect's deps when the effect only calls it. Bugs 5, 9 and 11, plus the `use-level` and theme-provider items, all come from that.

## Batches

Each batch gets its own commit, with tests (including `<Activity>` and StrictMode cases), `pnpm test`, `pnpm typecheck`, `pnpm exec ultracite check`, `pnpm react-doctor`, and e2e for the touched surfaces.

1. **Media hooks:** bugs 1, 2, 3, 7, 16, plus the `use-sound:255` deletion and the `:132` resync.
2. **Mixer persistence and the blocks:** bugs 4, 5, 6, 11 (mixer), 15, the `stopSignal` handle, and the `:403` deletion.
3. **Meters and readouts:** bugs 8, 9, 14, 17, and the level-meter render-time style.
4. **Controls:** bugs 10, 12, 13, the sound-pad render-time progress, and the `audio-player` `onTimeUpdate` fix.
5. **`useEffectEvent` sweep** (needs Decision 1): `use-frame-source`, `use-audio-player:128`, `use-web-audio-mixer:450`, `use-level`, theme-provider, wheel handlers, waveform stroke props.
6. **Visualizers:** `WaveformHover` to `useSyncExternalStore`, the shared `useVisibility`, and the spectrum and bar fixes.

## Decisions

1. **React floor.** `useEffectEvent` needs React 19.2, but the docs and README promise "React 19". Bugs 5, 9 and 11 and batch 5 are cleanest with it.
   - Recommendation: raise the documented minimum to React 19.2 before launch.
   - Alternative: keep the refs in registry files and use `useEffectEvent` only in site code.
2. **Shared gain hook.** "Gain node in `useMemo`, then connect, then ramp" repeats in four blocks and in `use-sound`. A `useGainNode` registry hook would remove about seven effects. Every current copy is correct, so the payoff is consistency. Recommendation: defer.
3. **Scope.** Recommendation: batches 1–4 (bugs) first, then 5–6.

## Appendix: all 87 call sites

Keep: no change. Change: keep the effect, apply the fix above. Remove: see "Effects that go away".

| Call site | Verdict |
| --- | --- |
| `hooks/use-audio-player.ts:128` | Remove (`useEffectEvent`) |
| `hooks/use-audio-player.ts:132` | Change (call effect events, resync on setup) |
| `hooks/use-audio-player.ts:232` | Keep |
| `hooks/use-audio-player.ts:239` | Change (bug 3) |
| `hooks/use-audio-player.ts:259` | Change (bug 2, split in three) |
| `hooks/use-audio-player.ts:270` | Keep |
| `hooks/use-audio-player.ts:280` | Keep |
| `hooks/use-mixer.ts:208` | Keep (ref read from returned handlers) |
| `hooks/use-mixer.ts:234` | Keep |
| `hooks/use-mixer.ts:244` | Remove (bug 4) |
| `hooks/use-sound.ts:101` | Keep |
| `hooks/use-sound.ts:159` | Keep |
| `hooks/use-sound.ts:174` | Keep |
| `hooks/use-sound.ts:236` | Change (bug 16) |
| `hooks/use-sound.ts:253` | Keep |
| `hooks/use-sound.ts:255` | Remove |
| `hooks/use-web-audio-mixer.ts:450` | Remove (`useEffectEvent`) |
| `hooks/use-web-audio-mixer.ts:454` | Keep |
| `hooks/use-web-audio-mixer.ts:467` | Change (read inputs via effect event) |
| `hooks/use-web-audio-mixer.ts:478` | Keep |
| `hooks/use-web-audio-mixer.ts:482` | Keep (duck flag fix lives in `graph.duck`) |
| `hooks/use-audio-analyser.ts:287` | Keep (fix the doc comment) |
| `hooks/use-audio-context.tsx:94` | Keep |
| `hooks/use-audio-devices.ts:132` | Keep |
| `hooks/use-clip-hold.ts:36` | Keep (ref read from returned functions) |
| `hooks/use-clip-hold.ts:86` | Change (bug 14) |
| `hooks/use-demo-signal.ts:339` | Keep |
| `hooks/use-frame-source.ts:23` | Remove (`useEffectEvent`) |
| `hooks/use-frame-source.ts:27` | Keep (calls the effect event) |
| `hooks/use-level.ts:74` | Change (`useEffectEvent`, zone in render) |
| `hooks/use-microphone.ts:122` | Change (bug 1) |
| `hooks/use-system-audio.ts:144` | Change (bug 7) |
| `hooks/use-waveform-data.ts:96` | Keep (minor: no-Web-Audio state) |
| `lib/docs/use-demo-audio.ts:48` | Keep |
| `components/theme-provider.tsx:22` | Change (`useEffectEvent`) |
| `components/docs/theme-picker.tsx:63` | Keep |
| `components/home/showcase-tile.tsx:92` | Keep |
| `components/home/web-threads.tsx:309` | Keep |
| `components/home/web-threads.tsx:427` | Keep |
| `components/social/social-card.tsx:13` | Keep |
| `components/ui/audio-player.tsx:152` | Change (bug 11) |
| `components/ui/channel-strip.tsx:135` | Change (bug 17) |
| `components/ui/db-readout.tsx:115` | Change (bug 9) |
| `components/ui/db-scale.tsx:241` | Keep |
| `components/ui/fader.tsx:94` | Keep (`useLatest`, value ref stays) |
| `components/ui/fader.tsx:715` | Change (wheel via effect event) |
| `components/ui/knob.tsx:170` | Keep (`useLatest`, value ref stays) |
| `components/ui/knob.tsx:277` | Change (bug 10) |
| `components/ui/knob.tsx:557` | Keep (minor: detent step) |
| `components/ui/level-meter.tsx:603` | Keep (becomes the shared `useVisibility`) |
| `components/ui/level-meter.tsx:689` | Keep |
| `components/ui/level-meter.tsx:718` | Change (bug 8) |
| `components/ui/level-meter.tsx:755` | Remove (render-time style) |
| `components/ui/parameter-slider.tsx:160` | Change (bug 13) |
| `components/ui/sound-pad.tsx:146` | Change (bug 12) |
| `components/ui/sound-pad.tsx:281` | Remove (optional, `useEffectEvent`) |
| `components/ui/sound-pad.tsx:306` | Change (bug 12) |
| `components/ui/sound-pad.tsx:461` | Remove (render-time style) |
| `components/ui/sound-pad.tsx:470` | Remove (render-time style) |
| `components/ui/bar-visualizer.tsx:114` | Keep (minor: freeze on undefined) |
| `components/ui/bar-visualizer.tsx:122` | Keep (shared `useVisibility`) |
| `components/ui/bar-visualizer.tsx:138` | Keep |
| `components/ui/electric-bar-visualizer.tsx:750` | Keep |
| `components/ui/electric-bar-visualizer.tsx:758` | Keep (shared `useVisibility`) |
| `components/ui/electric-bar-visualizer.tsx:774` | Keep |
| `components/ui/electric-waveform.tsx:600` | Keep (shared `useVisibility`) |
| `components/ui/electric-waveform.tsx:616` | Change (stroke props via effect event) |
| `components/ui/live-waveform.tsx:309` | Keep (add visibility gating) |
| `components/ui/smooth-waveform.tsx:137` | Keep (shared `useVisibility`) |
| `components/ui/smooth-waveform.tsx:153` | Change (stroke props via effect event) |
| `components/ui/spectrum.tsx:172` | Change (colour compare, decay per time) |
| `components/ui/waveform.tsx:104` | Keep |
| `components/ui/waveform.tsx:274` | Remove (`useSyncExternalStore`) |
| `components/ui/waveform.tsx:675` | Keep |
| `components/blocks/mic-setup/mic-setup.tsx:107` | Keep |
| `components/blocks/mic-setup/mic-setup.tsx:116` | Keep |
| `components/blocks/mic-setup/mic-setup.tsx:135` | Keep |
| `components/blocks/music-player/music-player.tsx:159` | Keep (bug 15 is in the frame handler) |
| `components/blocks/soundboard/soundboard.tsx:99` | Remove (imperative handle) |
| `components/blocks/soundboard/soundboard.tsx:208` | Keep |
| `components/blocks/soundboard/soundboard.tsx:222` | Keep |
| `components/blocks/system-audio-mixer/system-audio-mixer.tsx:403` | Remove |
| `components/blocks/system-audio-mixer/system-audio-mixer.tsx:415` | Change (bug 11) |
| `components/blocks/system-audio-settings/system-audio-settings.tsx:83` | Keep |
| `components/blocks/system-audio-settings/system-audio-settings.tsx:91` | Change (bug 5) |
| `components/blocks/system-audio-settings/system-audio-settings.tsx:102` | Keep |
| `components/blocks/system-audio-settings/system-audio-settings.tsx:114` | Change (bug 6) |
