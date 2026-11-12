//! Gallery page for `music-player`. One variant per docs example:
//! `components/examples/music-player-demo.tsx`, `components/examples/music-player-ducking.tsx`.
//!
//! The docs synthesise `DEMO_TRACKS` in an `OfflineAudioContext`
//! (`lib/docs/demo-audio.ts`). There is no decoder here, so the seek bars use
//! the same arrangements rendered to peaks (`useWaveformData` asks for 400).
//! The ducking example opens the real microphone with the `device` feature
//! (native); without it (the wasm build) a speech demo signal stands in.

use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::signal::waveform_data::compute_peaks;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{MusicPlayer, MusicTrack};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "music-player",
    title: "Music Player",
    description: "A playlist player with a waveform seek bar, shuffle, repeat and ducking under the microphone.",
    variants: &["default", "ducking"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "ducking" => ducking(window, cx),
        _ => return None,
    })
}

/// `music-player-demo.tsx`: `w-full max-w-md`.
fn demo(_window: &mut Window, _cx: &mut App) -> AnyElement {
    MusicPlayer::new(id("music-player", "default"))
        .tracks(demo_tracks())
        .w_full()
        .max_w(px(448.0))
        .into_any_element()
}

struct MicToggle {
    listening: bool,
}

/// `music-player-ducking.tsx`: a button that starts and stops the microphone,
/// whose analyser meter ducks the player.
fn ducking(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("music-player", "ducking");
    let toggle = window.use_keyed_state(format!("{base}/mic"), cx, |_, _| MicToggle {
        listening: false,
    });
    let wanted = toggle.read(cx).listening;
    let (listening, meter) = microphone(&base, wanted, window, cx);
    div()
        .flex()
        .w_full()
        .max_w(px(448.0))
        .flex_col()
        .gap(px(12.0))
        .child(
            Button::new(format!("{base}/mic-button"))
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .self_start()
                .on_click({
                    let toggle = toggle.clone();
                    move |_, _, cx| {
                        toggle.update(cx, |mic, cx| {
                            mic.listening = !mic.listening;
                            cx.notify();
                        });
                    }
                })
                .child(if listening {
                    "Stop microphone"
                } else {
                    "Use my microphone to duck"
                }),
        )
        .child(
            MusicPlayer::new(base)
                .tracks(demo_tracks())
                .ducking_source(meter),
        )
        .into_any_element()
}

/// `useMicrophone()` + `useAudioAnalyser(stream).meter`, open while `wanted`.
/// Returns whether it is listening (`status === "active"`) and the meter.
#[cfg(all(feature = "device", not(target_family = "wasm")))]
fn microphone(
    base: &str,
    wanted: bool,
    window: &mut Window,
    cx: &mut App,
) -> (bool, audiocn::FrameSource<audiocn::MeterFrame>) {
    use audiocn::io::{MicrophoneOptions, MicrophoneStatus, use_microphone};
    let microphone = use_microphone(
        format!("{base}/microphone"),
        MicrophoneOptions {
            enabled: wanted,
            ..Default::default()
        },
        window,
        cx,
    );
    (
        microphone.status == MicrophoneStatus::Active,
        microphone.meter,
    )
}

/// No microphone: a speech [`DemoSignal`](audiocn::signal::demo_signal::DemoSignal)
/// stands in, "listening" as soon as it is asked to.
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
fn microphone(
    base: &str,
    wanted: bool,
    window: &mut Window,
    cx: &mut App,
) -> (bool, audiocn::FrameSource<audiocn::MeterFrame>) {
    let signal = use_demo_signal(
        format!("{base}/voice"),
        DemoSignalOptions::kind(DemoSignalKind::Speech).with_playing(wanted),
        window,
        cx,
    );
    (wanted, signal.meter)
}

fn demo_tracks() -> Vec<MusicTrack> {
    let peaks = all_peaks();
    vec![
        MusicTrack::new("night-drive", "Night Drive", "night-drive")
            .artist("audiocn")
            .duration(19.2)
            .peaks(peaks[0].clone()),
        MusicTrack::new("low-tide", "Low Tide", "low-tide")
            .artist("audiocn")
            .duration(24.0)
            .peaks(peaks[1].clone()),
        MusicTrack::new("arcade", "Arcade", "arcade")
            .artist("audiocn")
            .duration(13.7)
            .peaks(peaks[2].clone()),
    ]
}

fn all_peaks() -> &'static [Arc<[f32]>; 3] {
    static PEAKS: OnceLock<[Arc<[f32]>; 3]> = OnceLock::new();
    PEAKS.get_or_init(|| {
        [
            peaks_of(19.2, render_night_drive),
            peaks_of(24.0, render_low_tide),
            peaks_of(13.7, render_arcade),
        ]
    })
}

/// `useWaveformData(..., { samples: 400 })`.
const PEAK_COUNT: usize = 400;
const SAMPLE_RATE: f64 = 22_050.0;

fn peaks_of(seconds: f64, render: fn(&mut [f32])) -> Arc<[f32]> {
    let mut samples = vec![0.0f32; index_at(seconds)];
    render(&mut samples);
    compute_peaks(&[samples], PEAK_COUNT).into()
}

fn index_at(seconds: f64) -> usize {
    (seconds * SAMPLE_RATE).round().max(0.0) as usize
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Sawtooth,
    Square,
    Triangle,
}

#[derive(Clone, Copy)]
enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
}

/// Web Audio `BiquadFilterNode` at the default Q of 1.
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn new(kind: FilterKind, frequency: f64) -> Self {
        let w0 = TAU * frequency / SAMPLE_RATE;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0;
        let (b0, b1, b2) = match kind {
            FilterKind::Lowpass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            FilterKind::Highpass => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
            FilterKind::Bandpass => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// `envelope()`: 0.0001 → peak over attack, then → 0.0001 over decay.
struct Envelope {
    value: f64,
    attack_ratio: f64,
    decay_ratio: f64,
    attack_samples: usize,
    decay_samples: usize,
    index: usize,
}

impl Envelope {
    fn new(peak: f64, attack: f64, decay: f64) -> Self {
        let attack_samples = ((attack * SAMPLE_RATE).round() as usize).max(1);
        let decay_samples = ((decay * SAMPLE_RATE).round() as usize).max(1);
        let peak = peak.max(0.0001);
        Self {
            value: 0.0001,
            attack_ratio: (peak / 0.0001).powf(1.0 / attack_samples as f64),
            decay_ratio: (0.0001 / peak).powf(1.0 / decay_samples as f64),
            attack_samples,
            decay_samples,
            index: 0,
        }
    }

    fn next(&mut self) -> f64 {
        let value = self.value;
        if self.index < self.attack_samples {
            self.value *= self.attack_ratio;
        } else if self.index < self.attack_samples + self.decay_samples {
            self.value *= self.decay_ratio;
        }
        self.index += 1;
        value
    }
}

struct Tone {
    wave: Wave,
    frequency: f64,
    end_frequency: Option<f64>,
    detune: f64,
    start: f64,
    peak: f64,
    attack: f64,
    decay: f64,
}

fn tone(out: &mut [f32], tone: &Tone) {
    let first = index_at(tone.start);
    let length = index_at(tone.attack + tone.decay + 0.05);
    let ramp = index_at(tone.attack + tone.decay).max(1) as f64;
    let detune = 2f64.powf(tone.detune / 1200.0);
    let mut envelope = Envelope::new(tone.peak, tone.attack, tone.decay);
    let mut phase = 0.0f64;
    for step in 0..length {
        let Some(slot) = out.get_mut(first + step) else {
            break;
        };
        let frequency = match tone.end_frequency {
            Some(end) if (step as f64) < ramp && tone.frequency > 0.0 && end > 0.0 => {
                tone.frequency * (end / tone.frequency).powf(step as f64 / ramp)
            }
            Some(end) => end,
            None => tone.frequency,
        } * detune;
        let sample = match tone.wave {
            Wave::Sine => (TAU * phase).sin(),
            Wave::Sawtooth => 2.0 * (phase - (phase + 0.5).floor()),
            Wave::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Triangle => 1.0 - 4.0 * ((phase + 0.25).fract() - 0.5).abs(),
        };
        *slot += (sample * envelope.next()) as f32;
        phase = (phase + frequency / SAMPLE_RATE).fract();
    }
}

/// `noiseHit()`: seeded noise through a biquad and an envelope. The seed
/// restarts at 7 for every hit, like a fresh `noiseBuffer`.
fn noise_hit(out: &mut [f32], start: f64, peak: f64, decay: f64, kind: FilterKind, frequency: f64) {
    let first = index_at(start);
    let length = index_at(decay + 0.05);
    let mut filter = Biquad::new(kind, frequency);
    let mut envelope = Envelope::new(peak, 0.002, decay);
    let mut seed: u64 = 7;
    for step in 0..length {
        let Some(slot) = out.get_mut(first + step) else {
            break;
        };
        seed = (seed * 16_807) % 2_147_483_647;
        let noise = (seed as f64 / 2_147_483_647.0) * 2.0 - 1.0;
        *slot += (filter.process(noise) * envelope.next()) as f32;
    }
}

fn midi(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

fn kick(out: &mut [f32], start: f64) {
    tone(
        out,
        &Tone {
            wave: Wave::Sine,
            frequency: 150.0,
            end_frequency: Some(40.0),
            detune: 0.0,
            start,
            peak: 0.9,
            attack: 0.002,
            decay: 0.35,
        },
    );
}

fn snare(out: &mut [f32], start: f64) {
    noise_hit(out, start, 0.4, 0.18, FilterKind::Bandpass, 1800.0);
    tone(
        out,
        &Tone {
            wave: Wave::Triangle,
            frequency: 190.0,
            end_frequency: None,
            detune: 0.0,
            start,
            peak: 0.24,
            attack: 0.002,
            decay: 0.12,
        },
    );
}

fn hat(out: &mut [f32], start: f64, peak: f64) {
    noise_hit(out, start, peak, 0.05, FilterKind::Highpass, 7000.0);
}

fn drum_pattern(out: &mut [f32], bpm: f64, bars: i32, swing: f64) {
    let beat = 60.0 / bpm;
    for bar in 0..bars {
        for step in 0..8 {
            let time = (f64::from(bar) * 4.0 + f64::from(step) / 2.0) * beat
                + if step % 2 == 1 { swing * beat } else { 0.0 };
            if step == 0 || step == 5 {
                kick(out, time);
            }
            if step == 2 || step == 6 {
                snare(out, time);
            }
            hat(out, time, if step % 2 == 0 { 0.12 } else { 0.07 });
        }
    }
}

fn chord_progression(
    out: &mut [f32],
    chords: &[[f64; 4]],
    beats_per_chord: f64,
    bpm: f64,
    wave: Wave,
    peak: f64,
) {
    let beat = 60.0 / bpm;
    let mut pad = vec![0.0f32; out.len()];
    for (index, chord) in chords.iter().enumerate() {
        let start = index as f64 * beats_per_chord * beat;
        for &note in chord {
            for detune in [-7.0, 7.0] {
                tone(
                    &mut pad,
                    &Tone {
                        wave,
                        frequency: midi(note),
                        end_frequency: None,
                        detune,
                        start,
                        peak,
                        attack: 0.4,
                        decay: beats_per_chord * beat,
                    },
                );
            }
        }
        tone(
            out,
            &Tone {
                wave: Wave::Sine,
                frequency: midi(chord[0] - 12.0),
                end_frequency: None,
                detune: 0.0,
                start,
                peak: peak * 2.2,
                attack: 0.02,
                decay: beats_per_chord * beat * 0.9,
            },
        );
    }
    let mut lowpass = Biquad::new(FilterKind::Lowpass, 1800.0);
    for (slot, sample) in out.iter_mut().zip(pad) {
        *slot += lowpass.process(f64::from(sample)) as f32;
    }
}

fn render_night_drive(out: &mut [f32]) {
    let chords: [[f64; 4]; 8] = [
        [50.0, 53.0, 57.0, 60.0],
        [46.0, 50.0, 53.0, 57.0],
        [48.0, 52.0, 55.0, 59.0],
        [45.0, 48.0, 52.0, 55.0],
        [50.0, 53.0, 57.0, 60.0],
        [46.0, 50.0, 53.0, 57.0],
        [48.0, 52.0, 55.0, 59.0],
        [45.0, 48.0, 52.0, 55.0],
    ];
    chord_progression(out, &chords, 4.0, 100.0, Wave::Sawtooth, 0.025);
    drum_pattern(out, 100.0, 8, 0.0);
}

fn render_low_tide(out: &mut [f32]) {
    let chords: [[f64; 4]; 8] = [
        [57.0, 60.0, 64.0, 67.0],
        [53.0, 57.0, 60.0, 64.0],
        [55.0, 59.0, 62.0, 65.0],
        [52.0, 55.0, 59.0, 62.0],
        [57.0, 60.0, 64.0, 67.0],
        [53.0, 57.0, 60.0, 64.0],
        [55.0, 59.0, 62.0, 65.0],
        [52.0, 55.0, 59.0, 62.0],
    ];
    chord_progression(out, &chords, 4.0, 80.0, Wave::Triangle, 0.03);
    drum_pattern(out, 80.0, 8, 0.08);
}

fn render_arcade(out: &mut [f32]) {
    let bpm = 140.0;
    let beat = 60.0 / bpm;
    let notes = [72.0, 76.0, 79.0, 84.0, 79.0, 76.0, 72.0, 67.0];
    let roots = [0.0, 0.0, 5.0, 7.0];
    for step in 0..64 {
        let root = roots[(step / 16) % 4];
        tone(
            out,
            &Tone {
                wave: Wave::Square,
                frequency: midi(notes[step % notes.len()] + root),
                end_frequency: None,
                detune: 0.0,
                start: (step as f64 * beat) / 2.0,
                peak: 0.08,
                attack: 0.005,
                decay: beat / 2.0,
            },
        );
    }
    drum_pattern(out, bpm, 8, 0.0);
}
