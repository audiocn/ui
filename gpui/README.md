# audiocn for GPUI

Audio UI components for [GPUI](https://gpui.rs), Zed's Rust UI framework, ported one-to-one
from [audiocn](https://audiocn.dev), the React registry at the root of this repo: level meters,
visualizers, faders, knobs, channel strips, a complete mixer, players and sound pads.

Every component, every block and every docs example is ported, with the same names, props,
defaults, visuals and keyboard behaviour. The React source is the spec; differences that GPUI
can't express are listed in each module's docs.

```sh
cargo add audiocn --git https://github.com/radiumcoders/audiocn-gpui
```

```rust
use audiocn::signal::demo_signal::{use_demo_signal, DemoSignalKind, DemoSignalOptions};
use audiocn::{Fader, Knob, LevelMeter};

// inside a `render`:
let signal = use_demo_signal(
    "signal",
    DemoSignalOptions::kind(DemoSignalKind::Music).with_channels(2),
    window,
    cx,
);

div()
    .child(LevelMeter::new("master").source(signal.meter.clone()))
    .child(Fader::new("gain").value(gain_db).on_value_change(|db, _, _| println!("{db} dB")))
    .child(Knob::new("pan").min(-1.).max(1.).default_value(0.))
```

Call `audiocn::init(cx)` once at startup (it registers the text-input key bindings).

## What's in it

| | |
| --- | --- |
| **Components** (24) | LevelMeter, LcdMeter, DbScale, DbReadout, ClipIndicator, BarVisualizer, ElectricBarVisualizer, ElectricWaveform, SmoothWaveform, LiveWaveform, Waveform, Spectrum, Fader, ParameterSlider, TempoControl, Knob, LcdDisplay, PanControl, ChannelToggle, VolumeControl, AudioDeviceSelect, ChannelStrip, Mixer, AudioPlayer, TrackList, SoundPad, with all their parts |
| **Blocks** (6) | SystemAudioMixer, MicSetup, SystemAudioSettings, QuickAudioPopover, Soundboard, MusicPlayer |
| **Hooks → state** | `signal::*`: demo signal, mixer state, clip hold, tap tempo, level sampling, waveform peaks, reduced motion |
| **Audio maths** | `audio::*`: decibels, tapers, ballistics, zones, bands, history, wave lines, click synthesis, electric arcs (a port of `lib/audio`) |
| **shadcn/ui primitives** | `ui::*`: Button, Badge, Card, Alert, Empty, Field, Input, Select, Popover, DropdownMenu, ContextMenu, Tabs, Switch, Toggle, Tooltip, Kbd, Separator, Skeleton, Toast, plus Phosphor icons |
| **Native audio** (`device` feature) | `io::*`: microphone, system audio (PipeWire/Pulse monitor sources), device lists, a player engine, sounds, gain and a Web Audio–style mixer, on cpal + rustfft + symphonia |

## How it maps from React

- **Components are builders.** `<LevelMeter source={src} />` → `LevelMeter::new(id).source(src)`.
  Props are snake_case builder methods with the React defaults; callbacks take
  `(value, &mut Window, &mut App)`. Interactive components keep their state in GPUI's keyed
  element state, so give each one an id that's unique among its siblings. Controlled
  (`.value(v)`) and uncontrolled (`.default_value(v)`) both work, as in React.
- **Themed by shadcn.** `audiocn::Theme` holds shadcn's CSS variables (`background`,
  `muted_foreground`, `destructive`, ...) and audiocn's own (`meter_ok`, `channel_solo`, ...) under the
  same names. The defaults are audiocn.dev's stone theme; `Theme::apply_css` reads any shadcn
  theme's CSS, so any shadcn theme restyles audiocn. Set it with `audiocn::theme::set_theme(theme, cx)`.
- **Fast by default.** Meters and visualizers read a `FrameSource` while painting and only request
  animation frames while something moves.
- **Bring your own audio.** Components take levels, frames and callbacks; they never open a device.
  Push frames from any engine with `create_frame_emitter()`, use the demo signal, or enable the
  `device` feature for native I/O:

  ```rust
  let mic = audiocn::io::microphone::use_microphone("mic", MicrophoneOptions { enabled: true, ..Default::default() }, window, cx);
  LevelMeter::new("mic-meter").source(mic.meter.clone())
  ```

  `audiocn::io::install_audio_player_backend(cx)` makes every `AudioPlayer::new(..).src(..)` play
  for real.
- **Context → explicit config.** React's `AudioConfigProvider` becomes `AudioConfig`, passed with
  `.config(..)`. `ChannelStrip` and `Mixer` hand the right config to their children (see the docs at
  the top of `components/channel_strip.rs`).

## Gallery

`crates/gallery` is the docs site in GPUI: every page and every example, in the docs order.

```sh
cargo run -p audiocn-gallery                                   # sidebar browser
cargo run -p audiocn-gallery -- --demo knob --theme dark       # open one page
cargo run -p audiocn-gallery -- --demo knob --variant metal    # one example, centred
cargo run -p audiocn-gallery --features device                 # real microphone in the "microphone" examples
```

It also builds for the browser (GPUI's web platform):

```sh
scripts/build-wasm.sh                     # → target/web (needs wasm-bindgen-cli 0.2.129)
python3 -m http.server -d target/web      # open /?demo=knob&variant=metal&theme=dark
```

## Develop

```sh
cargo test -p audiocn                     # 400+ unit tests, ported from the React test suites
cargo clippy -p audiocn -p audiocn-gallery --all-targets -- -D warnings
cargo fmt --all
cargo build --release --target wasm32-unknown-unknown -p audiocn -p audiocn-gallery
```

Visual parity is checked headlessly, side by side with the live React site:

```sh
scripts/site-shot.sh components/knob site.png dark 0      # the React original on audiocn.dev
scripts/web-shot.sh knob port.png default dark            # this port (after build-wasm.sh)
```

`PORTING.md` has the conventions the port follows. CI (`.github/workflows/gpui.yml`) runs fmt,
clippy, tests and the wasm build when anything under `gpui/` changes.

The crate targets `gpui-pre` 0.3 (the published snapshot of Zed's GPUI) and a current stable Rust.

## Licence

MIT, like audiocn. Fonts bundled with the gallery are under the SIL Open Font License; Phosphor
icon paths are MIT.
