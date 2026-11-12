//! The signal source picker: what meters and visualizers in the playground read.

use audiocn::audio::decibels::{DEFAULT_MAX_DB, DEFAULT_MIN_DB, db_to_level};
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::{ChannelLevel, FrameEmitter, FrameSource, MeterFrame, VisualFrame};
use gpui::{App, Window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Speech,
    Music,
    Tone,
    Noise,
    Silence,
    /// Fixed peak / RMS levels from the panel's sliders.
    Static,
    /// The real default microphone (gallery `device` feature, native only).
    Microphone,
    /// No source at all (components show their idle state).
    Off,
}

impl SourceKind {
    pub fn all() -> Vec<SourceKind> {
        let mut all = vec![
            Self::Speech,
            Self::Music,
            Self::Tone,
            Self::Noise,
            Self::Silence,
            Self::Static,
        ];
        if cfg!(all(feature = "device", not(target_family = "wasm"))) {
            all.push(Self::Microphone);
        }
        all.push(Self::Off);
        all
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Speech => "speech",
            Self::Music => "music",
            Self::Tone => "tone",
            Self::Noise => "noise",
            Self::Silence => "silence",
            Self::Static => "static",
            Self::Microphone => "microphone",
            Self::Off => "off",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Speech => "Demo: speech",
            Self::Music => "Demo: music",
            Self::Tone => "Demo: tone",
            Self::Noise => "Demo: noise",
            Self::Silence => "Demo: silence",
            Self::Static => "Static level",
            Self::Microphone => "Microphone (real)",
            Self::Off => "No source",
        }
    }

    pub fn from_key(key: &str) -> Self {
        Self::all()
            .into_iter()
            .find(|k| k.key() == key)
            .unwrap_or(Self::Speech)
    }
}

/// The source panel's settings.
#[derive(Clone, Debug)]
pub struct SourceSettings {
    pub kind: SourceKind,
    pub channels: usize,
    pub static_peak_db: f32,
    pub static_rms_db: f32,
    pub gain_db: f32,
}

impl Default for SourceSettings {
    fn default() -> Self {
        Self {
            kind: SourceKind::Speech,
            channels: 2,
            static_peak_db: -12.0,
            static_rms_db: -18.0,
            gain_db: 0.0,
        }
    }
}

/// Emitters for the static source, kept by the playground.
pub struct StaticEmitters {
    pub meter: FrameEmitter<MeterFrame>,
    pub visual: FrameEmitter<VisualFrame>,
}

impl Default for StaticEmitters {
    fn default() -> Self {
        Self {
            meter: FrameEmitter::new(),
            visual: FrameEmitter::new(),
        }
    }
}

const STATIC_BANDS: usize = 32;
const STATIC_HISTORY: usize = 60;
const STATIC_SAMPLES: usize = 1024;

/// Resolves the picked source to frame sources. Must be called every render
/// (the demo signal and microphone hooks keep their state in keyed element
/// state, so they stop when no longer called).
pub fn resolve(
    settings: &SourceSettings,
    emitters: &StaticEmitters,
    window: &mut Window,
    cx: &mut App,
) -> (
    Option<FrameSource<MeterFrame>>,
    Option<FrameSource<VisualFrame>>,
) {
    let demo = |kind: DemoSignalKind, window: &mut Window, cx: &mut App| {
        let options = DemoSignalOptions::kind(kind)
            .with_channels(settings.channels)
            .with_gain_db(settings.gain_db);
        let handle = use_demo_signal(format!("pg-source-{:?}", kind), options, window, cx);
        (Some(handle.meter), Some(handle.visual))
    };
    match settings.kind {
        SourceKind::Speech => demo(DemoSignalKind::Speech, window, cx),
        SourceKind::Music => demo(DemoSignalKind::Music, window, cx),
        SourceKind::Tone => demo(DemoSignalKind::Tone, window, cx),
        SourceKind::Noise => demo(DemoSignalKind::Noise, window, cx),
        SourceKind::Silence => demo(DemoSignalKind::Silence, window, cx),
        SourceKind::Static => {
            emit_static(settings, emitters);
            (
                Some(emitters.meter.source()),
                Some(emitters.visual.source()),
            )
        }
        SourceKind::Microphone => microphone(window, cx),
        SourceKind::Off => (None, None),
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
fn microphone(
    window: &mut Window,
    cx: &mut App,
) -> (
    Option<FrameSource<MeterFrame>>,
    Option<FrameSource<VisualFrame>>,
) {
    use audiocn::io::microphone::{MicrophoneOptions, use_microphone};
    let mic = use_microphone(
        "pg-source-microphone",
        MicrophoneOptions {
            enabled: true,
            ..Default::default()
        },
        window,
        cx,
    );
    (Some(mic.meter), Some(mic.visual))
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
fn microphone(
    _window: &mut Window,
    _cx: &mut App,
) -> (
    Option<FrameSource<MeterFrame>>,
    Option<FrameSource<VisualFrame>>,
) {
    (None, None)
}

fn emit_static(settings: &SourceSettings, emitters: &StaticEmitters) {
    let peak = settings.static_peak_db;
    let rms = settings.static_rms_db.min(peak);
    let level = db_to_level(peak, DEFAULT_MIN_DB, DEFAULT_MAX_DB).clamp(0.0, 1.0);
    let channels = (0..settings.channels.max(1))
        .map(|_| ChannelLevel::with_rms(peak, rms))
        .collect();
    emitters.meter.emit(MeterFrame { channels });

    let amplitude = audiocn::audio::decibels::db_to_gain(peak);
    let time_domain = (0..STATIC_SAMPLES)
        .map(|i| amplitude * (i as f32 / STATIC_SAMPLES as f32 * std::f32::consts::TAU * 4.0).sin())
        .collect();
    emitters.visual.emit(VisualFrame {
        bands: vec![level; STATIC_BANDS],
        history: vec![level; STATIC_HISTORY],
        history_start: 0,
        history_length: STATIC_HISTORY,
        history_updated_at: None,
        history_interval_ms: None,
        history_previous_level: None,
        time_domain: Some(time_domain),
        peak_db: peak,
    });
}
