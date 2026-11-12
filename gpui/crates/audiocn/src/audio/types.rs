//! Port of `lib/audio/types.ts`.

/// A level reading for one audio channel, in dBFS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelLevel {
    /// Sample peak in dBFS. `f32::NEG_INFINITY` is digital silence.
    pub peak_db: f32,
    /// RMS level in dBFS, when the source measures it.
    pub rms_db: Option<f32>,
}

impl ChannelLevel {
    pub const SILENT: Self = Self {
        peak_db: f32::NEG_INFINITY,
        rms_db: None,
    };

    pub fn new(peak_db: f32) -> Self {
        Self {
            peak_db,
            rms_db: None,
        }
    }

    pub fn with_rms(peak_db: f32, rms_db: f32) -> Self {
        Self {
            peak_db,
            rms_db: Some(rms_db),
        }
    }
}

/// One metering update: one entry per channel (1 = mono, 2 = stereo).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeterFrame {
    pub channels: Vec<ChannelLevel>,
}

impl MeterFrame {
    pub fn mono(level: ChannelLevel) -> Self {
        Self {
            channels: vec![level],
        }
    }

    pub fn stereo(left: ChannelLevel, right: ChannelLevel) -> Self {
        Self {
            channels: vec![left, right],
        }
    }
}

/// One visualisation update from an analyser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualFrame {
    /// Frequency bands, 0..1 each, low to high.
    pub bands: Vec<f32>,
    /// Ring buffer of recent broadband levels, 0..1 each.
    pub history: Vec<f32>,
    /// Index of the oldest entry in `history`.
    pub history_start: usize,
    /// Number of valid entries in `history`.
    pub history_length: usize,
    /// Time of the newest history entry, in milliseconds on the frame clock
    /// (see [`crate::audio::frame_loop::now_ms`]).
    pub history_updated_at: Option<f64>,
    /// Minimum time between entries, used for playback delay. 0 disables smoothing.
    pub history_interval_ms: Option<f64>,
    /// Last entry removed from a full history ring, for its scrolling edge.
    pub history_previous_level: Option<f32>,
    /// Raw time-domain samples, −1..1, for line and scope drawing.
    pub time_domain: Option<Vec<f32>>,
    /// Sample peak of this frame in dBFS.
    pub peak_db: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeterZoneName {
    Ok,
    Warn,
    Clip,
}

/// A colour zone of a meter, starting at `from_db` and running to the next zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterZone {
    pub from_db: f32,
    pub zone: MeterZoneName,
}

/// Maps a value (usually dB) to a 0..1 position on a control, and back.
pub trait Taper: Send + Sync {
    fn to_position(&self, value: f32) -> f32;
    fn to_value(&self, position: f32) -> f32;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}
