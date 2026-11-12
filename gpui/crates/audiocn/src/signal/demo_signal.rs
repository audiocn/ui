//! Port of `hooks/use-demo-signal.ts` (and the docs-only `lib/docs/use-demo-mixer.ts`).
//!
//! A synthetic signal as frame sources, so meters and visualizers can move
//! without a microphone. [`DemoSignal`] is plain Rust: call
//! [`DemoSignal::tick`] with the frame time and read
//! [`DemoSignal::meter_source`] / [`DemoSignal::visual_source`]. Inside a GPUI
//! render, [`use_demo_signal`] does all of that in one line.
//!
//! The generator is a bit-for-bit port of the TypeScript one: the same hash
//! noise (computed in `f64`, like JavaScript numbers), envelopes, band shapes,
//! crest factors, history ring cadence and time-domain waveforms.

use gpui::{App, ElementId, SharedString, Window};

use crate::audio::frame_source::{FrameEmitter, FrameSource};
use crate::audio::history::append_history;
use crate::audio::types::{ChannelLevel, MeterFrame, VisualFrame};
use crate::signal::mixer::{MixerState, is_channel_audible};

/// What the demo signal sounds like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DemoSignalKind {
    /// Phrases of syllables over room tone.
    #[default]
    Speech,
    /// A kick on every beat over a steady body, with the odd crash.
    Music,
    /// A steady 1 kHz sine at −12 dBFS.
    Tone,
    /// Broadband noise around −20 dBFS.
    Noise,
    /// Digital silence.
    Silence,
}

/// Options for [`DemoSignal`], the same as the hook's `DemoSignalOptions`.
///
/// ```ignore
/// DemoSignalOptions { kind: DemoSignalKind::Music, channels: 2, seed: 4.0, ..Default::default() }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DemoSignalOptions {
    /// What the signal sounds like. Default `Speech`.
    pub kind: DemoSignalKind,
    /// 1 for mono, 2 for stereo (clamped to 1..=8). Default 1.
    pub channels: usize,
    /// Changes the pattern while keeping it repeatable. Default 1.
    pub seed: f64,
    /// Frequency bands per visual frame. Default 32.
    pub bands: usize,
    /// Entries in the level history ring. Default 60.
    pub history_size: usize,
    /// Time between history entries. Default 50 ms.
    pub history_interval_ms: f64,
    /// Gain applied to the signal in dB. Default 0; `f32::NEG_INFINITY` silences it.
    pub gain_db: f32,
    /// When false the signal falls silent. Default true.
    pub playing: bool,
}

impl Default for DemoSignalOptions {
    fn default() -> Self {
        Self {
            kind: DemoSignalKind::Speech,
            channels: 1,
            seed: 1.0,
            bands: 32,
            history_size: 60,
            history_interval_ms: 50.0,
            gain_db: 0.0,
            playing: true,
        }
    }
}

impl DemoSignalOptions {
    /// Default options with this kind.
    pub fn kind(kind: DemoSignalKind) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }

    pub fn with_kind(mut self, kind: DemoSignalKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_channels(mut self, channels: usize) -> Self {
        self.channels = channels;
        self
    }

    pub fn with_seed(mut self, seed: impl Into<f64>) -> Self {
        self.seed = seed.into();
        self
    }

    pub fn with_bands(mut self, bands: usize) -> Self {
        self.bands = bands;
        self
    }

    pub fn with_history_size(mut self, history_size: usize) -> Self {
        self.history_size = history_size;
        self
    }

    pub fn with_history_interval_ms(mut self, history_interval_ms: f64) -> Self {
        self.history_interval_ms = history_interval_ms;
        self
    }

    pub fn with_gain_db(mut self, gain_db: f32) -> Self {
        self.gain_db = gain_db;
        self
    }

    pub fn with_playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }
}

const TIME_DOMAIN_SIZE: usize = 256;
const TIME_DOMAIN_RATE: f64 = 12_800.0;
const TWO_PI: f64 = std::f64::consts::PI * 2.0;
const ROOM_TONE_DB: f64 = -52.0;
const MS_PER_SECOND: f64 = 1000.0;
const MAX_CHANNELS: usize = 8;
const LEVEL_MIN_DB: f64 = -60.0;
const LEVEL_MAX_DB: f64 = 0.0;

fn crest_db(kind: DemoSignalKind) -> f64 {
    match kind {
        DemoSignalKind::Music => 8.0,
        DemoSignalKind::Noise => 11.0,
        DemoSignalKind::Silence => 0.0,
        DemoSignalKind::Speech => 12.0,
        DemoSignalKind::Tone => 3.01,
    }
}

// The generator runs in f64 like the JavaScript original: the hash below
// depends on the low bits of `sin`, so the f32 helpers in
// `crate::audio::decibels` would give a different pattern. These are the same
// formulas in f64.

fn db_to_gain(db: f64) -> f64 {
    if db == f64::NEG_INFINITY {
        0.0
    } else {
        10f64.powf(db / 20.0)
    }
}

fn gain_to_db(gain: f64) -> f64 {
    if gain <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * gain.log10()
    }
}

fn db_to_level(db: f64) -> f64 {
    // Also catches NaN, like the TS `!(db > minDb)`.
    if db.is_nan() || db <= LEVEL_MIN_DB {
        return 0.0;
    }
    ((db - LEVEL_MIN_DB) / (LEVEL_MAX_DB - LEVEL_MIN_DB)).clamp(0.0, 1.0)
}

fn fract(value: f64) -> f64 {
    value - value.floor()
}

fn hash(seed: f64, index: f64) -> f64 {
    fract((seed * 12.9898 + index * 78.233).sin() * 43_758.545_3)
}

fn smooth_noise(seed: f64, time: f64) -> f64 {
    let index = time.floor();
    let phase = time - index;
    let eased = phase * phase * (3.0 - 2.0 * phase);
    let from = hash(seed, index);
    let to = hash(seed, index + 1.0);
    from + (to - from) * eased
}

fn speech_amplitude(seed: f64, seconds: f64) -> f64 {
    let phrase_length = 2.4;
    let phrase = (seconds / phrase_length).floor();
    let phrase_time = seconds - phrase * phrase_length;
    let speaking = phrase_time < 1.5 + 0.5 * hash(seed, phrase);
    let room = db_to_gain(ROOM_TONE_DB) * (0.6 + 0.4 * smooth_noise(seed, seconds * 9.0));
    if !speaking {
        return room;
    }
    let syllable_rate = 5.0;
    let syllable = (seconds * syllable_rate).floor();
    let shape = (std::f64::consts::PI * fract(seconds * syllable_rate))
        .sin()
        .powf(1.5);
    let chance = hash(seed + 3.0, syllable);
    let mut syllable_db = -18.0 + 12.0 * hash(seed, syllable);
    if chance > 0.985 {
        syllable_db = -0.4;
    } else if chance > 0.95 {
        syllable_db = -4.0;
    }
    room + db_to_gain(syllable_db) * shape
}

fn music_amplitude(seed: f64, seconds: f64) -> f64 {
    let beat = 0.5;
    let phase = fract(seconds / beat);
    let kick = db_to_gain(-6.0) * (-phase * 8.0).exp();
    let body = db_to_gain(-16.0) * (0.8 + 0.2 * smooth_noise(seed, seconds * 2.0));
    let bar = (seconds / (beat * 4.0)).floor();
    let bar_phase = fract(seconds / (beat * 4.0));
    let crash = if hash(seed + 5.0, bar) > 0.85 {
        db_to_gain(-3.0) * (-bar_phase * 3.0).exp()
    } else {
        0.0
    };
    body + kick + crash
}

fn amplitude_for(kind: DemoSignalKind, seed: f64, seconds: f64) -> f64 {
    match kind {
        DemoSignalKind::Speech => speech_amplitude(seed, seconds),
        DemoSignalKind::Music => music_amplitude(seed, seconds),
        DemoSignalKind::Tone => db_to_gain(-12.0),
        DemoSignalKind::Noise => {
            db_to_gain(-20.0) * (0.7 + 0.3 * smooth_noise(seed, seconds * 8.0))
        }
        DemoSignalKind::Silence => 0.0,
    }
}

fn gaussian(x: f64, center: f64, width: f64) -> f64 {
    (-((x - center) / width).powi(2)).exp()
}

fn band_template(kind: DemoSignalKind, x: f64) -> f64 {
    match kind {
        DemoSignalKind::Speech => 0.25 + gaussian(x, 0.4, 0.16) + 0.6 * gaussian(x, 0.66, 0.1),
        DemoSignalKind::Music => 1.05 - 0.55 * x + 0.3 * gaussian(x, 0.85, 0.08),
        DemoSignalKind::Tone => 0.05 + gaussian(x, 0.54, 0.03),
        DemoSignalKind::Noise => 0.85 - 0.2 * x,
        DemoSignalKind::Silence => 0.0,
    }
}

fn sample_wave(kind: DemoSignalKind, seed: f64, time: f64) -> f64 {
    match kind {
        DemoSignalKind::Speech => {
            0.6 * (TWO_PI * 140.0 * time).sin()
                + 0.3 * (TWO_PI * 420.0 * time).sin()
                + 0.1 * (TWO_PI * 1100.0 * time).sin()
        }
        DemoSignalKind::Music => {
            0.5 * (TWO_PI * 55.0 * time).sin()
                + 0.3 * (TWO_PI * 220.0 * time).sin()
                + 0.2 * (TWO_PI * 880.0 * time).sin()
        }
        DemoSignalKind::Tone => (TWO_PI * 1000.0 * time).sin(),
        DemoSignalKind::Noise => hash(seed, time * TIME_DOMAIN_RATE) * 2.0 - 1.0,
        DemoSignalKind::Silence => 0.0,
    }
}

/// A synthetic signal for previews, prototypes and tests (`createDemoSignal`).
///
/// Owns a [`FrameEmitter<MeterFrame>`] and a [`FrameEmitter<VisualFrame>`];
/// every [`tick`](Self::tick) produces one frame on each.
///
/// ```ignore
/// let mut signal = DemoSignal::new(DemoSignalOptions::kind(DemoSignalKind::Music));
/// let meter = signal.meter_source();
/// signal.tick(now_ms); // once per animation frame
/// LevelMeter::new("demo").source(meter)
/// ```
pub struct DemoSignal {
    options: DemoSignalOptions,
    start_ms: Option<f64>,
    visual_frame: VisualFrame,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
}

impl DemoSignal {
    pub fn new(options: DemoSignalOptions) -> Self {
        let defaults = DemoSignalOptions::default();
        let mut signal = Self {
            visual_frame: VisualFrame {
                bands: vec![0.0; defaults.bands],
                history: vec![0.0; defaults.history_size],
                history_start: 0,
                history_length: 0,
                history_updated_at: None,
                history_interval_ms: None,
                history_previous_level: None,
                time_domain: Some(vec![0.0; TIME_DOMAIN_SIZE]),
                peak_db: f32::NEG_INFINITY,
            },
            options: defaults,
            start_ms: None,
            meter: FrameEmitter::new(),
            visual: FrameEmitter::new(),
        };
        signal.configure(&options);
        signal
    }

    /// Changes options without replacing the sources or restarting the
    /// pattern. A new `history_size` clears the history ring (but keeps its
    /// sample clock); a new `bands` count resizes the bands.
    pub fn configure(&mut self, options: &DemoSignalOptions) {
        let previous_bands = self.visual_frame.bands.len();
        let previous_history = self.visual_frame.history.len();
        self.options = DemoSignalOptions {
            channels: options.channels.clamp(1, MAX_CHANNELS),
            ..options.clone()
        };
        if options.bands != previous_bands {
            self.visual_frame.bands = vec![0.0; options.bands];
        }
        if options.history_size != previous_history {
            self.visual_frame.history = vec![0.0; options.history_size];
            self.visual_frame.history_start = 0;
            self.visual_frame.history_length = 0;
            self.visual_frame.history_previous_level = None;
        }
    }

    /// The options in effect (channels already clamped).
    pub fn options(&self) -> &DemoSignalOptions {
        &self.options
    }

    /// Meter frames: one [`ChannelLevel`] per channel with peak and RMS.
    pub fn meter_source(&self) -> FrameSource<MeterFrame> {
        self.meter.source()
    }

    /// Visual frames: bands, a history ring, time-domain samples and peak.
    pub fn visual_source(&self) -> FrameSource<VisualFrame> {
        self.visual.source()
    }

    /// The last visual frame produced (the same one the visual source holds).
    pub fn visual_frame(&self) -> &VisualFrame {
        &self.visual_frame
    }

    /// Produces and emits one meter frame and one visual frame for `now_ms`
    /// (a monotonic frame clock in milliseconds, such as
    /// [`now_ms`]). The first tick sets the signal's time zero.
    pub fn tick(&mut self, now_ms: f64) {
        let meter_frame = self.produce(now_ms);
        self.meter.emit(meter_frame);
        self.visual.emit(self.visual_frame.clone());
    }

    fn produce(&mut self, now_ms: f64) -> MeterFrame {
        let start_ms = *self.start_ms.get_or_insert(now_ms);
        let seconds = (now_ms - start_ms) / MS_PER_SECOND;
        let options = &self.options;
        let seed = options.seed;
        let active_kind = if options.playing {
            options.kind
        } else {
            DemoSignalKind::Silence
        };
        let input_amplitude = amplitude_for(active_kind, seed, seconds).min(1.0);
        let gain = db_to_gain(options.gain_db as f64);
        let amplitude = input_amplitude * gain;
        let crest_gain = db_to_gain(-crest_db(active_kind));

        let mut channels = Vec::with_capacity(options.channels);
        let mut loudest: f64 = 0.0;
        for channel in 0..options.channels {
            let wobble = if channel == 0 {
                1.0
            } else {
                1.0 + 0.3 * (smooth_noise(seed + 17.0 * channel as f64, seconds * 3.0) - 0.5)
            };
            let channel_amplitude = (input_amplitude * wobble).min(1.0) * gain;
            loudest = loudest.max(channel_amplitude);
            channels.push(ChannelLevel {
                peak_db: gain_to_db(channel_amplitude) as f32,
                rms_db: Some(gain_to_db(channel_amplitude * crest_gain) as f32),
            });
        }

        let frame = &mut self.visual_frame;
        let level = db_to_level(gain_to_db(loudest));
        let count = frame.bands.len();
        for (band, value) in frame.bands.iter_mut().enumerate() {
            let x = if count > 1 {
                band as f64 / (count - 1) as f64
            } else {
                0.0
            };
            let movement = 0.7 + 0.6 * smooth_noise(seed + band as f64 * 13.0, seconds * 6.0);
            *value = (band_template(active_kind, x) * level * movement).clamp(0.0, 1.0) as f32;
        }

        append_history(frame, level as f32, now_ms, options.history_interval_ms);

        let time_domain = frame
            .time_domain
            .get_or_insert_with(|| vec![0.0; TIME_DOMAIN_SIZE]);
        for (index, sample) in time_domain.iter_mut().enumerate() {
            let time = seconds + index as f64 / TIME_DOMAIN_RATE;
            *sample = (amplitude * sample_wave(active_kind, seed, time)) as f32;
        }

        frame.peak_db = gain_to_db(loudest) as f32;

        MeterFrame { channels }
    }
}

impl Default for DemoSignal {
    fn default() -> Self {
        Self::new(DemoSignalOptions::default())
    }
}

/// The frame clock to tick demo signals with (re-exported from
/// [`crate::audio::frame_loop`]). History timestamps are on this clock.
pub use crate::audio::frame_loop::now_ms;

/// Cloneable read side of a demo signal driven by [`use_demo_signal`]:
/// the hook's `signal.meter` and `signal.visual`.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoSignalHandle {
    pub meter: FrameSource<MeterFrame>,
    pub visual: FrameSource<VisualFrame>,
}

impl DemoSignalHandle {
    pub fn meter_source(&self) -> FrameSource<MeterFrame> {
        self.meter.clone()
    }

    pub fn visual_source(&self) -> FrameSource<VisualFrame> {
        self.visual.clone()
    }
}

/// `useDemoSignal` for GPUI: keeps a [`DemoSignal`] in keyed element state,
/// applies `options` (without resetting the pattern), ticks it and requests
/// the next animation frame, so it runs for as long as it is rendered.
///
/// Call it from `Render::render` / `RenderOnce::render`; `id` must be unique
/// among its siblings.
///
/// ```ignore
/// let signal = use_demo_signal("voice", DemoSignalOptions::kind(DemoSignalKind::Speech), window, cx);
/// div().child(LevelMeter::new("meter").source(signal.meter))
/// ```
pub fn use_demo_signal(
    id: impl Into<ElementId>,
    options: DemoSignalOptions,
    window: &mut Window,
    cx: &mut App,
) -> DemoSignalHandle {
    let state = window.use_keyed_state(id, cx, |_, _| DemoSignal::new(options.clone()));
    let now = now_ms();
    let handle = state.update(cx, |signal, _| {
        if signal.options != options {
            signal.configure(&options);
        }
        signal.tick(now);
        DemoSignalHandle {
            meter: signal.meter_source(),
            visual: signal.visual_source(),
        }
    });
    window.request_animation_frame();
    handle
}

// ---------------------------------------------------------------------------
// Demo mixer (`lib/docs/use-demo-mixer.ts`), used by the mixer docs examples.
// ---------------------------------------------------------------------------

/// One input of a [`DemoMixer`]: an id and the options of its demo signal.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoMixerChannel {
    pub id: SharedString,
    pub options: DemoSignalOptions,
}

impl DemoMixerChannel {
    pub fn new(id: impl Into<SharedString>, options: DemoSignalOptions) -> Self {
        Self {
            id: id.into(),
            options,
        }
    }
}

/// Docs-only metering (`createDemoMixer`): one demo signal per channel, and a
/// stereo master that sums peak amplitudes and RMS powers (mono inputs feed
/// both sides). Gain, mute and solo follow a [`MixerState`].
pub struct DemoMixer {
    signals: Vec<(SharedString, DemoSignal)>,
    master_gain: f64,
    master: FrameEmitter<MeterFrame>,
}

impl DemoMixer {
    pub fn new(channels: &[DemoMixerChannel]) -> Self {
        Self {
            signals: channels
                .iter()
                .map(|channel| (channel.id.clone(), DemoSignal::new(channel.options.clone())))
                .collect(),
            master_gain: 1.0,
            master: FrameEmitter::new(),
        }
    }

    /// Follows mixer state: each signal takes its channel's gain and plays
    /// only while audible; the master follows master gain and mute.
    pub fn configure(&mut self, state: &MixerState) {
        for (id, signal) in &mut self.signals {
            let channel = state.channel(id);
            let options = DemoSignalOptions {
                gain_db: channel.map_or(0.0, |channel| channel.gain_db),
                playing: is_channel_audible(state, id),
                ..signal.options().clone()
            };
            signal.configure(&options);
        }
        self.master_gain = if state.master.muted {
            0.0
        } else {
            db_to_gain(state.master.gain_db as f64)
        };
    }

    /// The meter source of one channel.
    pub fn source(&self, id: &str) -> Option<FrameSource<MeterFrame>> {
        self.signals
            .iter()
            .find(|(channel, _)| channel.as_ref() == id)
            .map(|(_, signal)| signal.meter_source())
    }

    /// Every channel's meter source, in channel order.
    pub fn sources(&self) -> Vec<(SharedString, FrameSource<MeterFrame>)> {
        self.signals
            .iter()
            .map(|(id, signal)| (id.clone(), signal.meter_source()))
            .collect()
    }

    /// The stereo master meter.
    pub fn master_source(&self) -> FrameSource<MeterFrame> {
        self.master.source()
    }

    /// Ticks every input, then mixes them into the master frame.
    pub fn tick(&mut self, now_ms: f64) {
        for (_, signal) in &mut self.signals {
            signal.tick(now_ms);
        }
        let mut channels = Vec::with_capacity(2);
        for side in 0..2 {
            let mut peak = 0.0;
            let mut power = 0.0;
            for (_, signal) in &self.signals {
                signal.meter_source().with_latest(|frame| {
                    let Some(input) = frame else { return };
                    let index = if input.channels.len() == 1 { 0 } else { side };
                    if let Some(level) = input.channels.get(index) {
                        peak += db_to_gain(level.peak_db as f64);
                        power += db_to_gain(level.rms_db.unwrap_or(level.peak_db) as f64).powi(2);
                    }
                });
            }
            channels.push(ChannelLevel {
                peak_db: gain_to_db(peak * self.master_gain) as f32,
                rms_db: Some(gain_to_db(f64::sqrt(power) * self.master_gain) as f32),
            });
        }
        self.master.emit(MeterFrame { channels });
    }
}

/// Cloneable read side of [`use_demo_mixer`].
#[derive(Clone, Debug)]
pub struct DemoMixerHandle {
    /// Per-channel meter sources, in channel order.
    pub sources: Vec<(SharedString, FrameSource<MeterFrame>)>,
    /// The stereo master meter.
    pub master: FrameSource<MeterFrame>,
}

impl DemoMixerHandle {
    pub fn source(&self, id: &str) -> Option<FrameSource<MeterFrame>> {
        self.sources
            .iter()
            .find(|(channel, _)| channel.as_ref() == id)
            .map(|(_, source)| source.clone())
    }
}

/// `useDemoMixer` for GPUI. Keep `channels` stable (they are read once);
/// gain, mute and solo follow `state` on every render.
pub fn use_demo_mixer(
    id: impl Into<ElementId>,
    channels: &[DemoMixerChannel],
    state: &MixerState,
    window: &mut Window,
    cx: &mut App,
) -> DemoMixerHandle {
    let entity = window.use_keyed_state(id, cx, |_, _| DemoMixer::new(channels));
    let now = now_ms();
    let handle = entity.update(cx, |mixer, _| {
        mixer.configure(state);
        mixer.tick(now);
        DemoMixerHandle {
            sources: mixer.sources(),
            master: mixer.master_source(),
        }
    });
    window.request_animation_frame();
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Like the TS `useFakeFrames` + `advance(ms)`: animation frames every
    /// 16 ms of fake time.
    struct FakeFrames {
        now: f64,
    }

    impl FakeFrames {
        fn new() -> Self {
            Self { now: 0.0 }
        }

        fn advance(&mut self, ms: f64, signals: &mut [&mut DemoSignal]) {
            let end = self.now + ms;
            let mut next = (self.now / 16.0).floor() * 16.0 + 16.0;
            while next <= end {
                for signal in signals.iter_mut() {
                    signal.tick(next);
                }
                next += 16.0;
            }
            self.now = end;
        }
    }

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.005, "{a} is not close to {b}");
    }

    fn meter(signal: &DemoSignal) -> MeterFrame {
        signal
            .meter_source()
            .latest()
            .expect("Expected a signal frame")
    }

    fn visual(signal: &DemoSignal) -> VisualFrame {
        signal
            .visual_source()
            .latest()
            .expect("Expected a signal frame")
    }

    #[test]
    fn applies_gain_to_stereo_peak_rms_bands_history_and_waveform_without_clipping_boosts() {
        let tone = DemoSignalOptions {
            channels: 2,
            kind: DemoSignalKind::Tone,
            ..Default::default()
        };
        let mut dry = DemoSignal::new(tone.clone());
        let mut wet = DemoSignal::new(DemoSignalOptions {
            gain_db: -6.0,
            ..tone
        });
        let mut frames = FakeFrames::new();
        frames.advance(100.0, &mut [&mut dry, &mut wet]);
        for side in 0..2 {
            let input = meter(&dry).channels[side];
            let output = meter(&wet).channels[side];
            close(output.peak_db, input.peak_db - 6.0);
            close(output.rms_db.unwrap(), input.rms_db.unwrap() - 6.0);
        }
        let input = visual(&dry);
        let output = visual(&wet);
        close(output.peak_db, input.peak_db - 6.0);
        assert!(output.bands[0] < input.bands[0]);
        assert!(output.history[0] < input.history[0]);
        close(
            output.time_domain.as_ref().unwrap()[1],
            input.time_domain.as_ref().unwrap()[1] * 10f32.powf(-6.0 / 20.0),
        );

        let options = DemoSignalOptions {
            gain_db: 18.0,
            ..wet.options().clone()
        };
        wet.configure(&options);
        frames.advance(16.0, &mut [&mut dry, &mut wet]);
        close(meter(&wet).channels[0].peak_db, 6.0);
        close(visual(&wet).peak_db, visual(&dry).peak_db + 18.0);

        let options = DemoSignalOptions {
            gain_db: f32::NEG_INFINITY,
            ..wet.options().clone()
        };
        wet.configure(&options);
        frames.advance(64.0, &mut [&mut dry, &mut wet]);
        let silent = ChannelLevel {
            peak_db: f32::NEG_INFINITY,
            rms_db: Some(f32::NEG_INFINITY),
        };
        assert_eq!(meter(&wet).channels, vec![silent, silent]);
        assert!(visual(&wet).bands.iter().all(|value| *value == 0.0));
        assert!(
            visual(&wet)
                .time_domain
                .unwrap()
                .iter()
                .all(|value| *value == 0.0)
        );
    }

    #[test]
    fn updates_gain_without_resetting_the_signal_pattern() {
        let music = DemoSignalOptions {
            kind: DemoSignalKind::Music,
            seed: 4.0,
            ..Default::default()
        };
        let mut reference = DemoSignal::new(music.clone());
        let mut signal = DemoSignal::new(music.clone());
        let source = signal.meter_source();
        let mut frames = FakeFrames::new();
        frames.advance(320.0, &mut [&mut reference, &mut signal]);
        signal.configure(&DemoSignalOptions {
            gain_db: -12.0,
            ..music
        });
        frames.advance(16.0, &mut [&mut reference, &mut signal]);
        assert_eq!(signal.meter_source(), source);
        close(
            meter(&signal).channels[0].peak_db,
            meter(&reference).channels[0].peak_db - 12.0,
        );
    }

    #[test]
    fn emits_stereo_tone_frames() {
        let mut signal = DemoSignal::new(DemoSignalOptions {
            channels: 2,
            kind: DemoSignalKind::Tone,
            ..Default::default()
        });
        assert!(signal.meter_source().latest().is_none());
        FakeFrames::new().advance(100.0, &mut [&mut signal]);
        let frame = meter(&signal);
        assert_eq!(frame.channels.len(), 2);
        assert!((frame.channels[0].peak_db - -12.0).abs() < 0.5);
    }

    #[test]
    fn sends_history_timing_and_keeps_the_sample_schedule() {
        let mut signal = DemoSignal::default();
        let source = signal.visual_source();
        let mut frames: Vec<(usize, Option<f64>, Option<f64>)> = Vec::new();
        let mut now = 16.0;
        while now <= 1024.0 {
            signal.tick(now);
            let frame = source.latest().unwrap();
            frames.push((
                frame.history_length,
                frame.history_interval_ms,
                frame.history_updated_at,
            ));
            now += 16.0;
        }
        let samples: Vec<_> = frames.iter().filter(|frame| frame.0 > 0).collect();
        assert_eq!(frames[0].0, 0);
        assert_eq!(*samples[0], (1, Some(50.0), Some(64.0)));
        assert_eq!(**samples.last().unwrap(), (16, Some(50.0), Some(1024.0)));
        let mut times: Vec<_> = samples
            .iter()
            .map(|frame| frame.2.unwrap() as i64)
            .collect();
        times.dedup();
        assert_eq!(times.len(), 16);
        assert!(frames.len() > 16);
    }

    #[test]
    fn skips_missed_samples_when_frames_are_throttled_and_resumes_its_cadence() {
        let mut signal = DemoSignal::default();
        for now in [16.0, 80.0, 1017.0, 2018.0, 100_019.0] {
            signal.tick(now);
        }
        let frame = visual(&signal);
        assert_eq!(frame.history_length, 4);
        assert_eq!(frame.history_updated_at, Some(100_019.0));
        signal.tick(100_035.0);
        assert_eq!(visual(&signal).history_length, 4);
        signal.tick(100_083.0);
        let frame = visual(&signal);
        assert_eq!(frame.history_length, 5);
        assert_eq!(frame.history_updated_at, Some(100_083.0));
    }

    #[test]
    fn keeps_the_sample_clock_when_the_history_size_changes() {
        let mut signal = DemoSignal::new(DemoSignalOptions {
            history_size: 2,
            ..Default::default()
        });
        let mut frames = FakeFrames::new();
        frames.advance(208.0, &mut [&mut signal]);
        assert!(visual(&signal).history_previous_level.is_some());
        signal.configure(&DemoSignalOptions {
            history_interval_ms: 100.0,
            history_size: 4,
            ..Default::default()
        });
        frames.advance(16.0, &mut [&mut signal]);
        assert_eq!(visual(&signal).history_length, 0);
        frames.advance(80.0, &mut [&mut signal]);
        let frame = visual(&signal);
        assert_eq!(frame.history_interval_ms, Some(100.0));
        assert_eq!(frame.history_length, 1);
        assert_eq!(frame.history_previous_level, None);
        assert_eq!(frame.history_start, 0);
        assert_eq!(frame.history_updated_at, Some(304.0));
    }

    struct Reference {
        t: f64,
        meter: Vec<(f64, f64)>,
        peak: f64,
        bands: Vec<f64>,
        td: [f64; 3],
        history_length: usize,
        h0: f64,
    }

    /// Frames produced by the real `hooks/use-demo-signal.ts` under Node
    /// (ticks at 1000, 1016, 1500, 2733.5, 3901 and 7777 ms).
    #[test]
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)]
    fn frames_match_the_typescript_generator() {
        // (options, [(now_ms, [(peak, rms)], visual peak, bands[..8], time_domain[0,37,255], history_length, history[0])])
        let cases: Vec<(DemoSignalOptions, Vec<Reference>)> = vec![
            (
                DemoSignalOptions {
                    kind: DemoSignalKind::Speech,
                    seed: 1.0,
                    channels: 1,
                    bands: 32,
                    gain_db: 0.0,
                    ..Default::default()
                },
                vec![
                    Reference {
                        t: 1000.0,
                        meter: vec![(-52.27642780679241_f64, -64.27642780679241_f64)],
                        peak: -52.27642780679241_f64,
                        bands: vec![
                            0.04063531383872032_f64,
                            0.03906147554516792_f64,
                            0.02364373207092285_f64,
                            0.037800345569849014_f64,
                            0.04077108949422836_f64,
                            0.04877953603863716_f64,
                            0.041074734181165695_f64,
                            0.07280323654413223_f64,
                        ],
                        td: [
                            0.0_f64,
                            0.001754357828758657_f64,
                            -0.0010003966744989157_f64,
                        ],
                        history_length: 1,
                        h0: 0.12872619926929474_f64,
                    },
                    Reference {
                        t: 1016.0,
                        meter: vec![(-24.700661912740244_f64, -36.700661912740244_f64)],
                        peak: -24.700661912740244_f64,
                        bands: vec![
                            0.18529920279979706_f64,
                            0.17860114574432373_f64,
                            0.10843011736869812_f64,
                            0.17323088645935059_f64,
                            0.18616148829460144_f64,
                            0.22125181555747986_f64,
                            0.18862579762935638_f64,
                            0.3347887396812439_f64,
                        ],
                        td: [
                            0.014280891977250576_f64,
                            -0.04030172526836395_f64,
                            0.014922705478966236_f64,
                        ],
                        history_length: 1,
                        h0: 0.12872619926929474_f64,
                    },
                    Reference {
                        t: 1500.0,
                        meter: vec![(-16.989439081141956_f64, -28.98943908114196_f64)],
                        peak: -16.989439081141956_f64,
                        bands: vec![
                            0.17191512882709503_f64,
                            0.13493111729621887_f64,
                            0.13977594673633575_f64,
                            0.1832716315984726_f64,
                            0.23198679089546204_f64,
                            0.232161283493042_f64,
                            0.3111797273159027_f64,
                            0.42011258006095886_f64,
                        ],
                        td: [
                            -2.326639205122295e-15_f64,
                            0.10196886956691742_f64,
                            -0.058146245777606964_f64,
                        ],
                        history_length: 2,
                        h0: 0.12872619926929474_f64,
                    },
                    Reference {
                        t: 2733.5,
                        meter: vec![(-8.175630993157771_f64, -20.175630993157768_f64)],
                        peak: -8.175630993157771_f64,
                        bands: vec![
                            0.21793736517429352_f64,
                            0.21491771936416626_f64,
                            0.22537238895893097_f64,
                            0.2551698386669159_f64,
                            0.29505205154418945_f64,
                            0.2875984013080597_f64,
                            0.37098798155784607_f64,
                            0.5564001202583313_f64,
                        ],
                        td: [
                            -0.19937379658222198_f64,
                            0.2528000473976135_f64,
                            0.03684626892209053_f64,
                        ],
                        history_length: 3,
                        h0: 0.12872619926929474_f64,
                    },
                    Reference {
                        t: 3901.0,
                        meter: vec![(-11.88205356165592_f64, -23.882053561655923_f64)],
                        peak: -11.88205356165592_f64,
                        bands: vec![
                            0.2089555263519287_f64,
                            0.23861631751060486_f64,
                            0.19741332530975342_f64,
                            0.1806541383266449_f64,
                            0.25251999497413635_f64,
                            0.27852359414100647_f64,
                            0.31038787961006165_f64,
                            0.36791184544563293_f64,
                        ],
                        td: [
                            0.16948014497756958_f64,
                            -0.07431382685899734_f64,
                            -0.13792529702186584_f64,
                        ],
                        history_length: 4,
                        h0: 0.12872619926929474_f64,
                    },
                    Reference {
                        t: 7777.0,
                        meter: vec![(-54.43554215907261_f64, -66.43554215907261_f64)],
                        peak: -54.43554215907261_f64,
                        bands: vec![
                            0.020992090925574303_f64,
                            0.02123904787003994_f64,
                            0.02200726419687271_f64,
                            0.030120443552732468_f64,
                            0.033246953040361404_f64,
                            0.040907129645347595_f64,
                            0.04440990462899208_f64,
                            0.044375862926244736_f64,
                        ],
                        td: [
                            -0.000818240805529058_f64,
                            0.0007242098799906671_f64,
                            -0.001152204000391066_f64,
                        ],
                        history_length: 5,
                        h0: 0.12872619926929474_f64,
                    },
                ],
            ),
            (
                DemoSignalOptions {
                    kind: DemoSignalKind::Music,
                    seed: 4.0,
                    channels: 2,
                    bands: 32,
                    gain_db: 0.0,
                    ..Default::default()
                },
                vec![
                    Reference {
                        t: 1000.0,
                        meter: vec![
                            (-3.8787725071023544_f64, -11.878772507102354_f64),
                            (-3.733983371938267_f64, -11.733983371938269_f64),
                        ],
                        peak: -3.733983371938267_f64,
                        bands: vec![
                            0.9100580215454102_f64,
                            0.9297975897789001_f64,
                            1.0_f64,
                            0.7751427888870239_f64,
                            0.9041516184806824_f64,
                            0.8085309267044067_f64,
                            0.6669123768806458_f64,
                            0.9989981055259705_f64,
                        ],
                        td: [0.0_f64, 0.08954968303442001_f64, 0.28492411971092224_f64],
                        history_length: 1,
                        h0: 0.937766969203949_f64,
                    },
                    Reference {
                        t: 1016.0,
                        meter: vec![
                            (-5.5700789089890375_f64, -13.57007890898904_f64),
                            (-5.420217113939707_f64, -13.420217113939707_f64),
                        ],
                        peak: -5.420217113939707_f64,
                        bands: vec![
                            0.8812817335128784_f64,
                            0.8990691900253296_f64,
                            1.0_f64,
                            0.7500053644180298_f64,
                            0.8745532631874084_f64,
                            0.7852743864059448_f64,
                            0.6507904529571533_f64,
                            0.9702445864677429_f64,
                        ],
                        td: [
                            -0.14930832386016846_f64,
                            0.12115323543548584_f64,
                            -0.19840097427368164_f64,
                        ],
                        history_length: 1,
                        h0: 0.937766969203949_f64,
                    },
                    Reference {
                        t: 1500.0,
                        meter: vec![
                            (-3.922487069570821_f64, -11.922487069570824_f64),
                            (-3.478290507551977_f64, -11.47829050755198_f64),
                        ],
                        peak: -3.478290507551977_f64,
                        bands: vec![
                            1.0_f64,
                            0.9378122091293335_f64,
                            1.0_f64,
                            0.6879167556762695_f64,
                            0.7086851000785828_f64,
                            1.0_f64,
                            1.0_f64,
                            1.0_f64,
                        ],
                        td: [
                            9.738957822700081e-15_f64,
                            -0.4462224841117859_f64,
                            -0.07665859907865524_f64,
                        ],
                        history_length: 2,
                        h0: 0.937766969203949_f64,
                    },
                    Reference {
                        t: 2733.5,
                        meter: vec![
                            (-16.41862320665267_f64, -24.418623206652672_f64),
                            (-16.044904194084822_f64, -24.044904194084822_f64),
                        ],
                        peak: -16.044904194084822_f64,
                        bands: vec![
                            0.7547088265419006_f64,
                            0.8704516291618347_f64,
                            0.7835811376571655_f64,
                            0.7292121052742004_f64,
                            0.7668684720993042_f64,
                            0.6888073682785034_f64,
                            0.5500398278236389_f64,
                            0.7200501561164856_f64,
                        ],
                        td: [
                            0.09993194043636322_f64,
                            0.0054765064269304276_f64,
                            -0.014578917063772678_f64,
                        ],
                        history_length: 3,
                        h0: 0.937766969203949_f64,
                    },
                    Reference {
                        t: 3901.0,
                        meter: vec![
                            (-16.382245656306175_f64, -24.38224565630618_f64),
                            (-17.638697229166883_f64, -25.638697229166887_f64),
                        ],
                        peak: -16.382245656306175_f64,
                        bands: vec![
                            0.9184344410896301_f64,
                            0.7470436692237854_f64,
                            0.6252833008766174_f64,
                            0.8830857276916504_f64,
                            0.870032012462616_f64,
                            0.7295305132865906_f64,
                            0.6275302171707153_f64,
                            0.6936652064323425_f64,
                        ],
                        td: [
                            -0.0017581414431333542_f64,
                            -0.09570369124412537_f64,
                            -0.07288511842489243_f64,
                        ],
                        history_length: 4,
                        h0: 0.937766969203949_f64,
                    },
                    Reference {
                        t: 7777.0,
                        meter: vec![
                            (-16.529548360287915_f64, -24.529548360287915_f64),
                            (-16.570517590186267_f64, -24.57051759018627_f64),
                        ],
                        peak: -16.529548360287915_f64,
                        bands: vec![
                            0.5811715722084045_f64,
                            0.5422276854515076_f64,
                            0.7240403890609741_f64,
                            0.6986393332481384_f64,
                            0.7548069953918457_f64,
                            0.7822083234786987_f64,
                            0.7318120002746582_f64,
                            0.6023181676864624_f64,
                        ],
                        td: [
                            -0.12045897543430328_f64,
                            -0.03847704082727432_f64,
                            0.003785104490816593_f64,
                        ],
                        history_length: 5,
                        h0: 0.937766969203949_f64,
                    },
                ],
            ),
            (
                DemoSignalOptions {
                    kind: DemoSignalKind::Noise,
                    seed: 3.0,
                    channels: 1,
                    bands: 8,
                    gain_db: 0.0,
                    ..Default::default()
                },
                vec![
                    Reference {
                        t: 1000.0,
                        meter: vec![(-21.234942988419174_f64, -32.23494298841917_f64)],
                        peak: -21.234942988419174_f64,
                        bands: vec![
                            0.5683561563491821_f64,
                            0.6666419506072998_f64,
                            0.4877238869667053_f64,
                            0.4798923432826996_f64,
                            0.5816845297813416_f64,
                            0.42619097232818604_f64,
                            0.4203866720199585_f64,
                            0.49074456095695496_f64,
                        ],
                        td: [
                            0.010101232677698135_f64,
                            0.07438281178474426_f64,
                            0.002912793541327119_f64,
                        ],
                        history_length: 1,
                        h0: 0.6460843086242676_f64,
                    },
                    Reference {
                        t: 1016.0,
                        meter: vec![(-21.248928061938024_f64, -32.24892806193802_f64)],
                        peak: -21.248928061938024_f64,
                        bands: vec![
                            0.5672690868377686_f64,
                            0.6622458696365356_f64,
                            0.48562222719192505_f64,
                            0.4811505675315857_f64,
                            0.5817514657974243_f64,
                            0.4270545542240143_f64,
                            0.4209100902080536_f64,
                            0.4897969365119934_f64,
                        ],
                        td: [
                            0.02832447551190853_f64,
                            0.07912096381187439_f64,
                            -0.026074768975377083_f64,
                        ],
                        history_length: 1,
                        h0: 0.6460843086242676_f64,
                    },
                    Reference {
                        t: 1500.0,
                        meter: vec![(-22.971168872993097_f64, -33.97116887299309_f64)],
                        peak: -22.971168872993097_f64,
                        bands: vec![
                            0.6515094041824341_f64,
                            0.5039805173873901_f64,
                            0.4560600221157074_f64,
                            0.3531297743320465_f64,
                            0.5122508406639099_f64,
                            0.5076154470443726_f64,
                            0.4070662260055542_f64,
                            0.29264384508132935_f64,
                        ],
                        td: [
                            -0.06574732810258865_f64,
                            0.06958989799022675_f64,
                            0.010157471522688866_f64,
                        ],
                        history_length: 2,
                        h0: 0.6460843086242676_f64,
                    },
                    Reference {
                        t: 2733.5,
                        meter: vec![(-20.745844496441023_f64, -31.745844496441023_f64)],
                        peak: -20.745844496441023_f64,
                        bands: vec![
                            0.6511350870132446_f64,
                            0.5894715785980225_f64,
                            0.46912652254104614_f64,
                            0.5301810503005981_f64,
                            0.3690437376499176_f64,
                            0.5722448229789734_f64,
                            0.4337617754936218_f64,
                            0.397136390209198_f64,
                        ],
                        td: [
                            0.00018522031314205378_f64,
                            -0.06643550843000412_f64,
                            -0.057642366737127304_f64,
                        ],
                        history_length: 3,
                        h0: 0.6460843086242676_f64,
                    },
                    Reference {
                        t: 3901.0,
                        meter: vec![(-20.350730799890407_f64, -31.35073079989041_f64)],
                        peak: -20.350730799890407_f64,
                        bands: vec![
                            0.629579484462738_f64,
                            0.49806758761405945_f64,
                            0.5524991154670715_f64,
                            0.5557332634925842_f64,
                            0.41339969635009766_f64,
                            0.46615785360336304_f64,
                            0.3804898262023926_f64,
                            0.3987561762332916_f64,
                        ],
                        td: [
                            -0.08370368182659149_f64,
                            -0.019703537225723267_f64,
                            0.07945617288351059_f64,
                        ],
                        history_length: 4,
                        h0: 0.6460843086242676_f64,
                    },
                    Reference {
                        t: 7777.0,
                        meter: vec![(-21.008392400505226_f64, -32.00839240050523_f64)],
                        peak: -21.008392400505226_f64,
                        bands: vec![
                            0.603145956993103_f64,
                            0.41222628951072693_f64,
                            0.4639596939086914_f64,
                            0.5426056385040283_f64,
                            0.5421789288520813_f64,
                            0.35628050565719604_f64,
                            0.45116162300109863_f64,
                            0.384085088968277_f64,
                        ],
                        td: [
                            -0.03169378265738487_f64,
                            0.0030605061911046505_f64,
                            -0.07393000274896622_f64,
                        ],
                        history_length: 5,
                        h0: 0.6460843086242676_f64,
                    },
                ],
            ),
            (
                DemoSignalOptions {
                    kind: DemoSignalKind::Speech,
                    seed: 7.0,
                    channels: 2,
                    bands: 32,
                    gain_db: -6.0,
                    ..Default::default()
                },
                vec![
                    Reference {
                        t: 1000.0,
                        meter: vec![
                            (-61.52748954946662_f64, -73.52748954946662_f64),
                            (-62.53257597640701_f64, -74.53257597640702_f64),
                        ],
                        peak: -61.52748954946662_f64,
                        bands: vec![
                            0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64,
                        ],
                        td: [
                            0.0_f64,
                            0.0006047349888831377_f64,
                            -0.00034484118805266917_f64,
                        ],
                        history_length: 1,
                        h0: 0.0_f64,
                    },
                    Reference {
                        t: 1016.0,
                        meter: vec![
                            (-27.944378194904473_f64, -39.94437819490447_f64),
                            (-28.933863685952584_f64, -40.93386368595258_f64),
                        ],
                        peak: -27.944378194904473_f64,
                        bands: vec![
                            0.10829617083072662_f64,
                            0.15779165923595428_f64,
                            0.1332261711359024_f64,
                            0.13004395365715027_f64,
                            0.13301125168800354_f64,
                            0.15462195873260498_f64,
                            0.21813517808914185_f64,
                            0.2195979207754135_f64,
                        ],
                        td: [
                            0.009830362163484097_f64,
                            -0.02774200215935707_f64,
                            0.010272160172462463_f64,
                        ],
                        history_length: 1,
                        h0: 0.0_f64,
                    },
                    Reference {
                        t: 1500.0,
                        meter: vec![
                            (-17.132968613088146_f64, -29.13296861308815_f64),
                            (-16.69532166255433_f64, -28.695321662554335_f64),
                        ],
                        peak: -16.69532166255433_f64,
                        bands: vec![
                            0.1404750943183899_f64,
                            0.143928661942482_f64,
                            0.15003418922424316_f64,
                            0.17644523084163666_f64,
                            0.2876659035682678_f64,
                            0.2603825628757477_f64,
                            0.33358603715896606_f64,
                            0.3091367483139038_f64,
                        ],
                        td: [
                            -2.288508746452222e-15_f64,
                            0.10029774159193039_f64,
                            -0.05719330534338951_f64,
                        ],
                        history_length: 2,
                        h0: 0.0_f64,
                    },
                    Reference {
                        t: 2733.5,
                        meter: vec![
                            (-60.684882675191616_f64, -72.68488267519162_f64),
                            (-60.04044087937559_f64, -72.0404408793756_f64),
                        ],
                        peak: -60.04044087937559_f64,
                        bands: vec![
                            0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64,
                        ],
                        td: [
                            -0.00047228645416907966_f64,
                            0.0005988451885059476_f64,
                            8.728325337870046e-05_f64,
                        ],
                        history_length: 3,
                        h0: 0.0_f64,
                    },
                    Reference {
                        t: 3901.0,
                        meter: vec![
                            (-22.064642047497948_f64, -34.06464204749795_f64),
                            (-22.02705170513171_f64, -34.02705170513171_f64),
                        ],
                        peak: -22.02705170513171_f64,
                        bands: vec![
                            0.16487941145896912_f64,
                            0.16667164862155914_f64,
                            0.2035670280456543_f64,
                            0.17767564952373505_f64,
                            0.24434170126914978_f64,
                            0.2447824776172638_f64,
                            0.24193505942821503_f64,
                            0.3941977322101593_f64,
                        ],
                        td: [
                            0.052479468286037445_f64,
                            -0.02301125042140484_f64,
                            -0.042708516120910645_f64,
                        ],
                        history_length: 4,
                        h0: 0.0_f64,
                    },
                    Reference {
                        t: 7777.0,
                        meter: vec![
                            (-58.81934800849951_f64, -70.81934800849952_f64),
                            (-58.43945205245792_f64, -70.43945205245792_f64),
                        ],
                        peak: -58.43945205245792_f64,
                        bands: vec![
                            0.00627922872081399_f64,
                            0.007482001557946205_f64,
                            0.00835328083485365_f64,
                            0.007250869181007147_f64,
                            0.009768517687916756_f64,
                            0.0105020422488451_f64,
                            0.013401636853814125_f64,
                            0.012066008523106575_f64,
                        ],
                        td: [
                            -0.0004939589416608214_f64,
                            0.0004371939867269248_f64,
                            -0.0006955671706236899_f64,
                        ],
                        history_length: 5,
                        h0: 0.0_f64,
                    },
                ],
            ),
        ];

        fn near(actual: f32, expected: f64, tolerance: f64, what: &str) {
            if expected.is_infinite() {
                assert_eq!(actual as f64, expected, "{what}");
            } else {
                assert!(
                    (actual as f64 - expected).abs() <= tolerance,
                    "{what}: {actual} vs {expected}"
                );
            }
        }
        for (options, references) in cases {
            let mut signal = DemoSignal::new(options.clone());
            for reference in references {
                signal.tick(reference.t);
                let what = format!("{:?} at {}", options.kind, reference.t);
                let frame = meter(&signal);
                assert_eq!(frame.channels.len(), reference.meter.len());
                for (level, (peak, rms)) in frame.channels.iter().zip(&reference.meter) {
                    near(level.peak_db, *peak, 1e-4, &what);
                    near(level.rms_db.unwrap(), *rms, 1e-4, &what);
                }
                let visual = visual(&signal);
                near(visual.peak_db, reference.peak, 1e-4, &what);
                for (band, expected) in visual.bands.iter().zip(&reference.bands) {
                    near(*band, *expected, 1e-6, &what);
                }
                let time_domain = visual.time_domain.unwrap();
                for (index, expected) in [0, 37, 255].into_iter().zip(reference.td) {
                    near(time_domain[index], expected, 1e-6, &what);
                }
                assert_eq!(visual.history_length, reference.history_length, "{what}");
                near(visual.history[0], reference.h0, 1e-6, &what);
            }
        }
    }

    #[test]
    fn channels_are_clamped_and_bands_resize() {
        let mut signal = DemoSignal::new(DemoSignalOptions {
            channels: 20,
            bands: 64,
            ..Default::default()
        });
        assert_eq!(signal.options().channels, 8);
        signal.tick(0.0);
        assert_eq!(visual(&signal).bands.len(), 64);
        assert_eq!(visual(&signal).time_domain.unwrap().len(), 256);
        signal.configure(&DemoSignalOptions {
            channels: 0,
            ..Default::default()
        });
        signal.tick(16.0);
        assert_eq!(meter(&signal).channels.len(), 1);
        assert_eq!(visual(&signal).bands.len(), 32);
    }

    #[test]
    fn paused_signal_is_silent() {
        let mut signal = DemoSignal::new(DemoSignalOptions {
            playing: false,
            ..Default::default()
        });
        signal.tick(16.0);
        assert_eq!(meter(&signal).channels[0].peak_db, f32::NEG_INFINITY);
        assert_eq!(visual(&signal).peak_db, f32::NEG_INFINITY);
    }

    #[test]
    fn demo_mixer_sums_inputs_into_a_stereo_master() {
        use crate::signal::mixer::{MixerChannelInit, MixerOptions};
        let tone = DemoSignalOptions::kind(DemoSignalKind::Tone);
        let mut mixer = DemoMixer::new(&[
            DemoMixerChannel::new("a", tone.clone()),
            DemoMixerChannel::new("b", tone),
        ]);
        let master = mixer.master_source();
        mixer.tick(16.0);
        let frame = master.latest().unwrap();
        assert_eq!(frame.channels.len(), 2);
        // Two −12 dB tones summed in amplitude: about −6 dB.
        assert!((frame.channels[0].peak_db - (-12.0 + 6.0206)).abs() < 0.01);

        let mut state = MixerState::new(MixerOptions {
            channels: vec![MixerChannelInit::new("a"), MixerChannelInit::new("b")],
            ..Default::default()
        });
        state.set_muted("b", true);
        mixer.configure(&state);
        mixer.tick(32.0);
        let frame = master.latest().unwrap();
        assert!((frame.channels[1].peak_db - -12.0).abs() < 0.01);
        assert_eq!(
            mixer.source("b").unwrap().latest().unwrap().channels[0].peak_db,
            f32::NEG_INFINITY
        );
    }
}
