//! Port of `hooks/use-audio-config.tsx`.
//!
//! React passes [`AudioConfig`] down through context. GPUI has no context, so
//! containers (channel strips, mixers, blocks) pass it explicitly: every
//! component that reads config in the React version has a `.config(AudioConfig)`
//! builder method, and resolves each setting as
//! `explicit prop → config → component default` (explicit props always win),
//! exactly like `useAudioConfig` + prop defaults.

use crate::audio::ballistics::BallisticsInput;
use crate::audio::types::{MeterZone, Orientation};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioSize {
    Sm,
    #[default]
    Default,
    Lg,
}

/// Settings a container (a mixer or channel strip) passes down to the audiocn
/// components inside it. Explicit props on a component always win.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioConfig {
    pub orientation: Option<Orientation>,
    pub size: Option<AudioSize>,
    pub disabled: bool,
    /// Muted or silenced by another channel's solo: meters render dimmed.
    pub dimmed: bool,
    pub min_db: Option<f32>,
    pub max_db: Option<f32>,
    pub zones: Option<Vec<MeterZone>>,
    pub ballistics: Option<BallisticsInput>,
}

impl AudioConfig {
    /// Merges `self` over `parent` (the `AudioConfigProvider` rule).
    pub fn merged_over(&self, parent: &AudioConfig) -> AudioConfig {
        AudioConfig {
            ballistics: self
                .ballistics
                .clone()
                .or_else(|| parent.ballistics.clone()),
            dimmed: self.dimmed || parent.dimmed,
            disabled: self.disabled || parent.disabled,
            max_db: self.max_db.or(parent.max_db),
            min_db: self.min_db.or(parent.min_db),
            orientation: self.orientation.or(parent.orientation),
            size: self.size.or(parent.size),
            zones: self.zones.clone().or_else(|| parent.zones.clone()),
        }
    }
}
