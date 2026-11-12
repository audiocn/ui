//! Port of `hooks/use-audio-analyser.ts` (`createAnalyserTap`).
//!
//! [`AnalyserNode`] is the Web Audio node, re-implemented to the spec:
//! a ring of the last `fft_size` samples, a Blackman window, an FFT, the
//! `smoothingTimeConstant` average of magnitudes and dB conversion
//! (`getFloatTimeDomainData` / `getFloatFrequencyData`).
//! [`AnalyserCore`] turns analysers into the same [`MeterFrame`] (peak/RMS
//! per channel) and [`VisualFrame`] (log bands, level history, time-domain
//! samples) as React's tap, and is pure (unit-tested with synthetic signals).
//! [`AnalyserTap`] runs a core off the audio thread: the audio callback
//! [`write`](TapWriter::write)s samples, a shared analysis thread makes frames
//! about 60 times a second (React: every animation frame) and emits them.
//!
//! ```ignore
//! let tap = AnalyserTap::new(AnalyserTapOptions::default(), 48_000);
//! let writer = tap.writer();          // move into the audio callback
//! LevelMeter::new("m").source(tap.meter());
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use super::context::AudioSource;
use crate::audio::bands::{
    BandsFromSpectrumOptions, SPECTRUM_MAX_DB, SPECTRUM_MIN_DB, bands_from_spectrum, log_band_edges,
};
use crate::audio::decibels::{DEFAULT_MAX_DB, DEFAULT_MIN_DB, db_to_level, peak_db, rms_db};
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
use crate::audio::history::append_history;
use crate::audio::types::{ChannelLevel, MeterFrame, VisualFrame};

/// `AnalyserNode.fftSize` default.
pub const DEFAULT_FFT_SIZE: usize = 2048;
/// `AnalyserNode.smoothingTimeConstant` default (the node's, not the tap's).
pub const ANALYSER_NODE_SMOOTHING: f32 = 0.8;
/// The analysis thread's period (React: one animation frame).
pub const ANALYSIS_INTERVAL: Duration = Duration::from_micros(16_667);

/// `channels` of [`AnalyserTapOptions`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AnalyserChannels {
    /// One meter channel from the down-mix. Default.
    #[default]
    Mono,
    /// Left and right measured separately.
    Stereo,
}

/// `AnalyserTapOptions`, same defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalyserTapOptions {
    /// Analyser FFT size (a power of two, 32..32768). Default 2048.
    pub fft_size: usize,
    /// Analyser smoothing constant, 0..1. Default 0.3.
    pub smoothing: f32,
    /// Frequency bands per visual frame. Default 32.
    pub bands: usize,
    /// Lowest band frequency. Default 40 Hz.
    pub min_hz: f32,
    /// Highest band frequency. Default 16 kHz.
    pub max_hz: f32,
    /// Entries in the level history ring. Default 60.
    pub history_size: usize,
    /// Time between history entries. Default 50 ms.
    pub history_interval_ms: f64,
    /// Minimum time between frames. 0 sends a frame every analysis tick.
    pub interval_ms: f64,
    /// Default `Mono`.
    pub channels: AnalyserChannels,
}

impl Default for AnalyserTapOptions {
    fn default() -> Self {
        Self {
            fft_size: DEFAULT_FFT_SIZE,
            smoothing: 0.3,
            bands: 32,
            min_hz: 40.0,
            max_hz: 16_000.0,
            history_size: 60,
            history_interval_ms: 50.0,
            interval_ms: 0.0,
            channels: AnalyserChannels::Mono,
        }
    }
}

/// `AudioAnalyserStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioAnalyserStatus {
    #[default]
    Idle,
    Running,
    Suspended,
}

// ---------------------------------------------------------------------------
// AnalyserNode
// ---------------------------------------------------------------------------

/// A Web Audio `AnalyserNode`: feed it samples, read time-domain and
/// frequency data exactly as the browser computes them.
pub struct AnalyserNode {
    fft_size: usize,
    /// `smoothingTimeConstant`, 0..1.
    pub smoothing_time_constant: f32,
    /// `minDecibels` (for byte data and bands). Default −100.
    pub min_decibels: f32,
    /// `maxDecibels`. Default −30.
    pub max_decibels: f32,
    ring: Vec<f32>,
    write: usize,
    smoothed: Vec<f32>,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

/// The spec's Blackman window (α = 0.16).
fn blackman(size: usize) -> Vec<f32> {
    const ALPHA: f64 = 0.16;
    let a0 = (1.0 - ALPHA) / 2.0;
    let a1 = 0.5;
    let a2 = ALPHA / 2.0;
    (0..size)
        .map(|n| {
            let x = std::f64::consts::TAU * n as f64 / size as f64;
            (a0 - a1 * x.cos() + a2 * (2.0 * x).cos()) as f32
        })
        .collect()
}

impl AnalyserNode {
    /// `fft_size` is rounded to a power of two in 32..=32768.
    pub fn new(fft_size: usize, smoothing_time_constant: f32) -> Self {
        let fft_size = fft_size.clamp(32, 32_768).next_power_of_two();
        let fft = FftPlanner::new().plan_fft_forward(fft_size);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Self {
            fft_size,
            smoothing_time_constant: smoothing_time_constant.clamp(0.0, 1.0),
            min_decibels: SPECTRUM_MIN_DB,
            max_decibels: SPECTRUM_MAX_DB,
            ring: vec![0.0; fft_size],
            write: 0,
            smoothed: vec![0.0; fft_size / 2],
            window: blackman(fft_size),
            fft,
            buffer: vec![Complex::default(); fft_size],
            scratch,
        }
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// `frequencyBinCount`: `fft_size / 2`.
    pub fn frequency_bin_count(&self) -> usize {
        self.fft_size / 2
    }

    /// Feeds mono samples (oldest first).
    pub fn push(&mut self, samples: &[f32]) {
        let skip = samples.len().saturating_sub(self.fft_size);
        for &sample in &samples[skip..] {
            self.ring[self.write] = sample;
            self.write = (self.write + 1) % self.fft_size;
        }
    }

    /// `getFloatTimeDomainData`: the last `out.len()` (≤ `fft_size`) samples.
    pub fn get_float_time_domain_data(&self, out: &mut [f32]) {
        let count = out.len().min(self.fft_size);
        let start = self.write + self.fft_size - count;
        for (index, slot) in out.iter_mut().take(count).enumerate() {
            *slot = self.ring[(start + index) % self.fft_size];
        }
    }

    /// `getFloatFrequencyData`: smoothed magnitude in dB per bin. Like the
    /// browser, each call advances the smoothing.
    pub fn get_float_frequency_data(&mut self, out: &mut [f32]) {
        let size = self.fft_size;
        for index in 0..size {
            let sample = self.ring[(self.write + index) % size];
            self.buffer[index] = Complex::new(sample * self.window[index], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let tau = self.smoothing_time_constant;
        let scale = 1.0 / size as f32;
        for (bin, smoothed) in self.smoothed.iter_mut().enumerate() {
            let magnitude = self.buffer[bin].norm() * scale;
            let mut value = tau * *smoothed + (1.0 - tau) * magnitude;
            if !value.is_finite() {
                value = 0.0;
            }
            *smoothed = value;
        }
        for (slot, value) in out.iter_mut().zip(&self.smoothed) {
            *slot = if *value > 0.0 {
                20.0 * value.log10()
            } else {
                f32::NEG_INFINITY
            };
        }
    }
}

// ---------------------------------------------------------------------------
// AnalyserCore: analysers → frames
// ---------------------------------------------------------------------------

/// The tap's analysis without threads: feed interleaved samples, call
/// [`tick`](Self::tick) once per frame.
pub struct AnalyserCore {
    options: AnalyserTapOptions,
    sample_rate: u32,
    mix: AnalyserNode,
    sides: Vec<AnalyserNode>,
    edges: Vec<f32>,
    spectrum: Vec<f32>,
    time_domain: Vec<f32>,
    side_time_domain: Vec<f32>,
    visual: VisualFrame,
    last_frame_ms: f64,
    mono: Vec<f32>,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl AnalyserCore {
    pub fn new(options: AnalyserTapOptions, sample_rate: u32) -> Self {
        let mix = AnalyserNode::new(options.fft_size, options.smoothing);
        let fft_size = mix.fft_size();
        let sides = match options.channels {
            AnalyserChannels::Mono => Vec::new(),
            AnalyserChannels::Stereo => (0..2)
                .map(|_| AnalyserNode::new(fft_size, options.smoothing))
                .collect(),
        };
        let edges = log_band_edges(
            options.bands,
            options.min_hz,
            options.max_hz.min(sample_rate as f32 / 2.0),
        );
        Self {
            sample_rate,
            spectrum: vec![0.0; mix.frequency_bin_count()],
            time_domain: vec![0.0; fft_size],
            side_time_domain: vec![0.0; fft_size],
            visual: VisualFrame {
                bands: vec![0.0; options.bands],
                history: vec![0.0; options.history_size],
                history_length: 0,
                history_start: 0,
                peak_db: f32::NEG_INFINITY,
                ..Default::default()
            },
            last_frame_ms: 0.0,
            mix,
            sides,
            edges,
            options,
            mono: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    pub fn options(&self) -> &AnalyserTapOptions {
        &self.options
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Feeds interleaved samples with `channels` channels. The mix analyser
    /// gets the speakers down-mix (`(L + R) / 2`); in stereo mode the side
    /// analysers get left and right (a mono input feeds both).
    pub fn push_interleaved(&mut self, samples: &[f32], channels: usize) {
        let channels = channels.max(1);
        self.mono.clear();
        self.left.clear();
        self.right.clear();
        let stereo = !self.sides.is_empty();
        for frame in samples.chunks_exact(channels) {
            let left = frame[0];
            let right = if channels > 1 { frame[1] } else { left };
            self.mono.push(if channels > 1 {
                (left + right) * 0.5
            } else {
                left
            });
            if stereo {
                self.left.push(left);
                self.right.push(right);
            }
        }
        self.mix.push(&self.mono);
        if stereo {
            self.sides[0].push(&self.left);
            self.sides[1].push(&self.right);
        }
    }

    /// One frame: the meter and visual frames, or `None` while `interval_ms`
    /// hasn't passed since the last one.
    pub fn tick(&mut self, now_ms: f64) -> Option<(MeterFrame, VisualFrame)> {
        if now_ms - self.last_frame_ms < self.options.interval_ms {
            return None;
        }
        self.last_frame_ms = now_ms;

        let mut meter = MeterFrame::default();
        if self.sides.is_empty() {
            self.mix
                .get_float_time_domain_data(&mut self.side_time_domain);
            meter.channels.push(level_of(&self.side_time_domain));
        } else {
            for analyser in &self.sides {
                analyser.get_float_time_domain_data(&mut self.side_time_domain);
                meter.channels.push(level_of(&self.side_time_domain));
            }
        }

        self.mix.get_float_time_domain_data(&mut self.time_domain);
        self.mix.get_float_frequency_data(&mut self.spectrum);
        bands_from_spectrum(
            &self.spectrum,
            self.sample_rate as f32,
            &self.edges,
            &mut self.visual.bands,
            BandsFromSpectrumOptions {
                min_db: self.mix.min_decibels,
                max_db: self.mix.max_decibels,
            },
        );
        self.visual.peak_db = peak_db(&self.time_domain);
        let level = db_to_level(self.visual.peak_db, DEFAULT_MIN_DB, DEFAULT_MAX_DB);
        append_history(
            &mut self.visual,
            level,
            now_ms,
            self.options.history_interval_ms,
        );
        let mut visual = self.visual.clone();
        visual.time_domain = Some(self.time_domain.clone());
        Some((meter, visual))
    }
}

fn level_of(samples: &[f32]) -> ChannelLevel {
    ChannelLevel::with_rms(peak_db(samples), rms_db(samples))
}

// ---------------------------------------------------------------------------
// AnalyserTap: a core on the shared analysis thread
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Pending {
    samples: Vec<f32>,
    channels: usize,
}

struct TapShared {
    pending: Mutex<Pending>,
    core: Mutex<AnalyserCore>,
    meter: FrameEmitter<MeterFrame>,
    visual: FrameEmitter<VisualFrame>,
    disposed: AtomicBool,
    capacity: usize,
}

impl TapShared {
    fn analyse(&self, now_ms: f64) {
        let pending = {
            let mut pending = self.pending.lock();
            let channels = pending.channels;
            (std::mem::take(&mut pending.samples), channels)
        };
        let mut core = self.core.lock();
        if !pending.0.is_empty() {
            core.push_interleaved(&pending.0, pending.1);
        }
        if let Some((meter, visual)) = core.tick(now_ms) {
            drop(core);
            self.meter.emit(meter);
            self.visual.emit(visual);
        }
    }
}

static TAPS: Mutex<Vec<Weak<TapShared>>> = Mutex::new(Vec::new());
static ANALYSIS_THREAD: OnceLock<()> = OnceLock::new();

fn register(tap: &Arc<TapShared>) {
    TAPS.lock().push(Arc::downgrade(tap));
    ANALYSIS_THREAD.get_or_init(|| {
        let spawned = thread::Builder::new()
            .name("audiocn-analysis".into())
            .spawn(analysis_loop);
        if let Err(error) = spawned {
            eprintln!("audiocn: could not start the analysis thread: {error}");
        }
    });
}

fn analysis_loop() {
    let mut next = std::time::Instant::now();
    loop {
        next += ANALYSIS_INTERVAL;
        let now = std::time::Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
        let taps: Vec<Arc<TapShared>> = {
            let mut taps = TAPS.lock();
            taps.retain(|tap| {
                tap.upgrade()
                    .is_some_and(|tap| !tap.disposed.load(Ordering::Acquire))
            });
            taps.iter().filter_map(Weak::upgrade).collect()
        };
        let now_ms = now_ms();
        for tap in taps {
            tap.analyse(now_ms);
        }
    }
}

/// `createAnalyserTap`: meter and visual frame sources for a signal the audio
/// thread writes through a [`TapWriter`]. Stops when dropped or disposed.
pub struct AnalyserTap {
    shared: Arc<TapShared>,
}

impl AnalyserTap {
    pub fn new(options: AnalyserTapOptions, sample_rate: u32) -> Self {
        Self::with_emitters(
            options,
            sample_rate,
            create_frame_emitter(),
            create_frame_emitter(),
        )
    }

    /// A tap that emits into existing emitters, so readers keep their
    /// sources when the input changes (`createFrameRelay`).
    pub fn with_emitters(
        options: AnalyserTapOptions,
        sample_rate: u32,
        meter: FrameEmitter<MeterFrame>,
        visual: FrameEmitter<VisualFrame>,
    ) -> Self {
        let core = AnalyserCore::new(options, sample_rate);
        let capacity = core.mix.fft_size() * 2 * 2;
        let shared = Arc::new(TapShared {
            pending: Mutex::new(Pending::default()),
            core: Mutex::new(core),
            meter,
            visual,
            disposed: AtomicBool::new(false),
            capacity,
        });
        register(&shared);
        Self { shared }
    }

    pub fn meter(&self) -> FrameSource<MeterFrame> {
        self.shared.meter.source()
    }

    pub fn visual(&self) -> FrameSource<VisualFrame> {
        self.shared.visual.source()
    }

    /// The write end, for the audio callback.
    pub fn writer(&self) -> TapWriter {
        TapWriter {
            shared: self.shared.clone(),
        }
    }

    /// Wraps a source so everything it plays is analysed on the way through.
    pub fn wrap(&self, source: impl AudioSource) -> TappedSource {
        TappedSource {
            inner: Box::new(source),
            writer: self.writer(),
            scratch: Vec::new(),
        }
    }

    /// Stops analysing (`dispose`). The sources keep their last frame.
    pub fn dispose(&self) {
        self.shared.disposed.store(true, Ordering::Release);
    }

    pub fn is_disposed(&self) -> bool {
        self.shared.disposed.load(Ordering::Acquire)
    }
}

impl Drop for AnalyserTap {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Feeds an [`AnalyserTap`] from the audio thread. Cheap to clone.
#[derive(Clone)]
pub struct TapWriter {
    shared: Arc<TapShared>,
}

impl TapWriter {
    /// Queues interleaved samples (`channels` per frame) for the next frame.
    pub fn write(&self, samples: &[f32], channels: usize) {
        if self.shared.disposed.load(Ordering::Relaxed) {
            return;
        }
        let mut pending = self.shared.pending.lock();
        if pending.channels != channels {
            pending.samples.clear();
            pending.channels = channels;
        }
        pending.samples.extend_from_slice(samples);
        let capacity = self.shared.capacity * channels.max(1);
        let excess = pending.samples.len().saturating_sub(capacity);
        if excess > 0 {
            // Only the newest fft_size frames matter to the next frame.
            let excess = excess.div_ceil(channels.max(1)) * channels.max(1);
            let excess = excess.min(pending.samples.len());
            pending.samples.drain(..excess);
        }
    }
}

/// A source analysed on its way to the bus (see [`AnalyserTap::wrap`]).
pub struct TappedSource {
    inner: Box<dyn AudioSource>,
    writer: TapWriter,
    scratch: Vec<f32>,
}

impl AudioSource for TappedSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        self.scratch.clear();
        self.scratch.resize(out.len(), 0.0);
        let alive = self.inner.render(&mut self.scratch, sample_rate);
        self.writer.write(&self.scratch, 2);
        for (out, sample) in out.iter_mut().zip(&self.scratch) {
            *out += sample;
        }
        alive
    }

    fn channel_count(&self) -> usize {
        self.inner.channel_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(frequency: f32, amplitude: f32, count: usize) -> Vec<f32> {
        (0..count)
            .map(|n| amplitude * (std::f32::consts::TAU * frequency * n as f32 / RATE as f32).sin())
            .collect()
    }

    fn noise(amplitude: f32, count: usize) -> Vec<f32> {
        let mut seed = 0x2545_f491_u32;
        (0..count)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                amplitude * ((seed as f32 / u32::MAX as f32) * 2.0 - 1.0)
            })
            .collect()
    }

    #[test]
    fn time_domain_is_the_latest_fft_size_samples() {
        let mut node = AnalyserNode::new(32, 0.0);
        let samples: Vec<f32> = (0..40).map(|n| n as f32).collect();
        node.push(&samples);
        let mut out = [0.0f32; 32];
        node.get_float_time_domain_data(&mut out);
        assert_eq!(out[0], 8.0);
        assert_eq!(out[31], 39.0);
    }

    #[test]
    fn fft_size_rounds_to_a_power_of_two() {
        assert_eq!(AnalyserNode::new(1000, 0.8).fft_size(), 1024);
        assert_eq!(AnalyserNode::new(1, 0.8).fft_size(), 32);
    }

    #[test]
    fn full_scale_sine_peaks_at_its_bin() {
        let mut node = AnalyserNode::new(2048, 0.0);
        // Bin-centred: 64 cycles per 2048 samples.
        let frequency = 64.0 * RATE as f32 / 2048.0;
        node.push(&sine(frequency, 1.0, 2048));
        let mut spectrum = vec![0.0f32; 1024];
        node.get_float_frequency_data(&mut spectrum);
        let loudest = spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(bin, _)| bin);
        assert_eq!(loudest, Some(64));
        // |X| / N of a unit sine through a Blackman window: 0.5 · a0 = 0.21 → −13.6 dB.
        assert!((spectrum[64] - 20.0 * (0.21f32).log10()).abs() < 0.1);
        assert!(spectrum[400] < -100.0);
    }

    #[test]
    fn smoothing_averages_successive_frames() {
        let mut node = AnalyserNode::new(256, 0.5);
        let frequency = 8.0 * RATE as f32 / 256.0;
        node.push(&sine(frequency, 1.0, 256));
        let mut first = vec![0.0f32; 128];
        node.get_float_frequency_data(&mut first);
        node.push(&vec![0.0; 256]);
        let mut second = vec![0.0f32; 128];
        node.get_float_frequency_data(&mut second);
        // Half the magnitude: −6 dB.
        assert!((first[8] - second[8] - 6.02).abs() < 0.05);
    }

    #[test]
    fn meters_peak_and_rms_of_a_sine() {
        let mut core = AnalyserCore::new(AnalyserTapOptions::default(), RATE);
        core.push_interleaved(&sine(1000.0, 0.5, 4096), 1);
        let (meter, visual) = core.tick(50.0).expect("a frame");
        assert_eq!(meter.channels.len(), 1);
        let level = meter.channels[0];
        assert!((level.peak_db - (-6.02)).abs() < 0.05, "{}", level.peak_db);
        assert!((level.rms_db.unwrap() - (-9.03)).abs() < 0.1);
        assert!((visual.peak_db - level.peak_db).abs() < 1e-6);
        assert_eq!(visual.time_domain.as_ref().map(Vec::len), Some(2048));
        assert_eq!(visual.history_length, 1);
        assert_eq!(visual.bands.len(), 32);
    }

    #[test]
    fn sine_lights_the_band_that_contains_it() {
        let options = AnalyserTapOptions {
            smoothing: 0.0,
            ..Default::default()
        };
        let mut core = AnalyserCore::new(options, RATE);
        core.push_interleaved(&sine(1000.0, 0.9, 4096), 1);
        let (_, visual) = core.tick(16.0).unwrap();
        let edges = log_band_edges(32, 40.0, 16_000.0);
        let band = edges
            .windows(2)
            .position(|w| w[0] <= 1000.0 && 1000.0 < w[1])
            .unwrap();
        let loudest = visual
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap();
        assert!(loudest.abs_diff(band) <= 1, "{loudest} vs {band}");
        assert!(visual.bands[0] < 0.2 && visual.bands[31] < 0.2);
    }

    #[test]
    fn white_noise_fills_every_band() {
        let mut core = AnalyserCore::new(AnalyserTapOptions::default(), RATE);
        core.push_interleaved(&noise(0.5, 4096), 1);
        let (meter, visual) = core.tick(16.0).unwrap();
        assert!(
            visual.bands.iter().all(|band| *band > 0.3),
            "{:?}",
            visual.bands
        );
        let level = meter.channels[0];
        // Uniform noise: RMS = A / √3 → −10.8 dBFS for A = 0.5.
        assert!((level.rms_db.unwrap() - (-10.79)).abs() < 0.5);
    }

    #[test]
    fn stereo_meters_left_and_right() {
        let options = AnalyserTapOptions {
            channels: AnalyserChannels::Stereo,
            ..Default::default()
        };
        let mut core = AnalyserCore::new(options, RATE);
        let left = sine(440.0, 1.0, 4096);
        let interleaved: Vec<f32> = left.iter().flat_map(|s| [*s, s * 0.1]).collect();
        core.push_interleaved(&interleaved, 2);
        let (meter, visual) = core.tick(16.0).unwrap();
        assert_eq!(meter.channels.len(), 2);
        assert!(meter.channels[0].peak_db > -0.1);
        assert!((meter.channels[1].peak_db - (-20.0)).abs() < 0.1);
        // The visual frame is the down-mix: (1 + 0.1) / 2.
        assert!((visual.peak_db - 20.0 * 0.55f32.log10()).abs() < 0.1);
    }

    #[test]
    fn silence_is_minus_infinity() {
        let mut core = AnalyserCore::new(AnalyserTapOptions::default(), RATE);
        let (meter, visual) = core.tick(16.0).unwrap();
        assert_eq!(meter.channels[0].peak_db, f32::NEG_INFINITY);
        assert!(visual.bands.iter().all(|band| *band == 0.0));
    }

    #[test]
    fn interval_throttles_frames_and_history_follows_its_interval() {
        let options = AnalyserTapOptions {
            interval_ms: 30.0,
            ..Default::default()
        };
        let mut core = AnalyserCore::new(options, RATE);
        assert!(core.tick(16.0).is_none());
        assert!(core.tick(32.0).is_some());
        assert!(core.tick(48.0).is_none());
        let (_, visual) = core.tick(100.0).unwrap();
        assert_eq!(visual.history_length, 1);
        assert_eq!(visual.history_interval_ms, Some(50.0));
    }

    #[test]
    fn writer_bounds_pending_samples() {
        let tap = AnalyserTap::new(
            AnalyserTapOptions {
                fft_size: 32,
                ..Default::default()
            },
            RATE,
        );
        let writer = tap.writer();
        for _ in 0..100 {
            writer.write(&[0.5; 64], 2);
        }
        assert!(tap.shared.pending.lock().samples.len() <= 32 * 2 * 2 * 2);
        tap.shared.analyse(10.0);
        let meter = tap.meter().latest().expect("a frame");
        assert!((meter.channels[0].peak_db - (-6.02)).abs() < 0.05);
    }
}
