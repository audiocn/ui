//! Port of `hooks/use-level.ts`: reads a meter source at a low rate (default
//! every 250 ms) for labels and conditional UI, not for drawing meters.
//!
//! [`LevelSampler`] is the plain state; [`use_level`] keeps one in GPUI keyed
//! state and re-renders the calling view only when the sampled level changes.

use std::time::Duration;

use gpui::{App, ElementId, Task, Window};

use crate::audio::frame_source::FrameSource;
use crate::audio::types::{ChannelLevel, MeterFrame, MeterZone, MeterZoneName};
use crate::audio::zones::{DEFAULT_ZONES, zone_for_db};

const DEFAULT_INTERVAL_MS: f64 = 250.0;

/// Which channel a level reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LevelChannel {
    /// The loudest channel (by peak). The default.
    #[default]
    Max,
    /// One channel by index.
    Index(usize),
}

/// Options for [`use_level`] / [`LevelSampler`] (`UseLevelOptions`).
#[derive(Clone, Debug, PartialEq)]
pub struct LevelOptions {
    /// How often the level updates. Default 250 ms.
    pub interval_ms: f64,
    /// A channel index, or the loudest channel. Default `Max`.
    pub channel: LevelChannel,
    /// Zones used for [`LevelState::zone`]. Default: the meter's default zones.
    pub zones: Vec<MeterZone>,
    pub enabled: bool,
}

impl Default for LevelOptions {
    fn default() -> Self {
        Self {
            interval_ms: DEFAULT_INTERVAL_MS,
            channel: LevelChannel::Max,
            zones: default_zones(),
            enabled: true,
        }
    }
}

/// A sampled level (`LevelState`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelState {
    pub peak_db: f32,
    pub rms_db: Option<f32>,
    pub zone: MeterZoneName,
}

fn default_zones() -> Vec<MeterZone> {
    DEFAULT_ZONES.to_vec()
}

/// The channel a level reads from a frame (`pickChannel`): by index, or the
/// loudest by peak (the first one on ties).
pub fn pick_channel(frame: &MeterFrame, channel: LevelChannel) -> Option<ChannelLevel> {
    match channel {
        LevelChannel::Index(index) => frame.channels.get(index).copied(),
        LevelChannel::Max => {
            let mut loudest: Option<ChannelLevel> = None;
            for level in &frame.channels {
                if loudest.is_none_or(|current| level.peak_db > current.peak_db) {
                    loudest = Some(*level);
                }
            }
            loudest
        }
    }
}

/// Low-rate level reading from a meter source (`useLevel` without React).
///
/// ```ignore
/// let mut level = LevelSampler::new(LevelOptions::default());
/// level.poll(&source, now_ms);       // samples at most every interval_ms
/// let LevelState { peak_db, zone, .. } = level.state();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LevelSampler {
    options: LevelOptions,
    peak_db: f32,
    rms_db: Option<f32>,
    last_sample_ms: Option<f64>,
}

impl LevelSampler {
    pub fn new(options: LevelOptions) -> Self {
        Self {
            options,
            peak_db: f32::NEG_INFINITY,
            rms_db: None,
            last_sample_ms: None,
        }
    }

    pub fn options(&self) -> &LevelOptions {
        &self.options
    }

    /// New options. Zones and channel apply immediately (the zone is
    /// computed on read, like the hook computes it in render).
    pub fn set_options(&mut self, options: LevelOptions) {
        if !options.enabled || options.interval_ms != self.options.interval_ms {
            self.last_sample_ms = None;
        }
        self.options = options;
    }

    /// Reads the latest frame of `source` now. Returns whether the level
    /// changed. A source with no frame yet leaves the level as it is.
    pub fn sample(&mut self, source: &FrameSource<MeterFrame>) -> bool {
        let Some(picked) = source
            .with_latest(|frame| frame.map(|frame| pick_channel(frame, self.options.channel)))
        else {
            return false;
        };
        let peak_db = picked.map_or(f32::NEG_INFINITY, |level| level.peak_db);
        let rms_db = picked.and_then(|level| level.rms_db);
        let unchanged = same_db(self.peak_db, peak_db) && rms_db.map(Db) == self.rms_db.map(Db);
        if unchanged {
            return false;
        }
        self.peak_db = peak_db;
        self.rms_db = rms_db;
        true
    }

    /// Samples when `interval_ms` has passed since the last sample (the first
    /// sample comes one interval after the first poll, like `setInterval`).
    /// Returns whether the level changed. Does nothing while disabled.
    pub fn poll(&mut self, source: &FrameSource<MeterFrame>, now_ms: f64) -> bool {
        if !self.options.enabled {
            return false;
        }
        let Some(last) = self.last_sample_ms else {
            self.last_sample_ms = Some(now_ms);
            return false;
        };
        if now_ms - last < self.options.interval_ms {
            return false;
        }
        self.last_sample_ms = Some(now_ms);
        self.sample(source)
    }

    /// The last sampled level, with its zone.
    pub fn state(&self) -> LevelState {
        LevelState {
            peak_db: self.peak_db,
            rms_db: self.rms_db,
            zone: zone_for_db(self.peak_db, &self.options.zones),
        }
    }
}

impl Default for LevelSampler {
    fn default() -> Self {
        Self::new(LevelOptions::default())
    }
}

/// Compares like JavaScript `===` on numbers, except NaN equals NaN (so a NaN
/// reading doesn't re-render forever).
fn same_db(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

#[derive(Clone, Copy)]
struct Db(f32);

impl PartialEq for Db {
    fn eq(&self, other: &Self) -> bool {
        same_db(self.0, other.0)
    }
}

/// Keyed state behind [`use_level`].
pub struct LevelSamplerState {
    sampler: LevelSampler,
    source: Option<FrameSource<MeterFrame>>,
    timer: Option<(u64, Task<()>)>,
}

/// `useLevel` for GPUI: samples `source` every `options.interval_ms` on a
/// timer and re-renders the calling view only when the reading changes.
///
/// ```ignore
/// let LevelState { peak_db, rms_db, zone } =
///     use_level("level", Some(signal.meter.clone()), LevelOptions { interval_ms: 100.0, ..Default::default() }, window, cx);
/// ```
pub fn use_level(
    id: impl Into<ElementId>,
    source: Option<FrameSource<MeterFrame>>,
    options: LevelOptions,
    window: &mut Window,
    cx: &mut App,
) -> LevelState {
    let entity = window.use_keyed_state(id, cx, |_, _| LevelSamplerState {
        sampler: LevelSampler::new(options.clone()),
        source: None,
        timer: None,
    });
    entity.update(cx, |state, cx| {
        if state.sampler.options != options {
            state.sampler.set_options(options.clone());
        }
        state.source = source;
        let interval_ms = options.interval_ms.max(1.0);
        let key = interval_ms.to_bits();
        let wants_timer = options.enabled && state.source.is_some();
        match (&state.timer, wants_timer) {
            (Some((current, _)), true) if *current == key => {}
            (_, true) => {
                let task = cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_secs_f64(interval_ms / 1000.0))
                            .await;
                        let alive = this.update(cx, |state, cx| {
                            let changed = match &state.source {
                                Some(source) => state.sampler.sample(source),
                                None => false,
                            };
                            if changed {
                                cx.notify();
                            }
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                });
                state.timer = Some((key, task));
            }
            (_, false) => state.timer = None,
        }
        state.sampler.state()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::frame_source::FrameEmitter;

    #[test]
    fn samples_a_source_at_its_interval() {
        let emitter = FrameEmitter::<MeterFrame>::new();
        let source = emitter.source();
        let mut level = LevelSampler::new(LevelOptions {
            interval_ms: 100.0,
            ..Default::default()
        });
        level.poll(&source, 0.0);
        emitter.emit(MeterFrame::mono(ChannelLevel::with_rms(-6.0, -12.0)));
        assert!(!level.poll(&source, 50.0));
        assert_eq!(level.state().peak_db, f32::NEG_INFINITY);
        assert!(level.poll(&source, 120.0));
        assert_eq!(
            level.state(),
            LevelState {
                peak_db: -6.0,
                rms_db: Some(-12.0),
                zone: MeterZoneName::Clip
            }
        );
    }

    #[test]
    fn zones_apply_without_a_new_sample() {
        let emitter = FrameEmitter::<MeterFrame>::new();
        let mut level = LevelSampler::default();
        emitter.emit(MeterFrame::mono(ChannelLevel::with_rms(-6.0, -12.0)));
        assert!(level.sample(&emitter.source()));
        assert!(!level.sample(&emitter.source()));
        level.set_options(LevelOptions {
            zones: vec![MeterZone {
                from_db: f32::NEG_INFINITY,
                zone: MeterZoneName::Ok,
            }],
            ..Default::default()
        });
        assert_eq!(level.state().peak_db, -6.0);
        assert_eq!(level.state().zone, MeterZoneName::Ok);
    }

    #[test]
    fn picks_the_loudest_or_an_indexed_channel() {
        let frame = MeterFrame::stereo(ChannelLevel::new(-12.0), ChannelLevel::new(-3.0));
        assert_eq!(
            pick_channel(&frame, LevelChannel::Max).unwrap().peak_db,
            -3.0
        );
        assert_eq!(
            pick_channel(&frame, LevelChannel::Index(0))
                .unwrap()
                .peak_db,
            -12.0
        );
        assert_eq!(pick_channel(&frame, LevelChannel::Index(4)), None);
        assert_eq!(
            pick_channel(&MeterFrame::default(), LevelChannel::Max),
            None
        );
    }

    #[test]
    fn missing_channel_reads_as_silence() {
        let emitter = FrameEmitter::<MeterFrame>::new();
        let mut level = LevelSampler::new(LevelOptions {
            channel: LevelChannel::Index(3),
            ..Default::default()
        });
        assert!(!level.sample(&emitter.source()));
        emitter.emit(MeterFrame::mono(ChannelLevel::new(-1.0)));
        assert!(!level.sample(&emitter.source()));
        assert_eq!(level.state().peak_db, f32::NEG_INFINITY);
        assert_eq!(level.state().zone, MeterZoneName::Ok);
    }
}
