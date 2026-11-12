//! Port of `hooks/use-web-audio-mixer.ts`: real audio behind a
//! [`MixerState`] (from [`use_mixer`](crate::signal::mixer::use_mixer)).
//!
//! Per channel: input → gain (0 when muted or soloed out) → ducking stage →
//! equal-power stereo panner → post-fader meter, summed into a master bus
//! (master gain → optional peak limiter → master meter). Channels with
//! `monitor` on are also sent to the speakers at the master level, exactly
//! like React, where the mix itself goes to a `MediaStream` (not heard).
//! Every gain, pan and send ramps (5 ms) so changes never click, and strips
//! are built silent and fade in to their level.
//!
//! ```ignore
//! let mixer = WebAudioMixer::new(WebAudioMixerOptions {
//!     ducking: vec![DuckingOptions::new("mic", ["music"])],
//!     ..Default::default()
//! })?;
//! mixer.set_input("mic", AudioInput::Microphone(mic));
//! player.update(cx, |p, _| p.set_destination(Destination::Bus(mixer.input_bus("music"))));
//! mixer.apply(&mixer_state);                  // every render
//! ChannelStrip::new("mic").source(mixer.meter("mic"))
//! ```

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::SharedString;
use parking_lot::Mutex;

use super::Result;
use super::analyser::{AnalyserChannels, AnalyserTap, AnalyserTapOptions, TapWriter};
use super::context::{AudioInput, AudioSource, Bus, Destination};
use super::gain::ramp_coefficient;
use crate::audio::decibels::{db_to_gain, gain_to_db};
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::types::{MeterFrame, VisualFrame};
use crate::signal::mixer::{MixerState, is_channel_audible};

const RAMP_SECONDS: f32 = 0.005;
const MS_PER_SECOND: f32 = 1000.0;
const TIME_CONSTANTS_PER_RAMP: f32 = 3.0;
/// How long a duck holds after the trigger's last loud block.
const DUCK_HOLD_SECONDS: f64 = 0.2;

/// `DuckingOptions`: lower some channels while another is active.
#[derive(Clone, Debug, PartialEq)]
pub struct DuckingOptions {
    /// The channel whose level triggers ducking, usually the microphone.
    pub trigger: SharedString,
    /// Channels that get quieter while the trigger is active.
    pub targets: Vec<SharedString>,
    /// Level that counts as active. Default −35 dBFS.
    pub threshold_db: f32,
    /// How much quieter the targets get. Default −12 dB.
    pub amount_db: f32,
    /// Default 50 ms.
    pub attack_ms: f32,
    /// Default 400 ms.
    pub release_ms: f32,
}

impl DuckingOptions {
    pub fn new(
        trigger: impl Into<SharedString>,
        targets: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            trigger: trigger.into(),
            targets: targets.into_iter().map(Into::into).collect(),
            threshold_db: -35.0,
            amount_db: -12.0,
            attack_ms: 50.0,
            release_ms: 400.0,
        }
    }
}

/// `WebAudioMixerOptions` (inputs are set with [`WebAudioMixer::set_input`]).
#[derive(Clone, Debug, PartialEq)]
pub struct WebAudioMixerOptions {
    pub ducking: Vec<DuckingOptions>,
    /// A peak limiter on the master output. Default true.
    pub limiter: bool,
    /// Analysis settings for the channel and master meters (`fft_size`,
    /// `bands`, `history_size`, `smoothing`); meters are always stereo.
    pub analyser: AnalyserTapOptions,
}

impl Default for WebAudioMixerOptions {
    fn default() -> Self {
        Self {
            ducking: Vec::new(),
            limiter: true,
            analyser: AnalyserTapOptions::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Pure DSP
// ---------------------------------------------------------------------------

/// `StereoPannerNode`: equal-power panning, `pan` −1..1. `mono` uses the
/// one-channel law (the input is `left`).
pub fn stereo_pan(left: f32, right: f32, pan: f32, mono: bool) -> (f32, f32) {
    let pan = pan.clamp(-1.0, 1.0);
    if mono {
        let x = (pan + 1.0) / 2.0;
        return (left * (x * FRAC_PI_2).cos(), left * (x * FRAC_PI_2).sin());
    }
    if pan <= 0.0 {
        let x = pan + 1.0;
        (
            left + right * (x * FRAC_PI_2).cos(),
            right * (x * FRAC_PI_2).sin(),
        )
    } else {
        let x = pan;
        (
            left * (x * FRAC_PI_2).cos(),
            right + left * (x * FRAC_PI_2).sin(),
        )
    }
}

/// The master limiter: a `DynamicsCompressorNode` with threshold −1 dB,
/// knee 0, ratio 20, attack 1 ms, release 50 ms (peak detection, no makeup
/// gain).
#[derive(Clone, Debug)]
pub struct Limiter {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
    reduction_db: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Self {
            threshold_db: -1.0,
            ratio: 20.0,
            attack: 0.001,
            release: 0.05,
            reduction_db: 0.0,
        }
    }
}

impl Limiter {
    /// Limits interleaved stereo in place.
    pub fn process(&mut self, block: &mut [f32], sample_rate: u32) {
        let attack = ramp_coefficient(self.attack, sample_rate);
        let release = ramp_coefficient(self.release, sample_rate);
        let slope = 1.0 - 1.0 / self.ratio;
        for pair in block.as_chunks_mut::<2>().0.iter_mut() {
            let level = gain_to_db(pair[0].abs().max(pair[1].abs()));
            let wanted = (level - self.threshold_db).max(0.0) * slope;
            let coefficient = if wanted > self.reduction_db {
                attack
            } else {
                release
            };
            self.reduction_db += (wanted - self.reduction_db) * coefficient;
            let gain = db_to_gain(-self.reduction_db);
            pair[0] *= gain;
            pair[1] *= gain;
        }
    }

    /// Current gain reduction, in dB (≥ 0).
    pub fn reduction_db(&self) -> f32 {
        self.reduction_db
    }
}

/// A value ramped like `setTargetAtTime`, owned by the audio thread.
#[derive(Clone, Copy, Debug)]
struct Smoothed {
    current: f32,
    target: f32,
    time_constant: f32,
    coefficient: f32,
}

impl Smoothed {
    fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            time_constant: RAMP_SECONDS,
            coefficient: 1.0,
        }
    }

    fn set(&mut self, target: f32, time_constant: f32) {
        self.target = target;
        self.time_constant = time_constant;
    }

    fn begin(&mut self, sample_rate: u32) {
        self.coefficient = ramp_coefficient(self.time_constant, sample_rate);
    }

    fn next(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coefficient;
        self.current
    }
}

// ---------------------------------------------------------------------------
// The graph
// ---------------------------------------------------------------------------

struct Strip {
    id: SharedString,
    source: Box<dyn AudioSource>,
    finished: bool,
    gain: Smoothed,
    duck: Smoothed,
    pan: Smoothed,
    monitor: Smoothed,
    duck_hold_until: f64,
    peak_db: f32,
    writer: TapWriter,
    _tap: AnalyserTap,
}

struct Graph {
    strips: Vec<Strip>,
    state: Option<MixerState>,
    ducking: Vec<DuckingOptions>,
    master: Smoothed,
    monitor_master: Smoothed,
    limiter: Option<Limiter>,
    master_writer: TapWriter,
    destination: Bus,
    time: f64,
    strip_buffer: Vec<f32>,
    master_buffer: Vec<f32>,
    monitor_buffer: Vec<f32>,
}

impl Graph {
    /// `apply()`: ramps every strip and the master to the state.
    fn apply(&mut self) {
        let Some(state) = &self.state else {
            return;
        };
        for strip in &mut self.strips {
            let Some(channel) = state.channel(&strip.id) else {
                strip.gain.set(0.0, RAMP_SECONDS);
                continue;
            };
            let audible = is_channel_audible(state, &strip.id);
            let gain = if audible {
                db_to_gain(channel.gain_db)
            } else {
                0.0
            };
            strip.gain.set(gain, RAMP_SECONDS);
            strip.pan.set(channel.pan, RAMP_SECONDS);
            strip
                .monitor
                .set(if channel.monitor { 1.0 } else { 0.0 }, RAMP_SECONDS);
        }
        let level = if state.master.muted {
            0.0
        } else {
            db_to_gain(state.master.gain_db)
        };
        self.master.set(level, RAMP_SECONDS);
        self.monitor_master.set(level, RAMP_SECONDS);
    }

    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let len = out.len();
        for buffer in [
            &mut self.strip_buffer,
            &mut self.master_buffer,
            &mut self.monitor_buffer,
        ] {
            buffer.clear();
            buffer.resize(len, 0.0);
        }
        // Your own nodes, straight into the master bus.
        self.destination
            .render(&mut self.master_buffer, sample_rate);

        for strip in &mut self.strips {
            let buffer = &mut self.strip_buffer;
            buffer.fill(0.0);
            if !strip.finished {
                strip.finished = !strip.source.render(buffer, sample_rate);
            }
            let mono = strip.source.channel_count() == 1;
            for value in [
                &mut strip.gain,
                &mut strip.duck,
                &mut strip.pan,
                &mut strip.monitor,
            ] {
                value.begin(sample_rate);
            }
            let mut peak = 0.0f32;
            for ((pair, master), monitor) in buffer
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(self.master_buffer.as_chunks_mut::<2>().0.iter_mut())
                .zip(self.monitor_buffer.as_chunks_mut::<2>().0.iter_mut())
            {
                let level = strip.gain.next() * strip.duck.next();
                let (left, right) =
                    stereo_pan(pair[0] * level, pair[1] * level, strip.pan.next(), mono);
                pair[0] = left;
                pair[1] = right;
                master[0] += left;
                master[1] += right;
                let send = strip.monitor.next();
                monitor[0] += left * send;
                monitor[1] += right * send;
                peak = peak.max(left.abs()).max(right.abs());
            }
            strip.writer.write(buffer, 2);
            strip.peak_db = gain_to_db(peak);
        }

        let block_seconds = (len / 2) as f64 / sample_rate.max(1) as f64;
        self.time += block_seconds;
        self.duck(self.time);

        self.master.begin(sample_rate);
        self.monitor_master.begin(sample_rate);
        for ((master, monitor), out) in self
            .master_buffer
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(self.monitor_buffer.as_chunks::<2>().0.iter())
            .zip(out.as_chunks_mut::<2>().0.iter_mut())
        {
            let level = self.master.next();
            master[0] *= level;
            master[1] *= level;
            let send = self.monitor_master.next();
            out[0] += monitor[0] * send;
            out[1] += monitor[1] * send;
        }
        if let Some(limiter) = &mut self.limiter {
            limiter.process(&mut self.master_buffer, sample_rate);
        }
        self.master_writer.write(&self.master_buffer, 2);
    }

    /// Ducks targets while a trigger is loud; releases after the hold.
    fn duck(&mut self, now: f64) {
        for rule in &self.ducking {
            let loud = self
                .strips
                .iter()
                .any(|strip| strip.id == rule.trigger && strip.peak_db >= rule.threshold_db);
            if !loud {
                continue;
            }
            let attack = rule.attack_ms / MS_PER_SECOND / TIME_CONSTANTS_PER_RAMP;
            for strip in &mut self.strips {
                if rule.targets.contains(&strip.id) {
                    strip.duck.set(db_to_gain(rule.amount_db), attack);
                    strip.duck_hold_until = now + DUCK_HOLD_SECONDS;
                }
            }
        }
        for strip in &mut self.strips {
            if strip.duck.target != 1.0 && now >= strip.duck_hold_until {
                let release = self
                    .ducking
                    .iter()
                    .find(|rule| rule.targets.contains(&strip.id))
                    .map_or(RAMP_SECONDS, |rule| {
                        rule.release_ms / MS_PER_SECOND / TIME_CONSTANTS_PER_RAMP
                    });
                strip.duck.set(1.0, release);
            }
        }
    }
}

struct MixerSource {
    graph: Arc<Mutex<Graph>>,
    alive: Arc<AtomicBool>,
}

impl AudioSource for MixerSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        self.graph.lock().render(out, sample_rate);
        true
    }
}

struct Relays {
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
}

impl Default for Relays {
    fn default() -> Self {
        Self {
            meter: create_frame_emitter(),
            visual: create_frame_emitter(),
        }
    }
}

/// The mixer graph (`useWebAudioMixer`'s return value plus its controls).
/// Disconnects when dropped.
pub struct WebAudioMixer {
    graph: Arc<Mutex<Graph>>,
    relays: Mutex<HashMap<SharedString, Relays>>,
    inputs: Mutex<HashMap<SharedString, Bus>>,
    master: Relays,
    alive: Arc<AtomicBool>,
    analyser: AnalyserTapOptions,
    sample_rate: u32,
    _master_tap: AnalyserTap,
}

impl WebAudioMixer {
    /// Builds the graph on the shared output (its monitor sends play there).
    pub fn new(options: WebAudioMixerOptions) -> Result<WebAudioMixer> {
        Self::with_destination(options, Destination::Speakers)
    }

    /// Builds the graph with its speaker output (the monitor sends) going to
    /// `destination`.
    pub fn with_destination(
        options: WebAudioMixerOptions,
        destination: Destination,
    ) -> Result<WebAudioMixer> {
        let sample_rate = destination.sample_rate();
        let analyser = AnalyserTapOptions {
            channels: AnalyserChannels::Stereo,
            ..options.analyser
        };
        let master = Relays::default();
        let master_tap = AnalyserTap::with_emitters(
            analyser,
            sample_rate,
            master.meter.clone(),
            master.visual.clone(),
        );
        let graph = Arc::new(Mutex::new(Graph {
            strips: Vec::new(),
            state: None,
            ducking: options.ducking,
            // Built silent; apply() fades in to the mixer's level.
            master: Smoothed::new(0.0),
            monitor_master: Smoothed::new(0.0),
            limiter: options.limiter.then(Limiter::default),
            master_writer: master_tap.writer(),
            destination: Bus::new(),
            time: 0.0,
            strip_buffer: Vec::new(),
            master_buffer: Vec::new(),
            monitor_buffer: Vec::new(),
        }));
        let alive = Arc::new(AtomicBool::new(true));
        if let Some(output) = destination.connect(Box::new(MixerSource {
            graph: graph.clone(),
            alive: alive.clone(),
        }))? {
            drop(output);
        }
        Ok(WebAudioMixer {
            graph,
            relays: Mutex::new(HashMap::new()),
            inputs: Mutex::new(HashMap::new()),
            master,
            alive,
            analyser,
            sample_rate,
            _master_tap: master_tap,
        })
    }

    fn relays<R>(&self, id: &SharedString, f: impl FnOnce(&Relays) -> R) -> R {
        let mut relays = self.relays.lock();
        f(relays.entry(id.clone()).or_default())
    }

    /// Sets (or replaces) a channel's input.
    pub fn set_input(&self, id: impl Into<SharedString>, input: AudioInput) {
        let id = id.into();
        self.inputs.lock().remove(&id);
        self.set_strip(id, input);
    }

    fn set_strip(&self, id: SharedString, input: AudioInput) {
        let tap = self.relays(&id, |relays| {
            AnalyserTap::with_emitters(
                self.analyser,
                self.sample_rate,
                relays.meter.clone(),
                relays.visual.clone(),
            )
        });
        let strip = Strip {
            id: id.clone(),
            source: input.into_source(),
            finished: false,
            // Built silent, so a muted or soloed-out channel never leaks at
            // unity before apply() ramps it to its level.
            gain: Smoothed::new(0.0),
            duck: Smoothed::new(1.0),
            pan: Smoothed::new(0.0),
            monitor: Smoothed::new(0.0),
            duck_hold_until: 0.0,
            peak_db: f32::NEG_INFINITY,
            writer: tap.writer(),
            _tap: tap,
        };
        let mut graph = self.graph.lock();
        graph.strips.retain(|strip| strip.id != id);
        graph.strips.push(strip);
        graph.apply();
    }

    /// A bus feeding channel `id` (created and connected on first use), for
    /// routing players, sounds or gain nodes into the channel.
    pub fn input_bus(&self, id: impl Into<SharedString>) -> Bus {
        let id = id.into();
        let mut inputs = self.inputs.lock();
        if let Some(bus) = inputs.get(&id) {
            return bus.clone();
        }
        let bus = Bus::new();
        inputs.insert(id.clone(), bus.clone());
        drop(inputs);
        self.set_strip(id, AudioInput::Bus(bus.clone()));
        bus
    }

    /// Removes a channel's input (its meters stop).
    pub fn remove_input(&self, id: &str) {
        self.graph.lock().strips.retain(|strip| strip.id != id);
        self.inputs.lock().remove(id);
    }

    /// Applies the mixer state (gain, mute, solo, pan, monitor, master).
    pub fn apply(&self, state: &MixerState) {
        let mut graph = self.graph.lock();
        if graph.state.as_ref() != Some(state) {
            graph.state = Some(state.clone());
            graph.apply();
        }
    }

    /// Replaces the ducking rules. Ducked channels release.
    pub fn set_ducking(&self, rules: Vec<DuckingOptions>) {
        let mut graph = self.graph.lock();
        graph.ducking = rules;
        for strip in &mut graph.strips {
            strip.duck.set(1.0, RAMP_SECONDS);
        }
    }

    /// Post-fader meter of a channel (stable across input changes).
    pub fn meter(&self, id: impl Into<SharedString>) -> FrameSource<MeterFrame> {
        self.relays(&id.into(), |relays| relays.meter.source())
    }

    /// Post-fader visual frames of a channel.
    pub fn visual(&self, id: impl Into<SharedString>) -> FrameSource<VisualFrame> {
        self.relays(&id.into(), |relays| relays.visual.source())
    }

    /// The master meter (after the limiter).
    pub fn master_meter(&self) -> FrameSource<MeterFrame> {
        self.master.meter.source()
    }

    pub fn master_visual(&self) -> FrameSource<VisualFrame> {
        self.master.visual.source()
    }

    /// The master bus input, for routing your own sources into the mix.
    pub fn destination(&self) -> Bus {
        self.graph.lock().destination.clone()
    }

    /// The limiter's current gain reduction in dB (0 without a limiter).
    pub fn limiter_reduction_db(&self) -> f32 {
        self.graph
            .lock()
            .limiter
            .as_ref()
            .map_or(0.0, Limiter::reduction_db)
    }
}

impl Drop for WebAudioMixer {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::context::FnSource;
    use crate::signal::mixer::{MixerChannelInit, MixerOptions};

    fn constant(value: f32, channels: usize) -> AudioInput {
        struct Constant(f32, usize);
        impl AudioSource for Constant {
            fn render(&mut self, out: &mut [f32], _: u32) -> bool {
                out.iter_mut().for_each(|s| *s += self.0);
                true
            }
            fn channel_count(&self) -> usize {
                self.1
            }
        }
        AudioInput::source(Constant(value, channels))
    }

    fn state(channels: Vec<MixerChannelInit>) -> MixerState {
        MixerState::new(MixerOptions {
            channels,
            ..Default::default()
        })
    }

    /// Renders ~100 ms (ramps settle) and returns the last frame.
    fn settle(speakers: &Bus) -> [f32; 2] {
        let mut out = vec![0.0f32; 2 * 4800];
        speakers.render(&mut out, 48_000);
        [out[out.len() - 2], out[out.len() - 1]]
    }

    #[test]
    fn pan_law_is_equal_power() {
        let (l, r) = stereo_pan(1.0, 0.0, 0.0, true);
        assert!((l - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6 && (l - r).abs() < 1e-6);
        assert!((l * l + r * r - 1.0).abs() < 1e-5);
        assert_eq!(stereo_pan(1.0, 0.0, -1.0, true), (1.0, 0.0));
        let (l, r) = stereo_pan(1.0, 0.0, 1.0, true);
        assert!(l.abs() < 1e-6 && (r - 1.0).abs() < 1e-6);
        // Stereo: centre passes through; hard left folds right into left.
        let (l, r) = stereo_pan(0.3, 0.6, 0.0, false);
        assert!((l - 0.3).abs() < 1e-6 && (r - 0.6).abs() < 1e-6);
        let (l, r) = stereo_pan(0.3, 0.6, -1.0, false);
        assert!((l - 0.9).abs() < 1e-6 && r.abs() < 1e-6);
    }

    #[test]
    fn limiter_holds_peaks_near_the_threshold() {
        let mut limiter = Limiter::default();
        let mut block = vec![1.0f32; 2 * 4800];
        limiter.process(&mut block, 48_000);
        let out = gain_to_db(block[block.len() - 1]);
        // 0 dBFS in: −1 + 1/20 dB out.
        assert!((out - (-0.95)).abs() < 0.01, "{out}");
        let mut quiet = vec![0.5f32; 2 * 48_000];
        limiter.process(&mut quiet, 48_000);
        assert!((quiet[quiet.len() - 1] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn mixes_monitored_channels_with_gain_pan_mute_and_solo() {
        let speakers = Bus::new();
        let mixer = WebAudioMixer::with_destination(
            WebAudioMixerOptions {
                limiter: false,
                ..Default::default()
            },
            Destination::Bus(speakers.clone()),
        )
        .unwrap();
        mixer.set_input("a", constant(0.5, 1));
        mixer.set_input("b", constant(0.25, 2));
        let mut mix = state(vec![
            MixerChannelInit::new("a").monitor(true).pan(-1.0),
            MixerChannelInit::new("b").monitor(true).gain_db(-6.0206),
        ]);
        mixer.apply(&mix);
        let [l, r] = settle(&speakers);
        // a: mono hard left → (0.5, 0). b: stereo centre at ½ → (0.125, 0.125).
        assert!(
            (l - 0.625).abs() < 1e-3 && (r - 0.125).abs() < 1e-3,
            "{l} {r}"
        );

        mix.set_muted("a", true);
        mixer.apply(&mix);
        let [l, r] = settle(&speakers);
        assert!((l - 0.125).abs() < 1e-3 && (r - 0.125).abs() < 1e-3);

        mix.set_muted("a", false);
        mix.set_solo("a", true, false);
        mixer.apply(&mix);
        let [l, r] = settle(&speakers);
        assert!((l - 0.5).abs() < 1e-3 && r.abs() < 1e-3);

        mix.set_monitor("a", false);
        mixer.apply(&mix);
        assert!(
            settle(&speakers)[0].abs() < 1e-3,
            "unmonitored channels aren't heard"
        );

        mix.set_master_muted(true);
        mix.set_monitor("a", true);
        mixer.apply(&mix);
        assert!(settle(&speakers)[0].abs() < 1e-3);
    }

    #[test]
    fn strips_start_silent_without_state() {
        let speakers = Bus::new();
        let mixer =
            WebAudioMixer::with_destination(Default::default(), Destination::Bus(speakers.clone()))
                .unwrap();
        mixer.set_input("a", constant(1.0, 2));
        assert_eq!(settle(&speakers), [0.0, 0.0]);
    }

    #[test]
    fn ducks_targets_while_the_trigger_is_loud() {
        let speakers = Bus::new();
        let mixer = WebAudioMixer::with_destination(
            WebAudioMixerOptions {
                limiter: false,
                ducking: vec![DuckingOptions::new("mic", ["music"])],
                ..Default::default()
            },
            Destination::Bus(speakers.clone()),
        )
        .unwrap();
        let talking = Arc::new(AtomicBool::new(true));
        let voice = talking.clone();
        mixer.set_input(
            "mic",
            AudioInput::source(FnSource(move |out: &mut [f32], _| {
                if voice.load(Ordering::Relaxed) {
                    out.iter_mut().for_each(|s| *s += 0.1);
                }
                true
            })),
        );
        mixer.set_input("music", constant(0.5, 2));
        let mix = state(vec![
            MixerChannelInit::new("mic"),
            MixerChannelInit::new("music").monitor(true),
        ]);
        mixer.apply(&mix);
        for _ in 0..5 {
            settle(&speakers);
        }
        let [l, _] = settle(&speakers);
        assert!((l - 0.5 * db_to_gain(-12.0)).abs() < 1e-3, "ducked: {l}");

        talking.store(false, Ordering::Relaxed);
        for _ in 0..30 {
            settle(&speakers);
        }
        let [l, _] = settle(&speakers);
        assert!((l - 0.5).abs() < 1e-3, "released: {l}");
    }

    #[test]
    fn meters_are_stable_per_channel() {
        let mixer =
            WebAudioMixer::with_destination(Default::default(), Destination::Bus(Bus::new()))
                .unwrap();
        assert_eq!(mixer.meter("a"), mixer.meter("a"));
        assert_ne!(mixer.meter("a"), mixer.meter("b"));
        let bus = mixer.input_bus("a");
        assert_eq!(bus, mixer.input_bus("a"));
    }
}
