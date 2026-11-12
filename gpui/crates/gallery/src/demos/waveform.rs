//! Gallery page for `waveform`. One variant per docs example:
//! `components/examples/waveform-demo.tsx`,
//! `components/examples/waveform-regions.tsx`,
//! `components/examples/waveform-variants.tsx`.
//!
//! The docs demo decodes "Night Drive", a track the docs synthesise with an
//! `OfflineAudioContext` (`lib/docs/demo-audio.ts`). There is no Web Audio
//! here, so [`night_drive`] renders the same score (chords, bass, kick, snare,
//! hats, same envelopes and filters) in plain Rust at 22.05 kHz, and its
//! peaks go through the same `compute_peaks`. The player is simulated: a
//! clock that emits the playhead on a `FrameSource<f64>` (no sound).

use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::audio::frame_loop::now_ms;
use audiocn::audio::frame_source::{FrameEmitter, create_frame_emitter};
use audiocn::audio::time::{FormatTimeOptions, format_time};
use audiocn::signal::waveform_data::compute_peaks;
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    Waveform, WaveformCanvas, WaveformCursor, WaveformHover, WaveformMarker, WaveformRegion,
    WaveformRegionValue, WaveformVariant,
};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "waveform",
    title: "Waveform",
    description: "A static waveform of a clip with a smooth playhead, seeking, hover time, regions and markers.",
    section: Section::Components,
    variants: &["default", "regions", "variants"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "regions" => regions(window, cx),
        "variants" => variants(cx),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// waveform-demo.tsx
// ---------------------------------------------------------------------------

/// `useAudioPlayer` stand-in: a clock with a smooth `time` source.
struct DemoPlayer {
    playing: bool,
    /// Position when last paused or seeked, in seconds.
    position: f64,
    /// Frame-clock time of the last play/seek, in ms.
    since_ms: f64,
    time: FrameEmitter<f64>,
}

impl DemoPlayer {
    fn current(&self, now: f64) -> f64 {
        if self.playing {
            self.position + (now - self.since_ms) / 1000.0
        } else {
            self.position
        }
    }

    fn toggle(&mut self, now: f64, duration: f64) {
        self.position = self.current(now);
        if !self.playing && self.position >= duration {
            self.position = 0.0;
        }
        self.since_ms = now;
        self.playing = !self.playing;
        self.time.emit(self.position);
    }

    fn seek(&mut self, value: f64, now: f64) {
        self.position = value;
        self.since_ms = now;
        self.time.emit(value);
    }
}

fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("waveform", "default");
    let (peaks, duration) = night_drive();
    let player = window.use_keyed_state(format!("{base}/player"), cx, |_, _| DemoPlayer {
        playing: false,
        position: 0.0,
        since_ms: 0.0,
        time: create_frame_emitter(),
    });
    let now = now_ms();
    let (playing, time) = player.update(cx, |p, _| {
        if p.playing {
            let at = p.current(now);
            if at >= duration {
                // `ended`: stop at the end.
                p.playing = false;
                p.position = duration;
                p.time.emit(duration);
            } else {
                p.time.emit(at);
            }
        }
        (p.playing, p.time.source())
    });
    if playing {
        window.request_animation_frame();
    }
    let seek_player = player.clone();
    let toggle_player = player;

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .w_full()
        .max_w(px(512.0))
        .child(
            Waveform::new(format!("{base}/waveform"))
                .aria_label("Night Drive")
                .h(px(80.0))
                .duration(duration)
                .loading(false)
                .on_seek_committed(move |value, _, cx| {
                    seek_player.update(cx, |p, cx| {
                        p.seek(value, now_ms());
                        cx.notify();
                    });
                })
                .peaks(peaks)
                .time(time)
                .child(WaveformCanvas::new())
                .child(WaveformCursor::new())
                .child(WaveformHover::new()),
        )
        .child(
            Button::new(format!("{base}/play"))
                .self_start()
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    toggle_player.update(cx, |p, cx| {
                        p.toggle(now_ms(), duration);
                        cx.notify();
                    });
                })
                .child(if playing { "Pause" } else { "Play" }),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// waveform-regions.tsx
// ---------------------------------------------------------------------------

fn region_peaks() -> Arc<[f32]> {
    static PEAKS: OnceLock<Arc<[f32]>> = OnceLock::new();
    PEAKS
        .get_or_init(|| {
            (0..320)
                .map(|index| {
                    let i = index as f64;
                    let bump = if index % 40 < 4 { 0.3 } else { 0.0 };
                    (0.2 + (i * 0.21).sin().abs() * 0.6 + bump).min(1.0) as f32
                })
                .collect()
        })
        .clone()
}

fn regions(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("waveform", "regions");
    let region = window.use_keyed_state(format!("{base}/region"), cx, |_, _| WaveformRegionValue {
        start: 9.0,
        end: 22.0,
    });
    let value = *region.read(cx);
    let t = cx.theme();
    let clip = format!(
        "Clip {} – {}",
        format_time(value.start, FormatTimeOptions::default()),
        format_time(value.end, FormatTimeOptions::default())
    );
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .w_full()
        .max_w(px(512.0))
        .child(
            Waveform::new(format!("{base}/waveform"))
                .aria_label("Episode")
                .h(px(80.0))
                .duration(60.0)
                .peaks(region_peaks())
                .variant(WaveformVariant::Mirror)
                .child(WaveformCanvas::new())
                .child(
                    WaveformRegion::new(format!("{base}/clip"), value.start, value.end)
                        .on_value_change(move |next, _, cx| {
                            region.update(cx, |r, cx| {
                                *r = next;
                                cx.notify();
                            });
                        }),
                )
                .child(WaveformMarker::new(40.0).child("Intro ends"))
                .child(WaveformCursor::new()),
        )
        .child(
            div()
                .text_color(t.muted_foreground)
                .font_family(t.font_mono.clone())
                .text_size(px(12.0))
                .line_height(px(16.0))
                .child(clip),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// waveform-variants.tsx
// ---------------------------------------------------------------------------

fn variant_peaks() -> Arc<[f32]> {
    static PEAKS: OnceLock<Arc<[f32]>> = OnceLock::new();
    PEAKS
        .get_or_init(|| {
            (0..240)
                .map(|index| {
                    let i = index as f64;
                    let envelope = ((i / 240.0) * std::f64::consts::PI).sin().powf(0.6);
                    let detail = 0.55 + 0.45 * ((i * 0.37).sin() * (i * 0.11).cos()).abs();
                    (envelope * detail) as f32
                })
                .collect()
        })
        .clone()
}

fn variants(cx: &mut App) -> AnyElement {
    let base = id("waveform", "variants");
    let t = cx.theme();
    let rows = [
        ("bars", WaveformVariant::Bars),
        ("mirror", WaveformVariant::Mirror),
        ("line", WaveformVariant::Line),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .w_full()
        .max_w(px(512.0))
        .children(rows.into_iter().map(|(name, variant)| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_color(t.muted_foreground)
                        .font_family(t.font_mono.clone())
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(name),
                )
                .child(
                    Waveform::new(format!("{base}/{name}"))
                        .aria_label(format!("{name} waveform"))
                        .h(px(56.0))
                        .default_current_time(12.0)
                        .duration(30.0)
                        .peaks(variant_peaks())
                        .variant(variant),
                )
        }))
        .into_any_element()
}

// ---------------------------------------------------------------------------
// "Night Drive" (lib/docs/demo-audio.ts), rendered offline
// ---------------------------------------------------------------------------

const SAMPLE_RATE: f64 = 22_050.0;
const NIGHT_DRIVE_SECONDS: f64 = 19.2;
const PEAK_COUNT: usize = 512;

/// Peaks and duration of the demo track, rendered once.
fn night_drive() -> (Arc<[f32]>, f64) {
    static TRACK: OnceLock<Arc<[f32]>> = OnceLock::new();
    let peaks = TRACK
        .get_or_init(|| {
            let samples = render_night_drive();
            compute_peaks(&[samples], PEAK_COUNT).into()
        })
        .clone();
    (peaks, NIGHT_DRIVE_SECONDS)
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Sawtooth,
    Triangle,
}

#[derive(Clone, Copy)]
enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
}

/// A Web Audio `BiquadFilterNode` (Audio EQ Cookbook, default Q = 1).
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
        // Lowpass/highpass take Q in dB; bandpass takes it as is.
        let alpha = match kind {
            FilterKind::Lowpass | FilterKind::Highpass => sin / (2.0 * 10f64.powf(1.0 / 20.0)),
            FilterKind::Bandpass => sin / 2.0,
        };
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

/// `envelope()`: 0.0001 → `peak` over `attack`, → 0.0001 over `decay`, as
/// exponential ramps, stepped per sample.
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

fn index_at(seconds: f64) -> usize {
    (seconds * SAMPLE_RATE).round().max(0.0) as usize
}

/// `tone()`: an oscillator through an envelope, added into `out`.
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
            Some(end) if (step as f64) < ramp => {
                tone.frequency * (end / tone.frequency).powf(step as f64 / ramp)
            }
            Some(end) => end,
            None => tone.frequency,
        } * detune;
        let sample = match tone.wave {
            Wave::Sine => (TAU * phase).sin(),
            Wave::Sawtooth => 2.0 * (phase - (phase + 0.5).floor()),
            Wave::Triangle => 1.0 - 4.0 * ((phase + 0.25).fract() - 0.5).abs(),
        };
        *slot += (sample * envelope.next()) as f32;
        phase = (phase + frequency / SAMPLE_RATE).fract();
    }
}

/// `noiseHit()`: seeded noise through a biquad and an envelope.
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

fn render_night_drive() -> Vec<f32> {
    let mut out = vec![0.0f32; index_at(NIGHT_DRIVE_SECONDS)];
    let bpm = 100.0;
    let beat = 60.0 / bpm;

    // chordProgression: detuned saws through a 1.8 kHz lowpass, plus a sine bass.
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
    let peak = 0.025;
    let beats_per_chord = 4.0;
    let mut pad = vec![0.0f32; out.len()];
    for (index, chord) in chords.iter().enumerate() {
        let start = index as f64 * beats_per_chord * beat;
        for &note in chord {
            for detune in [-7.0, 7.0] {
                tone(
                    &mut pad,
                    &Tone {
                        wave: Wave::Sawtooth,
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
            &mut out,
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

    // drumPattern: 8 bars of kick, snare and hats.
    for bar in 0..8 {
        for step in 0..8 {
            let time = (f64::from(bar) * 4.0 + f64::from(step) / 2.0) * beat;
            if step == 0 || step == 5 {
                tone(
                    &mut out,
                    &Tone {
                        wave: Wave::Sine,
                        frequency: 150.0,
                        end_frequency: Some(40.0),
                        detune: 0.0,
                        start: time,
                        peak: 0.9,
                        attack: 0.002,
                        decay: 0.35,
                    },
                );
            }
            if step == 2 || step == 6 {
                noise_hit(&mut out, time, 0.4, 0.18, FilterKind::Bandpass, 1800.0);
                tone(
                    &mut out,
                    &Tone {
                        wave: Wave::Triangle,
                        frequency: 190.0,
                        end_frequency: None,
                        detune: 0.0,
                        start: time,
                        peak: 0.4 * 0.6,
                        attack: 0.002,
                        decay: 0.12,
                    },
                );
            }
            let hat = if step % 2 == 0 { 0.12 } else { 0.07 };
            noise_hit(&mut out, time, hat, 0.05, FilterKind::Highpass, 7000.0);
        }
    }
    out
}
