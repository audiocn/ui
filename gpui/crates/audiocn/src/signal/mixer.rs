//! Port of `hooks/use-mixer.ts`.
//!
//! [`MixerState`] is plain data with the hook's operations (`set_gain`,
//! `set_solo`, `is_audible`, `is_dimmed`, ...). [`mixer_reducer`] is the pure
//! reducer the hook exports. [`MixerStore`] adds `reset()` to the initial
//! state, and [`use_mixer`] keeps one in GPUI keyed state.
//!
//! Not ported: `persistKey` (`localStorage`); save [`MixerState`] yourself.

use gpui::{App, ElementId, Entity, SharedString, Window};

/// One channel of a mixer.
#[derive(Clone, Debug, PartialEq)]
pub struct MixerChannelState {
    pub id: SharedString,
    /// Channel gain in dB.
    pub gain_db: f32,
    pub muted: bool,
    pub solo: bool,
    /// −1 (left) to 1 (right).
    pub pan: f32,
    /// Send the channel to the speakers.
    pub monitor: bool,
}

/// The master section of a mixer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MixerMasterState {
    pub gain_db: f32,
    pub muted: bool,
}

/// A whole mixer: channels plus master.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerState {
    pub channels: Vec<MixerChannelState>,
    pub master: MixerMasterState,
}

/// `MixerChannelInit`: an id plus any fields that differ from the defaults
/// (0 dB, unmuted, no solo, centre pan, not monitored).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerChannelInit {
    pub id: SharedString,
    pub gain_db: Option<f32>,
    pub muted: Option<bool>,
    pub solo: Option<bool>,
    pub pan: Option<f32>,
    pub monitor: Option<bool>,
}

impl MixerChannelInit {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }

    pub fn gain_db(mut self, gain_db: f32) -> Self {
        self.gain_db = Some(gain_db);
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    pub fn solo(mut self, solo: bool) -> Self {
        self.solo = Some(solo);
        self
    }

    pub fn pan(mut self, pan: f32) -> Self {
        self.pan = Some(pan);
        self
    }

    pub fn monitor(mut self, monitor: bool) -> Self {
        self.monitor = Some(monitor);
        self
    }
}

impl From<MixerChannelInit> for MixerChannelState {
    /// `createChannel`: fills missing fields with defaults (pan is not clamped,
    /// like the TS).
    fn from(init: MixerChannelInit) -> Self {
        Self {
            id: init.id,
            gain_db: init.gain_db.unwrap_or(0.0),
            muted: init.muted.unwrap_or(false),
            solo: init.solo.unwrap_or(false),
            pan: init.pan.unwrap_or(0.0),
            monitor: init.monitor.unwrap_or(false),
        }
    }
}

/// A partial update of one channel (`Partial<Omit<MixerChannelState, "id">>`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MixerChannelPatch {
    pub gain_db: Option<f32>,
    pub muted: Option<bool>,
    pub solo: Option<bool>,
    pub pan: Option<f32>,
    pub monitor: Option<bool>,
}

/// A partial update of the master (`Partial<MixerMasterState>`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MixerMasterPatch {
    pub gain_db: Option<f32>,
    pub muted: Option<bool>,
}

/// Options for [`MixerState::new`] / [`use_mixer`] (`UseMixerOptions`
/// without the React-only controlled and persistence fields).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerOptions {
    /// Initial channels. Missing fields get defaults.
    pub channels: Vec<MixerChannelInit>,
    /// Initial master settings.
    pub master: MixerMasterPatch,
}

/// The actions [`mixer_reducer`] understands.
#[derive(Clone, Debug, PartialEq)]
pub enum MixerAction {
    Replace(MixerState),
    Channel {
        id: SharedString,
        patch: MixerChannelPatch,
    },
    Solo {
        id: SharedString,
        solo: bool,
        exclusive: bool,
    },
    Master(MixerMasterPatch),
    Add(MixerChannelInit),
    Remove(SharedString),
}

/// The pure mixer reducer, for use outside GPUI.
pub fn mixer_reducer(state: &MixerState, action: MixerAction) -> MixerState {
    let mut next = state.clone();
    next.apply(action);
    next
}

/// Whether a channel is heard: not muted, and soloed if anything is soloed.
pub fn is_channel_audible(state: &MixerState, id: &str) -> bool {
    let Some(channel) = state.channel(id) else {
        return false;
    };
    if channel.muted {
        return false;
    }
    !state.any_solo() || channel.solo
}

impl MixerState {
    /// `createState`: channels with defaults filled in, master at 0 dB unmuted.
    pub fn new(options: MixerOptions) -> Self {
        Self {
            channels: options.channels.into_iter().map(Into::into).collect(),
            master: MixerMasterState {
                gain_db: options.master.gain_db.unwrap_or(0.0),
                muted: options.master.muted.unwrap_or(false),
            },
        }
    }

    /// Applies one reducer action in place. Returns whether anything changed.
    pub fn apply(&mut self, action: MixerAction) -> bool {
        let before = self.clone();
        match action {
            MixerAction::Replace(state) => *self = state,
            MixerAction::Channel { id, patch } => {
                if let Some(channel) = self.channel_mut(&id) {
                    if let Some(gain_db) = patch.gain_db {
                        channel.gain_db = gain_db;
                    }
                    if let Some(muted) = patch.muted {
                        channel.muted = muted;
                    }
                    if let Some(solo) = patch.solo {
                        channel.solo = solo;
                    }
                    if let Some(pan) = patch.pan {
                        channel.pan = pan;
                    }
                    if let Some(monitor) = patch.monitor {
                        channel.monitor = monitor;
                    }
                    channel.pan = clamp(channel.pan, -1.0, 1.0);
                }
            }
            MixerAction::Solo {
                id,
                solo,
                exclusive,
            } => {
                for channel in &mut self.channels {
                    if channel.id == id {
                        channel.solo = solo;
                    } else if exclusive && solo {
                        channel.solo = false;
                    }
                }
            }
            MixerAction::Master(patch) => {
                if let Some(gain_db) = patch.gain_db {
                    self.master.gain_db = gain_db;
                }
                if let Some(muted) = patch.muted {
                    self.master.muted = muted;
                }
            }
            MixerAction::Add(init) => {
                if self.channel(&init.id).is_none() {
                    self.channels.push(init.into());
                }
            }
            MixerAction::Remove(id) => self.channels.retain(|channel| channel.id != id),
        }
        *self != before
    }

    pub fn channel(&self, id: &str) -> Option<&MixerChannelState> {
        self.channels
            .iter()
            .find(|channel| channel.id.as_ref() == id)
    }

    pub fn channel_mut(&mut self, id: &str) -> Option<&mut MixerChannelState> {
        self.channels
            .iter_mut()
            .find(|channel| channel.id.as_ref() == id)
    }

    /// True when any channel is soloed.
    pub fn any_solo(&self) -> bool {
        self.channels.iter().any(|channel| channel.solo)
    }

    /// False when muted, or when another channel is soloed.
    pub fn is_audible(&self, id: &str) -> bool {
        is_channel_audible(self, id)
    }

    /// True when a channel is silenced only because another is soloed.
    pub fn is_dimmed(&self, id: &str) -> bool {
        self.channel(id)
            .is_some_and(|channel| !channel.muted && self.any_solo() && !channel.solo)
    }

    fn patch(&mut self, id: &str, patch: MixerChannelPatch) -> bool {
        self.apply(MixerAction::Channel {
            id: SharedString::from(id.to_string()),
            patch,
        })
    }

    pub fn set_gain(&mut self, id: &str, gain_db: f32) -> bool {
        self.patch(
            id,
            MixerChannelPatch {
                gain_db: Some(gain_db),
                ..Default::default()
            },
        )
    }

    pub fn set_muted(&mut self, id: &str, muted: bool) -> bool {
        self.patch(
            id,
            MixerChannelPatch {
                muted: Some(muted),
                ..Default::default()
            },
        )
    }

    /// Solos or unsolos a channel. With `exclusive`, soloing clears every other solo.
    pub fn set_solo(&mut self, id: &str, solo: bool, exclusive: bool) -> bool {
        self.apply(MixerAction::Solo {
            id: SharedString::from(id.to_string()),
            solo,
            exclusive,
        })
    }

    /// Pan, clamped to −1..1.
    pub fn set_pan(&mut self, id: &str, pan: f32) -> bool {
        self.patch(
            id,
            MixerChannelPatch {
                pan: Some(pan),
                ..Default::default()
            },
        )
    }

    pub fn set_monitor(&mut self, id: &str, monitor: bool) -> bool {
        self.patch(
            id,
            MixerChannelPatch {
                monitor: Some(monitor),
                ..Default::default()
            },
        )
    }

    pub fn set_master_gain(&mut self, gain_db: f32) -> bool {
        self.apply(MixerAction::Master(MixerMasterPatch {
            gain_db: Some(gain_db),
            muted: None,
        }))
    }

    pub fn set_master_muted(&mut self, muted: bool) -> bool {
        self.apply(MixerAction::Master(MixerMasterPatch {
            gain_db: None,
            muted: Some(muted),
        }))
    }

    /// Adds a channel unless one with the same id exists.
    pub fn add_channel(&mut self, channel: MixerChannelInit) -> bool {
        self.apply(MixerAction::Add(channel))
    }

    pub fn remove_channel(&mut self, id: &str) -> bool {
        self.apply(MixerAction::Remove(SharedString::from(id.to_string())))
    }
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

/// `useMixer`'s state holder: the current [`MixerState`] (deref) plus the
/// initial one for [`reset`](Self::reset).
#[derive(Clone, Debug, PartialEq)]
pub struct MixerStore {
    pub state: MixerState,
    initial: MixerState,
}

impl MixerStore {
    pub fn new(options: MixerOptions) -> Self {
        let initial = MixerState::new(options);
        Self {
            state: initial.clone(),
            initial,
        }
    }

    /// Back to the state the store was created with.
    pub fn reset(&mut self) -> bool {
        let changed = self.state != self.initial;
        self.state = self.initial.clone();
        changed
    }

    pub fn dispatch(&mut self, action: MixerAction) -> bool {
        self.state.apply(action)
    }
}

impl std::ops::Deref for MixerStore {
    type Target = MixerState;
    fn deref(&self) -> &MixerState {
        &self.state
    }
}

impl std::ops::DerefMut for MixerStore {
    fn deref_mut(&mut self) -> &mut MixerState {
        &mut self.state
    }
}

/// `useMixer` for GPUI: a [`MixerStore`] in keyed element state. `options`
/// only seed it on first render. Change it from callbacks and notify, which
/// re-renders the view that called this:
///
/// ```ignore
/// let mixer = use_mixer("mixer", MixerOptions { channels: vec![MixerChannelInit::new("mic")], ..Default::default() }, window, cx);
/// let gain = mixer.read(cx).channel("mic").map(|c| c.gain_db);
/// Fader::new("mic").on_value_change({
///     let mixer = mixer.clone();
///     move |db, _, cx| mixer.update(cx, |m, cx| { m.set_gain("mic", db); cx.notify() })
/// })
/// ```
pub fn use_mixer(
    id: impl Into<ElementId>,
    options: MixerOptions,
    window: &mut Window,
    cx: &mut App,
) -> Entity<MixerStore> {
    window.use_keyed_state(id, cx, |_, _| MixerStore::new(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_state() -> MixerState {
        MixerState {
            channels: vec![
                MixerChannelState {
                    gain_db: 0.0,
                    id: "mic".into(),
                    monitor: false,
                    muted: false,
                    pan: 0.0,
                    solo: false,
                },
                MixerChannelState {
                    gain_db: -6.0,
                    id: "music".into(),
                    monitor: true,
                    muted: false,
                    pan: 0.0,
                    solo: false,
                },
            ],
            master: MixerMasterState {
                gain_db: 0.0,
                muted: false,
            },
        }
    }

    #[test]
    fn patches_one_channel_and_clamps_pan() {
        let base = base_state();
        let next = mixer_reducer(
            &base,
            MixerAction::Channel {
                id: "mic".into(),
                patch: MixerChannelPatch {
                    pan: Some(3.0),
                    ..Default::default()
                },
            },
        );
        assert_eq!(next.channels[0].pan, 1.0);
        assert_eq!(next.channels[1], base.channels[1]);
    }

    #[test]
    fn solos_exclusively_when_asked() {
        let soloed = mixer_reducer(
            &base_state(),
            MixerAction::Solo {
                exclusive: false,
                id: "mic".into(),
                solo: true,
            },
        );
        let exclusive = mixer_reducer(
            &soloed,
            MixerAction::Solo {
                exclusive: true,
                id: "music".into(),
                solo: true,
            },
        );
        let solos: Vec<_> = exclusive.channels.iter().map(|c| c.solo).collect();
        assert_eq!(solos, vec![false, true]);
    }

    #[test]
    fn adds_and_removes_channels() {
        let added = mixer_reducer(
            &base_state(),
            MixerAction::Add(MixerChannelInit::new("system")),
        );
        let last = added.channels.last().unwrap();
        assert_eq!(last.id.as_ref(), "system");
        assert_eq!(last.gain_db, 0.0);
        assert!(!last.muted);
        assert_eq!(
            mixer_reducer(&added, MixerAction::Add(MixerChannelInit::new("system"))),
            added
        );
        assert_eq!(
            mixer_reducer(&added, MixerAction::Remove("mic".into()))
                .channels
                .len(),
            2
        );
    }

    #[test]
    fn silences_muted_channels_and_channels_outside_a_solo() {
        let mut muted = base_state();
        muted.set_muted("mic", true);
        assert!(!is_channel_audible(&muted, "mic"));
        let mut soloed = base_state();
        soloed.set_solo("music", true, false);
        assert!(!is_channel_audible(&soloed, "mic"));
        assert!(is_channel_audible(&soloed, "music"));
        assert!(!is_channel_audible(&soloed, "missing"));
    }

    #[test]
    fn manages_state_and_reports_dimmed_channels() {
        let mut mixer = MixerStore::new(MixerOptions {
            channels: vec![MixerChannelInit::new("a"), MixerChannelInit::new("b")],
            ..Default::default()
        });
        assert!(mixer.set_gain("a", -12.0));
        assert!(mixer.set_solo("b", true, false));
        assert_eq!(mixer.channel("a").unwrap().gain_db, -12.0);
        assert!(mixer.is_dimmed("a"));
        assert!(!mixer.is_dimmed("b"));
        assert!(mixer.is_audible("b"));
        assert!(mixer.reset());
        assert_eq!(mixer.channel("a").unwrap().gain_db, 0.0);
        assert!(!mixer.set_gain("a", 0.0));
    }

    #[test]
    fn master_and_monitor() {
        let mut state = base_state();
        state.set_master_gain(-3.0);
        state.set_master_muted(true);
        state.set_monitor("mic", true);
        state.set_pan("mic", -4.0);
        assert_eq!(
            state.master,
            MixerMasterState {
                gain_db: -3.0,
                muted: true
            }
        );
        assert!(state.channel("mic").unwrap().monitor);
        assert_eq!(state.channel("mic").unwrap().pan, -1.0);
    }
}
