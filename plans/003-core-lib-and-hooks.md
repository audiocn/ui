# 003 — Core: `lib/audio` and hooks

Status: PLANNED (2026-09-30). Follows the conventions in plan 002.

The `ui` components never touch Web Audio. Everything that does lives here, and every hook here is optional: a consumer with its own audio engine uses only `lib/audio` and the presentation hooks.

## `lib/audio` (registry item `@audiocn/core`)

Pure TypeScript. No React, no DOM, no Web Audio. Fully unit tested.

### `types.ts`

```ts
interface ChannelLevel {
  peakDb: number;
  rmsDb?: number;
}

interface MeterFrame {
  channels: ChannelLevel[]; // 1 = mono, 2 = stereo
}

interface VisualFrame {
  bands: Float32Array; // 0..1 per frequency band
  history: Float32Array; // ring buffer of 0..1 levels
  historyStart: number;
  historyLength: number;
  timeDomain?: Float32Array; // −1..1, for line and scope drawing
  peakDb: number;
}

interface FrameSource<T> {
  subscribe: (callback: (frame: T) => void) => () => void;
}

type MeterZoneName = "ok" | "warn" | "clip";

interface MeterZone {
  fromDb: number;
  zone: MeterZoneName;
}

interface Taper {
  toPosition: (db: number) => number; // 0..1
  toDb: (position: number) => number;
}
```

### `decibels.ts`

| Export | Behaviour |
| --- | --- |
| `DEFAULT_MIN_DB = -60`, `DEFAULT_MAX_DB = 0` | Default meter range |
| `dbToGain(db)` | `10^(db/20)`; `-Infinity` gives 0 |
| `gainToDb(gain)` | `20·log10(gain)`; 0 gives `-Infinity` |
| `dbToLevel(db, minDb?, maxDb?)` | 0..1, linear in dB, clamped |
| `levelToDb(level, minDb?, maxDb?)` | Inverse of `dbToLevel` |
| `clampDb(db, min, max)` | Clamp that keeps `-Infinity` when allowed |
| `formatDb(db, options?)` | "−12.3 dB", "+3.0 dB", "−∞ dB"; options: `decimals` (1), `unit` (true), `sign` ("auto") |
| `peakDb(samples)`, `rmsDb(samples)` | From a time-domain `Float32Array` |

### `ballistics.ts`

How a meter moves between readings.

```ts
interface BallisticsOptions {
  attackMs: number; // default 15
  releaseMs: number; // default 350
  peakHoldMs: number; // default 1200
  peakReleaseMs: number; // default 600
}

const createBallistics: (options?: Partial<BallisticsOptions>) => {
  step: (inputDb: number, nowMs: number) => { db: number; holdDb: number };
  reset: () => void;
};
```

Presets exported as `BALLISTICS`: `peak` (the defaults above), `vu` (300 ms attack and release, no hold), `instant` (no smoothing).

### `zones.ts`

- `DEFAULT_ZONES`: ok below −20 dBFS, warn from −20, clip from −9. These are the thresholds streamers know from OBS.
- `zoneForDb(db, zones)` returns the zone name.
- `CLIP_THRESHOLD_DB = -1` and `CLIP_HOLD_MS = 1500` for the clip light, which is separate from the clip zone colour.

### `taper.ts`

Maps a fader position to dB.

- `linearTaper(minDb, maxDb)`: equal distance per dB. The default.
- `audioTaper({ minDb, maxDb, unityPosition })`: console-style law with more travel around 0 dB and a fast fall to silence at the bottom. `unityPosition` defaults to 0.75.
- `logTaper(min, max)`: for frequency and time parameters.

### `bands.ts`

- `logBandEdges(count, minHz, maxHz)`: band boundaries on a log scale.
- `bandsFromSpectrum(spectrumDb, sampleRate, edges, out)`: FFT bins to 0..1 bands.
- `resampleLevels(source, start, length, out)`: fits any number of levels to any number of bars.

### `time.ts`

- `formatTime(seconds, options?)`: `m:ss` or `h:mm:ss`; `remaining` prefixes "−".

### `frame-loop.ts`

- `subscribeFrame(callback)`: one shared `requestAnimationFrame` loop for the whole page. It pauses when the document is hidden and stops when there are no subscribers.

## Presentation hooks

No Web Audio. Safe for any consumer.

### `use-frame-source`

```ts
useFrameSource(source, onFrame, { enabled = true });
```

Subscribes `onFrame` to a `FrameSource`, with a stable callback and automatic cleanup. Components use it internally for their `source` prop.

### `use-level`

```ts
const { peakDb, rmsDb, zone } = useLevel(source, { intervalMs: 250, channel });
```

React state sampled from a frame source at a low rate. For labels and conditional UI, not for meters.

### `use-clip-hold`

```ts
const { clipping, count, report, reset } = useClipHold({
  thresholdDb: -1,
  holdMs: 1500, // Infinity latches until reset()
});
```

### `use-mixer`

State for a mixer, with no audio attached.

```ts
interface MixerChannelState {
  id: string;
  gainDb: number;
  muted: boolean;
  solo: boolean;
  pan: number; // −1..1
}

const mixer = useMixer({
  channels: [{ id: "mic", gainDb: 0 }],
  master: { gainDb: 0 },
});

mixer.channels; // MixerChannelState[]
mixer.master; // { gainDb, muted }
mixer.setGain(id, db);
mixer.setMuted(id, muted);
mixer.setSolo(id, solo, { exclusive });
mixer.setPan(id, pan);
mixer.isAudible(id); // false when muted, or when another channel is soloed
mixer.addChannel(channel);
mixer.removeChannel(id);
mixer.reset();
```

Also accepts `state` and `onStateChange` for a fully controlled mixer, and `persistKey` to save to `localStorage`.

### `use-demo-signal`

```ts
const { meter, visual } = useDemoSignal({
  kind: "speech", // "speech" | "music" | "tone" | "noise" | "silence"
  channels: 2,
  seed: 1,
});
```

A deterministic synthetic signal as frame sources. It drives every docs preview with no microphone prompt, and is useful in tests and prototypes.

## Web Audio hooks

Optional. Each is its own registry item.

### `use-audio-context`

- One lazily created, shared `AudioContext`.
- Resumes on the first user gesture, which browsers require.
- `AudioContextProvider` lets an app supply its own context.

### `use-audio-analyser`

```ts
const analyser = useAudioAnalyser(input, options);

analyser.meter; // FrameSource<MeterFrame>
analyser.visual; // FrameSource<VisualFrame>
analyser.status; // "idle" | "running" | "suspended"
```

| Option | Default | Meaning |
| --- | --- | --- |
| `input` | — | `MediaStream`, `HTMLMediaElement`, `AudioNode` or `null` |
| `fftSize` | 2048 | Analyser FFT size |
| `smoothing` | 0.3 | Analyser smoothing constant |
| `bands` | 32 | Number of frequency bands |
| `minHz` / `maxHz` | 40 / 16000 | Band range |
| `historySize` | 60 | Length of the level history ring |
| `intervalMs` | 0 | Minimum time between frames (0 = every animation frame) |
| `channels` | `"mono"` | `"stereo"` splits left and right for stereo meters |
| `enabled` | `true` | Pauses analysis without tearing down |

It reuses its frame buffers, so steady-state analysis allocates nothing.

### `use-audio-devices`

```ts
const { devices, permission, requestPermission, refresh, isLoading, error } =
  useAudioDevices({ kind: "audioinput" });
```

- `devices`: `{ id, label, kind, isDefault }[]`.
- `permission`: `"granted" | "prompt" | "denied" | "unsupported"`.
- Listens for `devicechange` and refreshes automatically.

### `use-microphone`

```ts
const mic = useMicrophone({
  deviceId,
  enabled: true,
  echoCancellation: false,
  noiseSuppression: false,
  autoGainControl: false,
});

mic.stream; // MediaStream | null
mic.status; // "idle" | "acquiring" | "active" | "denied" | "unavailable" | "error"
mic.start();
mic.stop();
```

Browser processing is off by default, so meters show the real signal.

### `use-system-audio`

```ts
const system = useSystemAudio();

system.isSupported;
system.stream;
system.status; // "idle" | "prompting" | "active" | "denied" | "ended" | "unsupported"
system.start();
system.stop();
```

Captures system or tab audio through `getDisplayMedia`. Browser support is uneven (full system audio on Chromium for Windows and ChromeOS, tab audio on Chromium for macOS, none in Safari or Firefox), so the hook reports `isSupported` and the docs state the limits plainly.

### `use-audio-player`

```ts
const player = useAudioPlayer({ src, autoPlay, loop, volume, playbackRate });

player.ref; // attach to an <audio> element, or let <AudioPlayer> own it
player.status; // "idle" | "loading" | "ready" | "playing" | "paused" | "ended" | "error"
player.currentTime;
player.duration;
player.buffered;
player.volume;
player.muted;
player.play();
player.pause();
player.toggle();
player.seek(seconds);
player.setVolume(volume);
player.setMuted(muted);
player.setPlaybackRate(rate);
player.time; // FrameSource<number> for a smooth playhead
```

`currentTime` updates about four times a second; `time` updates every frame for components that paint a playhead.

### `use-sound`

```ts
const sound = useSound(src, {
  volume: 1,
  playbackRate: 1,
  loop: false,
  interrupt: true, // restart instead of layering
  maxVoices: 4,
});

sound.play();
sound.stop();
sound.isPlaying;
sound.isLoaded;
sound.duration;
sound.progress; // FrameSource<number>, 0..1
sound.output; // AudioNode, to route into a mixer
```

Decodes into an `AudioBuffer` for low-latency triggering.

### `use-waveform-data`

```ts
const { peaks, duration, status } = useWaveformData(src, { samples: 512 });
```

Decodes a file and reduces it to min/max peaks for the `waveform` component. Also accepts an `AudioBuffer`. Results are cached per source.

### `use-web-audio-mixer`

Binds `use-mixer` state to a real Web Audio graph.

```ts
const graph = useWebAudioMixer(mixer, {
  inputs: { mic: micStream, system: systemStream, music: player.ref },
  ducking: {
    trigger: "mic",
    targets: ["music"],
    thresholdDb: -35,
    amountDb: -12,
    attackMs: 50,
    releaseMs: 400,
  },
  limiter: true,
});

graph.meters.mic; // FrameSource<MeterFrame> per channel, post-fader
graph.visuals.mic; // FrameSource<VisualFrame> per channel
graph.master; // { meter, visual }
graph.output; // MediaStream of the mix
graph.destination; // AudioNode
```

- Per channel: source, gain, pan, analyser tap.
- Solo and mute are applied as gain ramps (5 ms) so they never click.
- The optional limiter is a `DynamicsCompressorNode` on the master.
- `monitor` controls whether the mix is also sent to the speakers.

## Registry items

| Item | Type | Depends on |
| --- | --- | --- |
| `core` | `registry:lib` | — (ships the audio tokens) |
| `use-frame-source` | `registry:hook` | `core` |
| `use-level` | `registry:hook` | `core`, `use-frame-source` |
| `use-clip-hold` | `registry:hook` | `core` |
| `use-mixer` | `registry:hook` | `core` |
| `use-demo-signal` | `registry:hook` | `core` |
| `use-audio-context` | `registry:hook` | — |
| `use-audio-analyser` | `registry:hook` | `core`, `use-audio-context` |
| `use-audio-devices` | `registry:hook` | — |
| `use-microphone` | `registry:hook` | — |
| `use-system-audio` | `registry:hook` | — |
| `use-audio-player` | `registry:hook` | `core` |
| `use-sound` | `registry:hook` | `core`, `use-audio-context` |
| `use-waveform-data` | `registry:hook` | `use-audio-context` |
| `use-web-audio-mixer` | `registry:hook` | `core`, `use-mixer`, `use-audio-context` |

## Tests

- `lib/audio`: conversions round-trip; ballistics hit their attack and release times; tapers are monotonic and invertible; band edges cover the range with no gaps; `formatDb` handles `-Infinity`, negative zero and rounding.
- Hooks: run against a mocked `AudioContext` and mocked `mediaDevices`; verify cleanup (tracks stopped, nodes disconnected) on unmount and on input change.
- `use-mixer`: solo and mute interaction, exclusive solo, controlled mode.
