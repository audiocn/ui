//! Port of `lib/audio/click.ts`.
//!
//! The knob's synthesised click: a short noise snap over a high, fast-damped
//! sine ring.
//!
//! **What changed for GPUI.** The TS plays through a Web Audio `AudioContext`
//! (resuming a suspended context, queueing one click meanwhile). There is no
//! audio device here, so everything up to the device is ported and the device
//! part is left to a native player (e.g. `crate::io` behind the `device`
//! feature):
//!
//! - [`synthesize`] renders the sample buffer (`AudioBuffer`) into a `Vec<f32>`.
//! - [`ClickSound::play`] applies the same rules as the TS `play` (one cached
//!   buffer per sample rate, `min_interval_ms` throttling per sound, scheduled
//!   clicks never held back, random pitch within `pitch_spread`) and returns a
//!   [`ClickVoice`]: mono samples, playback rate, gain and start time. Play it
//!   at `sample_rate * playback_rate`, scaled by `gain`. `None` = held back.
//! - Context resume/suspend/close handling has no equivalent and is dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use crate::audio::frame_loop::now_ms;

/// A ring: a sine at `hz` that fades over `decay_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickTone {
    pub hz: f32,
    pub decay_ms: f32,
    pub gain: f32,
}

/// A snap: white noise that fades over `decay_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickNoise {
    pub decay_ms: f32,
    pub gain: f32,
}

/// Everything that shapes a click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickSoundParams {
    pub tone: ClickTone,
    pub noise: ClickNoise,
    pub length_ms: f32,
    /// Random pitch per click, as a ratio: 0.04 is ±4%.
    pub pitch_spread: f32,
    /// Linear output gain.
    pub volume: f32,
}

/// A short noise snap over a high, fast-damped ring: a tick, not a thud.
pub const DEFAULT_CLICK_PARAMS: ClickSoundParams = ClickSoundParams {
    length_ms: 6.0,
    noise: ClickNoise {
        decay_ms: 0.4,
        gain: 0.6,
    },
    pitch_spread: 0.04,
    tone: ClickTone {
        decay_ms: 1.2,
        gain: 0.4,
        hz: 4200.0,
    },
    volume: 0.12,
};

impl Default for ClickSoundParams {
    fn default() -> Self {
        DEFAULT_CLICK_PARAMS
    }
}

/// Partial [`ClickTone`]; `None` keeps the base value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickToneChanges {
    pub hz: Option<f32>,
    pub decay_ms: Option<f32>,
    pub gain: Option<f32>,
}

/// Partial [`ClickNoise`]; `None` keeps the base value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickNoiseChanges {
    pub decay_ms: Option<f32>,
    pub gain: Option<f32>,
}

/// Changes deep-merged over a sound's parameters; `None` keeps the base value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickSoundChanges {
    pub tone: ClickToneChanges,
    pub noise: ClickNoiseChanges,
    pub length_ms: Option<f32>,
    pub pitch_spread: Option<f32>,
    pub volume: Option<f32>,
}

fn merge(base: &ClickSoundParams, changes: &ClickSoundChanges) -> ClickSoundParams {
    ClickSoundParams {
        length_ms: changes.length_ms.unwrap_or(base.length_ms),
        noise: ClickNoise {
            decay_ms: changes.noise.decay_ms.unwrap_or(base.noise.decay_ms),
            gain: changes.noise.gain.unwrap_or(base.noise.gain),
        },
        pitch_spread: changes.pitch_spread.unwrap_or(base.pitch_spread),
        tone: ClickTone {
            decay_ms: changes.tone.decay_ms.unwrap_or(base.tone.decay_ms),
            gain: changes.tone.gain.unwrap_or(base.tone.gain),
            hz: changes.tone.hz.unwrap_or(base.tone.hz),
        },
        volume: changes.volume.unwrap_or(base.volume),
    }
}

const MS_PER_SECOND: f64 = 1000.0;

static RANDOM_STATE: AtomicU64 = AtomicU64::new(0x853c_49e6_748f_ea9b);

/// `Math.random()` stand-in: a uniform value in 0..1 (SplitMix64, process-wide).
pub fn click_random() -> f64 {
    let mut z = RANDOM_STATE
        .fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed)
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// The sum of a decaying noise snap and a decaying sine ring:
/// `ceil(length_ms / 1000 * sample_rate)` mono samples. `random` returns 0..1.
pub fn synthesize(
    params: &ClickSoundParams,
    sample_rate: u32,
    mut random: impl FnMut() -> f64,
) -> Vec<f32> {
    let rate = sample_rate as f64;
    let length = ((params.length_ms as f64 / MS_PER_SECOND) * rate)
        .ceil()
        .max(0.0) as usize;
    let noise_decay = params.noise.decay_ms as f64 / MS_PER_SECOND;
    let tone_decay = params.tone.decay_ms as f64 / MS_PER_SECOND;
    let (noise_gain, tone_gain, hz) = (
        params.noise.gain as f64,
        params.tone.gain as f64,
        params.tone.hz as f64,
    );
    (0..length)
        .map(|index| {
            let time = index as f64 / rate;
            let snap = noise_gain * (random() * 2.0 - 1.0) * (-time / noise_decay).exp();
            let ring = tone_gain
                * (2.0 * std::f64::consts::PI * hz * time).sin()
                * (-time / tone_decay).exp();
            (snap + ring) as f32
        })
        .collect()
}

/// How to play a click.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClickPlayOptions {
    /// Time to play at, on the player's clock. Scheduled clicks are never
    /// held back (and `min_interval_ms` is ignored).
    pub when: Option<f64>,
    /// Clicks closer than this to this sound's last one are dropped. Default 0.
    pub min_interval_ms: f64,
}

/// One click ready for a player.
#[derive(Clone, Debug, PartialEq)]
pub struct ClickVoice {
    /// Mono samples at `sample_rate`, shared with the sound's cache.
    pub samples: Arc<[f32]>,
    pub sample_rate: u32,
    /// `1 ± pitch_spread`: play the buffer this much faster.
    pub playback_rate: f32,
    /// Linear output gain (the sound's `volume`).
    pub gain: f32,
    /// Start time from [`ClickPlayOptions::when`]; `None` = now.
    pub when: Option<f64>,
}

struct ClickSoundState {
    buffers: Mutex<Vec<(u32, Arc<[f32]>)>>,
    last_played_at: Mutex<f64>,
}

/// A synthesised click. Clones share the buffer cache and the throttle timer;
/// [`with`](Self::with) makes a new sound with its own. Compares by identity.
#[derive(Clone)]
pub struct ClickSound {
    params: ClickSoundParams,
    state: Arc<ClickSoundState>,
}

impl std::fmt::Debug for ClickSound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClickSound")
            .field("params", &self.params)
            .finish()
    }
}

impl PartialEq for ClickSound {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}

/// A synthesised click. With default changes, the knob's tick.
pub fn create_click_sound(changes: ClickSoundChanges) -> ClickSound {
    ClickSound::from_params(merge(&DEFAULT_CLICK_PARAMS, &changes))
}

impl Default for ClickSound {
    fn default() -> Self {
        create_click_sound(ClickSoundChanges::default())
    }
}

impl ClickSound {
    /// A sound with exactly these parameters.
    pub fn from_params(params: ClickSoundParams) -> Self {
        Self {
            params,
            state: Arc::new(ClickSoundState {
                buffers: Mutex::new(Vec::new()),
                last_played_at: Mutex::new(f64::NEG_INFINITY),
            }),
        }
    }

    /// The full parameters after merging, for docs or a tuning UI.
    pub fn params(&self) -> &ClickSoundParams {
        &self.params
    }

    /// A new sound with these changes deep-merged over this one's parameters.
    pub fn with(&self, changes: ClickSoundChanges) -> ClickSound {
        ClickSound::from_params(merge(&self.params, &changes))
    }

    /// The click's samples at `sample_rate`, synthesised once per rate.
    pub fn buffer(&self, sample_rate: u32) -> Arc<[f32]> {
        self.buffer_with(sample_rate, &mut click_random)
    }

    fn buffer_with(&self, sample_rate: u32, random: &mut impl FnMut() -> f64) -> Arc<[f32]> {
        let mut buffers = self.state.buffers.lock();
        if let Some((_, buffer)) = buffers.iter().find(|(rate, _)| *rate == sample_rate) {
            return buffer.clone();
        }
        let buffer: Arc<[f32]> = synthesize(&self.params, sample_rate, &mut *random).into();
        buffers.push((sample_rate, buffer.clone()));
        buffer
    }

    fn held_back(&self, min_interval_ms: f64, now: f64) -> bool {
        let mut last = self.state.last_played_at.lock();
        if now - *last < min_interval_ms {
            return true;
        }
        *last = now;
        false
    }

    /// Plays once (see the module docs): the voice to hand to a player, or
    /// `None` when held back by `min_interval_ms`. Uses [`now_ms`] and
    /// [`click_random`].
    pub fn play(&self, options: ClickPlayOptions, sample_rate: u32) -> Option<ClickVoice> {
        self.play_at(options, sample_rate, now_ms(), &mut click_random)
    }

    /// [`play`](Self::play) with an explicit clock and random source.
    pub fn play_at(
        &self,
        options: ClickPlayOptions,
        sample_rate: u32,
        now_ms: f64,
        random: &mut impl FnMut() -> f64,
    ) -> Option<ClickVoice> {
        if options.when.is_none() && self.held_back(options.min_interval_ms, now_ms) {
            return None;
        }
        let samples = self.buffer_with(sample_rate, random);
        let playback_rate = (1.0 + (random() - 0.5) * self.params.pitch_spread as f64 * 2.0) as f32;
        Some(ClickVoice {
            samples,
            sample_rate,
            playback_rate,
            gain: self.params.volume,
            when: options.when,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: Option<f32>) -> ClickToneChanges {
        ClickToneChanges {
            hz,
            ..Default::default()
        }
    }

    #[test]
    fn merges_changes_deeply_over_defaults() {
        let defaults = *create_click_sound(Default::default()).params();
        let sound = create_click_sound(ClickSoundChanges {
            tone: tone(Some(900.0)),
            volume: Some(0.5),
            ..Default::default()
        });
        let mut expected = defaults;
        expected.tone.hz = 900.0;
        expected.volume = 0.5;
        assert_eq!(*sound.params(), expected);
    }

    #[test]
    fn derives_over_the_base_not_the_defaults() {
        let thud = create_click_sound(ClickSoundChanges {
            noise: ClickNoiseChanges {
                gain: Some(0.21),
                ..Default::default()
            },
            tone: ClickToneChanges {
                gain: Some(1.0),
                hz: Some(460.0),
                ..Default::default()
            },
            volume: Some(0.18),
            ..Default::default()
        });
        let brighter = thud.with(ClickSoundChanges {
            tone: tone(Some(900.0)),
            ..Default::default()
        });
        let mut expected = *thud.params();
        expected.tone.hz = 900.0;
        assert_eq!(*brighter.params(), expected);
        assert_eq!(thud.params().tone.hz, 460.0);
    }

    #[test]
    fn keeps_base_values_for_unset_changes() {
        let thud = create_click_sound(ClickSoundChanges {
            tone: tone(Some(460.0)),
            volume: Some(0.18),
            ..Default::default()
        });
        assert_eq!(
            *create_click_sound(Default::default()).params(),
            DEFAULT_CLICK_PARAMS
        );
        assert_eq!(*thud.with(Default::default()).params(), *thud.params());
    }

    #[test]
    fn synthesises_length_ms_at_the_sample_rate() {
        let sound = create_click_sound(ClickSoundChanges {
            length_ms: Some(10.0),
            ..Default::default()
        });
        assert_eq!(sound.buffer(48_000).len(), 480);
    }

    #[test]
    fn starts_on_the_snap_then_rings_at_the_tone() {
        let snap = synthesize(
            &merge(
                &DEFAULT_CLICK_PARAMS,
                &ClickSoundChanges {
                    noise: ClickNoiseChanges {
                        gain: Some(0.5),
                        ..Default::default()
                    },
                    tone: ClickToneChanges {
                        gain: Some(0.0),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            48_000,
            || 1.0,
        );
        let ring = synthesize(
            &merge(
                &DEFAULT_CLICK_PARAMS,
                &ClickSoundChanges {
                    noise: ClickNoiseChanges {
                        gain: Some(0.0),
                        ..Default::default()
                    },
                    tone: ClickToneChanges {
                        decay_ms: Some(1000.0),
                        gain: Some(0.5),
                        hz: Some(12_000.0),
                    },
                    ..Default::default()
                },
            ),
            48_000,
            || 1.0,
        );
        assert_eq!(snap[0], 0.5);
        assert!(snap[1].abs() < 0.5);
        assert_eq!(ring[0], 0.0);
        assert!((ring[1] - 0.5).abs() < 0.005);
        assert!((ring[3] + 0.5).abs() < 0.005);
    }

    #[test]
    fn builds_its_buffer_once_per_rate() {
        let sound = create_click_sound(Default::default());
        let a = sound.buffer(48_000);
        let b = sound.buffer(48_000);
        let c = sound.buffer(44_100);
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
        let derived = sound.with(ClickSoundChanges {
            tone: tone(Some(900.0)),
            ..Default::default()
        });
        assert!(!Arc::ptr_eq(&a, &derived.buffer(48_000)));
    }

    #[test]
    fn plays_at_volume() {
        let sound = create_click_sound(ClickSoundChanges {
            volume: Some(0.3),
            ..Default::default()
        });
        let voice = sound
            .play_at(Default::default(), 48_000, 0.0, &mut || 0.5)
            .unwrap();
        assert_eq!(voice.gain, 0.3);
        assert_eq!(voice.when, None);
        assert_eq!(voice.playback_rate, 1.0);
    }

    #[test]
    fn varies_pitch_within_spread() {
        let wide = create_click_sound(ClickSoundChanges {
            pitch_spread: Some(0.1),
            ..Default::default()
        });
        let flat = create_click_sound(ClickSoundChanges {
            pitch_spread: Some(0.0),
            ..Default::default()
        });
        let opts = ClickPlayOptions::default();
        let a = wide.play_at(opts, 48_000, 0.0, &mut || 0.0).unwrap();
        let b = flat.play_at(opts, 48_000, 0.0, &mut || 0.0).unwrap();
        assert!((a.playback_rate - 0.9).abs() < 0.005);
        assert_eq!(b.playback_rate, 1.0);
    }

    #[test]
    fn holds_back_clicks_closer_than_min_interval() {
        let sound = create_click_sound(Default::default());
        let opts = ClickPlayOptions {
            min_interval_ms: 30.0,
            ..Default::default()
        };
        let played = [0.0, 20.0, 30.0]
            .into_iter()
            .filter(|&now| {
                sound
                    .play_at(opts, 48_000, now, &mut click_random)
                    .is_some()
            })
            .count();
        assert_eq!(played, 2);
    }

    #[test]
    fn limits_each_sound_on_its_own_timer() {
        let opts = ClickPlayOptions {
            min_interval_ms: 30.0,
            ..Default::default()
        };
        let tick = create_click_sound(Default::default());
        assert!(tick.play_at(opts, 48_000, 0.0, &mut click_random).is_some());
        assert!(
            tick.play_at(opts, 48_000, 10.0, &mut click_random)
                .is_none()
        );
        let louder = tick.with(ClickSoundChanges {
            volume: Some(0.5),
            ..Default::default()
        });
        assert!(
            louder
                .play_at(opts, 48_000, 10.0, &mut click_random)
                .is_some()
        );
    }

    #[test]
    fn never_holds_back_scheduled_clicks() {
        let sound = create_click_sound(Default::default());
        let throttled = ClickPlayOptions {
            min_interval_ms: 30.0,
            ..Default::default()
        };
        let at = |when| ClickPlayOptions {
            when: Some(when),
            min_interval_ms: 0.0,
        };
        let whens: Vec<Option<f64>> = [throttled, at(1.0), at(1.01)]
            .into_iter()
            .filter_map(|o| sound.play_at(o, 48_000, 0.0, &mut click_random))
            .map(|v| v.when)
            .collect();
        assert_eq!(whens, [None, Some(1.0), Some(1.01)]);
    }

    #[test]
    fn random_stays_in_range() {
        for _ in 0..1000 {
            let r = click_random();
            assert!((0.0..1.0).contains(&r));
        }
    }
}
