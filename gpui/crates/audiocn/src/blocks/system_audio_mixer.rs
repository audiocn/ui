//! Port of `components/blocks/system-audio-mixer`.
//!
//! # Bring your own audio
//!
//! The React block opens a microphone, captures a screen or tab, plays tracks
//! and sound pads, and mixes them in Web Audio. Music ducks by 12 dB while the
//! microphone meter is louder than −35 dBFS. The mix is a `MediaStream`
//! (`onOutputChange`). None of that graph lives here: this crate does not open
//! devices (see `io`, behind the `device` feature) and the gallery has no mic.
//!
//! Pass what you have, and omit the rest:
//!
//! - [`.mixer`](SystemAudioMixer::mixer) — a [`MixerStore`] for gain, mute,
//!   solo, monitor and the master. Omit it and the block keeps one with the
//!   React defaults: microphone 0 dB, system −6 dB, music −12 dB monitored,
//!   sounds −6 dB monitored.
//! - [`.meter`](SystemAudioMixer::meter) / [`.master_meter`](SystemAudioMixer::master_meter)
//!   — a [`FrameSource`](crate::FrameSource) of [`MeterFrame`] per channel and
//!   for the master. Omit any of them and that meter is filled by
//!   [`use_demo_mixer`]: speech, noise, music and a pad bus, each silent until
//!   that source is actually started. While the microphone is live and audible,
//!   the music demo signal is lowered by 12 dB (the duck is held for the whole
//!   time the mic is open, rather than following the −35 dBFS threshold).
//! - Devices and transport: [`.devices`](SystemAudioMixer::devices),
//!   [`.microphone_status`](SystemAudioMixer::microphone_status),
//!   [`.on_microphone_start`](SystemAudioMixer::on_microphone_start),
//!   [`.system_status`](SystemAudioMixer::system_status),
//!   [`.music_playing`](SystemAudioMixer::music_playing),
//!   [`.sound_playback`](SystemAudioMixer::sound_playback), and the matching
//!   callbacks. Omit them and Start, the system switch, play / next and the
//!   pads drive local state. Tracks and pads then play through a
//!   [`use_simulated_player`](crate::use_simulated_player) (the gallery path).
//!
//! [`.persist_key`](SystemAudioMixer::persist_key) is accepted and not written.
//! [`MixerStore`] does not touch `localStorage`; save `mixer.read(cx).state`
//! yourself.
//!
//! [`.on_output_change`](SystemAudioMixer::on_output_change) is not a
//! `MediaStream`. It runs (deferred, once per change, including the first
//! frame) with a [`MixerActivity`] describing which inputs are open, so a host
//! can attach its own graph.
//!
//! ```ignore
//! SystemAudioMixer::new("studio")
//!     .mixer(mixer)
//!     .meter(MixerSourceId::Microphone, mic_meter)
//!     .master_meter(master_meter)
//!     .devices(devices)
//!     .on_microphone_start(|_, _| mic.start())
//!     .tracks(tracks)
//!     .sounds(sounds)
//! ```

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Entity, Hsla, IntoElement, ParentElement, Pixels, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::audio::decibels::{FormatDbOptions, format_db};
use crate::audio::taper::TaperInput;
use crate::audio::types::{MeterFrame, Orientation};
use crate::config::AudioSize;
use crate::signal::demo_signal::{
    DemoMixerChannel, DemoMixerHandle, DemoSignalKind, DemoSignalOptions, use_demo_mixer,
};
use crate::signal::mixer::{MixerChannelInit, MixerOptions, MixerState, MixerStore, use_mixer};
use crate::theme::ActiveTheme;
use crate::ui::icons::{Icon, IconName};
use crate::ui::{
    Button, ButtonSize, ButtonVariant, Popover, PopoverContent, Switch, SwitchSize, Tabs, TabsList,
    TabsTrigger,
};
use crate::{
    AudioDevice, AudioDeviceSelect, AudioPermission, AudioPlayerController, AudioPlayerStatus,
    ChannelStrip, ChannelStripActions, ChannelStripDescription, ChannelStripHeader,
    ChannelStripIcon, ChannelStripNotice, ChannelStripNoticeVariant, ChannelStripStatus,
    ChannelStripStatusTone, ChannelStripText, ChannelStripTitle, ChannelStripValue,
    ChannelStripVariant, ChannelToggle, ChannelToggleSize, Fader, FrameSource, LevelMeter,
    LevelMeterBar, LevelMeterChannel, LevelMeterChannels, LevelMeterClip, LevelMeterHold,
    LevelMeterTrack, Mixer, MixerActions, MixerChannels, MixerEmpty, MixerHeader, MixerMaster,
    MixerSeparator, MixerTitle, MonitorToggle, MuteToggle, SoloToggle, SoundPad, SoundPadGrid,
    SoundPadLabel, SoundPadProgress, SoundPadShortcut, SoundPadSize, UseAudioPlayerOptions,
    use_simulated_player,
};

/// `--channel-strip-header-width: 16rem` on the React block.
const HEADER_WIDTH: f32 = 256.0;
/// `amountDb` of the block's ducking rule.
const DUCK_DB: f32 = 12.0;
/// Fader floor, also the `floorDb` of the value readout.
const FADER_MIN_DB: f32 = -60.0;
/// Source faders (`max={12}`).
const SOURCE_MAX_DB: f32 = 12.0;
/// Master fader (`max={6}`).
const MASTER_MAX_DB: f32 = 6.0;

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type DeviceAction = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;
type CheckedAction = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
type SoundAction = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;
type OutputAction = Rc<dyn Fn(&MixerActivity, &mut Window, &mut App) + 'static>;

/// Which source strips to show (`"microphone" | "system" | "music" | "sounds"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MixerSourceId {
    Microphone,
    System,
    Music,
    Sounds,
}

impl MixerSourceId {
    /// The React default: all four, in strip order.
    pub const ALL: [Self; 4] = [Self::Microphone, Self::System, Self::Music, Self::Sounds];

    /// The mixer channel id (`"microphone"`, `"system"`, `"music"`, `"sounds"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::System => "system",
            Self::Music => "music",
            Self::Sounds => "sounds",
        }
    }
}

/// A track on the music channel (`MixerTrack`).
///
/// `duration` is the length in seconds of the built-in simulated player. A
/// host that plays the file itself can leave it at 0.
///
/// ```ignore
/// MixerTrack::new("intro", "Intro").artist("audiocn").src("/music/intro.mp3").duration(19.2)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct MixerTrack {
    pub id: SharedString,
    pub title: SharedString,
    pub artist: Option<SharedString>,
    pub src: SharedString,
    pub duration: f64,
}

impl MixerTrack {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        let id = id.into();
        Self {
            src: id.clone(),
            id,
            title: title.into(),
            artist: None,
            duration: 0.0,
        }
    }

    pub fn artist(mut self, artist: impl Into<SharedString>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    pub fn src(mut self, src: impl Into<SharedString>) -> Self {
        self.src = src.into();
        self
    }

    /// Length of the built-in player, in seconds.
    pub fn duration(mut self, seconds: f64) -> Self {
        self.duration = seconds;
        self
    }
}

/// A pad on the sounds channel (`MixerSound`).
///
/// `src` is whatever the host's player loads (`string | AudioBuffer` in
/// React). `duration` is the built-in player's length. `accent` is the pad
/// colour (`oklch(...)` in the docs).
///
/// ```ignore
/// MixerSound::new("ding", "Ding").src("/sounds/ding.mp3").hotkey("1").duration(1.7)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct MixerSound {
    pub id: SharedString,
    pub label: SharedString,
    pub src: SharedString,
    pub hotkey: Option<SharedString>,
    pub accent: Option<Hsla>,
    pub duration: f64,
}

impl MixerSound {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        let id = id.into();
        Self {
            src: id.clone(),
            id,
            label: label.into(),
            hotkey: None,
            accent: None,
            duration: 1.0,
        }
    }

    pub fn src(mut self, src: impl Into<SharedString>) -> Self {
        self.src = src.into();
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<SharedString>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn accent(mut self, accent: Hsla) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Length of the built-in one-shot, in seconds. Default 1.
    pub fn duration(mut self, seconds: f64) -> Self {
        self.duration = seconds;
        self
    }
}

/// Playback of one pad, when the host drives the pads itself.
///
/// ```ignore
/// MixerSoundPlayback { playing: true, loading: false, progress: Some(source) }
/// ```
#[derive(Clone, Debug, Default)]
pub struct MixerSoundPlayback {
    pub playing: bool,
    pub loading: bool,
    pub progress: Option<FrameSource<f32>>,
}

/// `status` of a source strip: a badge label and its tone.
///
/// ```ignore
/// MixerStripStatus::new(ChannelStripStatusTone::Live, "Live")
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixerStripStatus {
    pub label: SharedString,
    pub tone: ChannelStripStatusTone,
}

impl MixerStripStatus {
    pub fn new(tone: ChannelStripStatusTone, label: impl Into<SharedString>) -> Self {
        Self {
            tone,
            label: label.into(),
        }
    }
}

/// Microphone capture state (`MicrophoneStatus` of `useMicrophone`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MicrophoneStatus {
    #[default]
    Idle,
    Acquiring,
    Active,
    Denied,
    Unavailable,
    Error,
}

/// System-audio capture state (`SystemAudioStatus` of `useSystemAudio`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SystemAudioStatus {
    #[default]
    Idle,
    Prompting,
    Active,
    NoAudio,
    Denied,
    Ended,
    Unsupported,
}

/// `useMicrophone().status` from the native [`crate::io`] microphone.
#[cfg(all(feature = "device", not(target_family = "wasm")))]
impl From<crate::io::MicrophoneStatus> for MicrophoneStatus {
    fn from(status: crate::io::MicrophoneStatus) -> Self {
        use crate::io::MicrophoneStatus as Io;
        match status {
            Io::Idle => Self::Idle,
            Io::Acquiring => Self::Acquiring,
            Io::Active => Self::Active,
            Io::Denied => Self::Denied,
            Io::Unavailable => Self::Unavailable,
            Io::Error => Self::Error,
        }
    }
}

/// `useSystemAudio().status` from the native [`crate::io`] capture.
#[cfg(all(feature = "device", not(target_family = "wasm")))]
impl From<crate::io::SystemAudioStatus> for SystemAudioStatus {
    fn from(status: crate::io::SystemAudioStatus) -> Self {
        use crate::io::SystemAudioStatus as Io;
        match status {
            Io::Idle => Self::Idle,
            Io::Prompting => Self::Prompting,
            Io::Active => Self::Active,
            Io::NoAudio => Self::NoAudio,
            Io::Denied => Self::Denied,
            Io::Ended => Self::Ended,
            Io::Unsupported => Self::Unsupported,
        }
    }
}

/// Which inputs are open. The payload of [`SystemAudioMixer::on_output_change`],
/// standing in for React's `MediaStream | null`.
///
/// ```ignore
/// mixer.on_output_change(|activity, _, _| graph.attach(activity))
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MixerActivity {
    /// The microphone stream should be connected.
    pub microphone_open: bool,
    /// The system-audio stream should be connected.
    pub system_open: bool,
    /// The selected input device, when the host is choosing one.
    pub device_id: Option<SharedString>,
    /// The music element's source. `None` means there is no track.
    pub music_src: Option<SharedString>,
    /// A pad is currently playing into the sound bus.
    pub sounds_playing: bool,
}

/// What the demo meters should treat as producing audio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SourceActivity {
    microphone: bool,
    system: bool,
    music: bool,
    sounds: bool,
}

impl SourceActivity {
    fn live(self, id: &str) -> bool {
        match id {
            "microphone" => self.microphone,
            "system" => self.system,
            "music" => self.music,
            "sounds" => self.sounds,
            _ => true,
        }
    }
}

/// UI state the block owns when the host does not control it.
#[derive(Clone, Debug)]
struct MixerUi {
    orientation: Orientation,
    mic_active: bool,
    system_active: bool,
    device_id: Option<SharedString>,
    track_index: usize,
    /// The ended track was already advanced, so a stuck `Ended` status
    /// does not skip again.
    handled_end: bool,
    emitted: Option<MixerActivity>,
}

fn gain_label(db: f32) -> String {
    format_db(
        db,
        FormatDbOptions {
            floor_db: FADER_MIN_DB,
            ..Default::default()
        },
    )
}

fn orientation_value(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    }
}

fn orientation_from_value(value: &str) -> Orientation {
    if value == "vertical" {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    }
}

/// The React `CHANNELS` list: the channels [`SystemAudioMixer`] creates when
/// it is not given a [`SystemAudioMixer::mixer`]. Pass them to
/// [`use_mixer`] to own the mixer state (to apply its gains to real inputs).
pub fn system_audio_mixer_channels() -> Vec<MixerChannelInit> {
    default_channels()
}

fn default_channels() -> Vec<MixerChannelInit> {
    vec![
        MixerChannelInit::new("microphone"),
        MixerChannelInit::new("system").gain_db(-6.0),
        MixerChannelInit::new("music").gain_db(-12.0).monitor(true),
        MixerChannelInit::new("sounds").gain_db(-6.0).monitor(true),
    ]
}

/// One stereo demo signal per source. Seeds differ so the four don't match.
fn demo_inputs() -> Vec<DemoMixerChannel> {
    vec![
        DemoMixerChannel::new(
            "microphone",
            DemoSignalOptions::kind(DemoSignalKind::Speech)
                .with_channels(2)
                .with_seed(1.0),
        ),
        DemoMixerChannel::new(
            "system",
            DemoSignalOptions::kind(DemoSignalKind::Noise)
                .with_channels(2)
                .with_seed(2.0),
        ),
        DemoMixerChannel::new(
            "music",
            DemoSignalOptions::kind(DemoSignalKind::Music)
                .with_channels(2)
                .with_seed(3.0),
        ),
        DemoMixerChannel::new(
            "sounds",
            DemoSignalOptions::kind(DemoSignalKind::Music)
                .with_channels(2)
                .with_seed(5.0),
        ),
    ]
}

/// Mixer state the demo meters follow: real gain, mute and solo, but a
/// channel that is not actually producing audio is silenced, and music is
/// ducked by [`DUCK_DB`] while the microphone is live and audible.
fn demo_meter_state(real: &MixerState, activity: SourceActivity) -> MixerState {
    let duck_music = activity.microphone && real.is_audible("microphone");
    let mut state = real.clone();
    for channel in &mut state.channels {
        let id = channel.id.as_ref();
        if id == "music" && duck_music {
            channel.gain_db -= DUCK_DB;
        }
        if !activity.live(id) {
            channel.muted = true;
        }
    }
    state
}

fn microphone_badge(status: MicrophoneStatus, audible: bool) -> MixerStripStatus {
    match status {
        MicrophoneStatus::Active if audible => {
            MixerStripStatus::new(ChannelStripStatusTone::Live, "Live")
        }
        MicrophoneStatus::Active => MixerStripStatus::new(ChannelStripStatusTone::Muted, "Muted"),
        MicrophoneStatus::Denied => MixerStripStatus::new(ChannelStripStatusTone::Error, "Blocked"),
        MicrophoneStatus::Idle
        | MicrophoneStatus::Acquiring
        | MicrophoneStatus::Unavailable
        | MicrophoneStatus::Error => MixerStripStatus::new(ChannelStripStatusTone::Default, "Off"),
    }
}

fn system_badge(status: SystemAudioStatus) -> MixerStripStatus {
    match status {
        SystemAudioStatus::Active => MixerStripStatus::new(ChannelStripStatusTone::Live, "On"),
        SystemAudioStatus::Unsupported => {
            MixerStripStatus::new(ChannelStripStatusTone::Warning, "Unsupported")
        }
        SystemAudioStatus::Idle
        | SystemAudioStatus::Prompting
        | SystemAudioStatus::NoAudio
        | SystemAudioStatus::Denied
        | SystemAudioStatus::Ended => MixerStripStatus::new(ChannelStripStatusTone::Default, "Off"),
    }
}

fn device_label(devices: &[AudioDevice], id: &Option<SharedString>) -> SharedString {
    id.as_ref()
        .and_then(|id| devices.iter().find(|device| &device.id == id))
        .map(|device| device.label.clone())
        .unwrap_or_else(|| "Default microphone".into())
}

fn bind_gain(
    mixer: &Entity<MixerStore>,
    id: SharedString,
) -> impl Fn(f32, &mut Window, &mut App) + 'static {
    let mixer = mixer.clone();
    move |db, _, cx| {
        mixer.update(cx, |mixer, cx| {
            mixer.set_gain(&id, db);
            cx.notify();
        });
    }
}

fn bind_muted(
    mixer: &Entity<MixerStore>,
    id: SharedString,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let mixer = mixer.clone();
    move |muted, _, cx| {
        mixer.update(cx, |mixer, cx| {
            mixer.set_muted(&id, muted);
            cx.notify();
        });
    }
}

fn bind_solo(
    mixer: &Entity<MixerStore>,
    id: SharedString,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let mixer = mixer.clone();
    move |solo, _, cx| {
        mixer.update(cx, |mixer, cx| {
            mixer.set_solo(&id, solo, false);
            cx.notify();
        });
    }
}

fn bind_monitor(
    mixer: &Entity<MixerStore>,
    id: SharedString,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let mixer = mixer.clone();
    move |monitor, _, cx| {
        mixer.update(cx, |mixer, cx| {
            mixer.set_monitor(&id, monitor);
            cx.notify();
        });
    }
}

fn source_fader(
    element_id: &str,
    channel_id: &str,
    title: &str,
    gain_db: f32,
    mixer: &Entity<MixerStore>,
) -> Fader {
    Fader::new(format!("{element_id}/fader"))
        .aria_label(format!("{title} volume"))
        .max(SOURCE_MAX_DB)
        .min(FADER_MIN_DB)
        .on_value_change(bind_gain(mixer, channel_id.into()))
        .silence_at_min(true)
        .size(AudioSize::Sm)
        .taper(TaperInput::Audio)
        .value(gain_db)
}

struct ChannelToggles<'a> {
    element_id: &'a str,
    channel_id: &'a str,
    title: &'a str,
    muted: bool,
    solo: bool,
    monitor: bool,
    monitorable: bool,
}

fn source_toggles(spec: ChannelToggles<'_>, mixer: &Entity<MixerStore>) -> Vec<ChannelToggle> {
    let ChannelToggles {
        element_id,
        channel_id,
        title,
        muted,
        solo,
        monitor,
        monitorable,
    } = spec;
    let channel = SharedString::from(channel_id);
    let mut toggles = vec![
        MuteToggle::new(format!("{element_id}/mute"))
            .aria_label(format!("Mute {title}"))
            .on_pressed_change(bind_muted(mixer, channel.clone()))
            .pressed(muted)
            .size(ChannelToggleSize::Sm)
            .child("M"),
        SoloToggle::new(format!("{element_id}/solo"))
            .aria_label(format!("Solo {title}"))
            .on_pressed_change(bind_solo(mixer, channel.clone()))
            .pressed(solo)
            .size(ChannelToggleSize::Sm)
            .child("S"),
    ];
    if monitorable {
        toggles.push(
            MonitorToggle::new(format!("{element_id}/monitor"))
                .aria_label(format!("Monitor {title}"))
                .icon(IconName::Headphones)
                .on_pressed_change(bind_monitor(mixer, channel))
                .pressed(monitor)
                .size(ChannelToggleSize::Sm),
        );
    }
    toggles
}

/// One source strip of the system mixer: icon, status, meter, fader, mute,
/// solo and (usually) a monitor send.
///
/// Renders nothing when `id` is not a channel of `mixer`.
///
/// ```ignore
/// MixerSourceStrip::new("microphone")
///     .title("Microphone")
///     .mixer(mixer)
///     .meter(mic_meter)
///     .icon(Icon::new(IconName::Microphone))
/// ```
#[derive(IntoElement)]
pub struct MixerSourceStrip {
    channel_id: SharedString,
    element_id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    icon: Option<AnyElement>,
    accent: Option<Hsla>,
    mixer: Option<Entity<MixerStore>>,
    meter: Option<FrameSource<MeterFrame>>,
    status: Option<MixerStripStatus>,
    actions: Vec<AnyElement>,
    notice: Option<AnyElement>,
    monitorable: bool,
    disabled: bool,
    header_width: Option<Pixels>,
}

impl MixerSourceStrip {
    /// `id` is the mixer channel id.
    pub fn new(id: impl Into<SharedString>) -> Self {
        let channel_id = id.into();
        Self {
            element_id: channel_id.clone(),
            title: channel_id.clone(),
            channel_id,
            description: None,
            icon: None,
            accent: None,
            mixer: None,
            meter: None,
            status: None,
            actions: Vec::new(),
            notice: None,
            monitorable: true,
            disabled: false,
            header_width: None,
        }
    }

    /// GPUI element id. Default: the channel id. Set this when two mixers
    /// share a window.
    pub fn element_id(mut self, id: impl Into<SharedString>) -> Self {
        self.element_id = id.into();
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }

    /// The channel's colour tag. React passes `var(--chart-*)`; pass the
    /// theme colour (`cx.theme().chart_2`).
    pub fn accent(mut self, accent: impl Into<Hsla>) -> Self {
        self.accent = Some(accent.into());
        self
    }

    pub fn mixer(mut self, mixer: Entity<MixerStore>) -> Self {
        self.mixer = Some(mixer);
        self
    }

    pub fn meter(mut self, meter: FrameSource<MeterFrame>) -> Self {
        self.meter = Some(meter);
        self
    }

    pub fn status(mut self, status: MixerStripStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Adds one header action. Call again for the next button.
    pub fn actions(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// A [`ChannelStripNotice`], shown under the strip.
    pub fn notice(mut self, notice: impl IntoElement) -> Self {
        self.notice = Some(notice.into_any_element());
        self
    }

    /// Show the monitor toggle. Default true.
    pub fn monitorable(mut self, monitorable: bool) -> Self {
        self.monitorable = monitorable;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `--channel-strip-header-width` for this strip. Default 12rem.
    pub fn header_width(mut self, width: impl Into<Pixels>) -> Self {
        self.header_width = Some(width.into());
        self
    }
}

impl RenderOnce for MixerSourceStrip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(mixer) = self.mixer else {
            return div().into_any_element();
        };
        let Some(channel) = mixer.read(cx).channel(&self.channel_id).cloned() else {
            return div().into_any_element();
        };
        let dimmed = mixer.read(cx).is_dimmed(&self.channel_id);
        let id = self.element_id;
        let title = self.title;
        let mut text = ChannelStripText::new().child(ChannelStripTitle::new(title.clone()));
        if let Some(description) = self.description.filter(|text| !text.is_empty()) {
            text = text.child(ChannelStripDescription::new(description));
        }
        let mut header = ChannelStripHeader::new()
            .child(ChannelStripIcon::new().children(self.icon))
            .child(text);
        if let Some(status) = self.status {
            header = header.child(
                ChannelStripStatus::new()
                    .tone(status.tone)
                    .child(status.label),
            );
        }
        if !self.actions.is_empty() {
            header = header.child(ChannelStripActions::new().children(self.actions));
        }

        let mut meter = LevelMeter::new(format!("{id}/meter"))
            .aria_label(format!("{title} level"))
            .channel_count(2)
            .h_full()
            .size(AudioSize::Sm);
        if let Some(source) = self.meter {
            meter = meter.source(source);
        }

        let mut strip = ChannelStrip::new(id.clone())
            .dimmed(dimmed)
            .disabled(self.disabled)
            .muted(channel.muted)
            .solo(channel.solo)
            .when_some(self.accent, |strip, accent| strip.accent(accent))
            .when_some(self.header_width, |strip, width| strip.header_width(width))
            .child(header)
            .meter(meter)
            .fader(source_fader(
                &id,
                &self.channel_id,
                &title,
                channel.gain_db,
                &mixer,
            ))
            .child(ChannelStripValue::new().child(gain_label(channel.gain_db)))
            .controls(source_toggles(
                ChannelToggles {
                    element_id: &id,
                    channel_id: &self.channel_id,
                    title: &title,
                    muted: channel.muted,
                    solo: channel.solo,
                    monitor: channel.monitor,
                    monitorable: self.monitorable,
                },
                &mixer,
            ));
        if let Some(notice) = self.notice {
            strip = strip.child(notice);
        }
        strip.into_any_element()
    }
}

/// The master strip: a stereo meter with a clip count, a master fader and mute.
///
/// ```ignore
/// MixerMasterStrip::new("master").mixer(mixer).meter(master_meter)
/// ```
#[derive(IntoElement)]
pub struct MixerMasterStrip {
    id: SharedString,
    mixer: Option<Entity<MixerStore>>,
    meter: Option<FrameSource<MeterFrame>>,
    header_width: Option<Pixels>,
}

impl MixerMasterStrip {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            mixer: None,
            meter: None,
            header_width: None,
        }
    }

    pub fn mixer(mut self, mixer: Entity<MixerStore>) -> Self {
        self.mixer = Some(mixer);
        self
    }

    pub fn meter(mut self, meter: FrameSource<MeterFrame>) -> Self {
        self.meter = Some(meter);
        self
    }

    /// `--channel-strip-header-width` for this strip. Default 12rem.
    pub fn header_width(mut self, width: impl Into<Pixels>) -> Self {
        self.header_width = Some(width.into());
        self
    }
}

impl RenderOnce for MixerMasterStrip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (gain_db, muted) = self
            .mixer
            .as_ref()
            .map(|mixer| {
                let master = mixer.read(cx).master;
                (master.gain_db, master.muted)
            })
            .unwrap_or((0.0, false));
        let id = self.id;
        let mut meter = LevelMeter::new(format!("{id}/meter"))
            .aria_label("Master level")
            .h_full()
            .child(LevelMeterChannels::new().children((0..2).map(|index| {
                LevelMeterChannel::new().index(index).child(
                    LevelMeterTrack::new()
                        .child(LevelMeterBar::new())
                        .child(LevelMeterHold::new()),
                )
            })))
            .child(
                LevelMeterClip::new()
                    .id(SharedString::from(format!("{id}/clip")))
                    .show_count(true),
            );
        if let Some(source) = self.meter {
            meter = meter.source(source);
        }
        let mixer = self.mixer.clone();
        let mut fader = Fader::new(format!("{id}/fader"))
            .aria_label("Master volume")
            .max(MASTER_MAX_DB)
            .min(FADER_MIN_DB)
            .silence_at_min(true)
            .size(AudioSize::Sm)
            .taper(TaperInput::Audio)
            .value(gain_db);
        if let Some(mixer) = mixer.clone() {
            fader = fader.on_value_change(move |db, _, cx| {
                mixer.update(cx, |mixer, cx| {
                    mixer.set_master_gain(db);
                    cx.notify();
                });
            });
        }
        let mut mute = MuteToggle::new(format!("{id}/mute"))
            .aria_label("Mute master")
            .pressed(muted)
            .size(ChannelToggleSize::Sm)
            .child("M");
        if let Some(mixer) = mixer {
            mute = mute.on_pressed_change(move |muted, _, cx| {
                mixer.update(cx, |mixer, cx| {
                    mixer.set_master_muted(muted);
                    cx.notify();
                });
            });
        }
        ChannelStrip::new(id)
            .muted(muted)
            .variant(ChannelStripVariant::Master)
            .when_some(self.header_width, |strip, width| strip.header_width(width))
            .child(
                ChannelStripHeader::new()
                    .child(ChannelStripIcon::new().child(Icon::new(IconName::SpeakerHigh)))
                    .child(
                        ChannelStripText::new()
                            .child(ChannelStripTitle::new("Master"))
                            .child(ChannelStripDescription::new("Mix output")),
                    ),
            )
            .meter(meter)
            .fader(fader)
            .child(ChannelStripValue::new().child(gain_label(gain_db)))
            .controls([mute])
    }
}

/// The complete mixer: microphone, system audio, music, sound pads and a
/// master, with a rows / console switch.
///
/// See the module docs for the audio seam. With no meters and no callbacks
/// this is the gallery: demo signals, and simulated tracks and pads.
///
/// ```ignore
/// SystemAudioMixer::new("mixer")
///     .tracks(tracks)
///     .sounds(sounds)
/// ```
#[derive(IntoElement)]
pub struct SystemAudioMixer {
    id: SharedString,
    default_orientation: Orientation,
    sources: Option<Vec<MixerSourceId>>,
    /// Accepted for API parity. See the module docs.
    #[allow(dead_code)]
    persist_key: Option<SharedString>,
    on_output_change: Option<OutputAction>,
    tracks: Vec<MixerTrack>,
    sounds: Vec<MixerSound>,
    mixer: Option<Entity<MixerStore>>,
    meters: HashMap<MixerSourceId, FrameSource<MeterFrame>>,
    master_meter: Option<FrameSource<MeterFrame>>,
    devices: Vec<AudioDevice>,
    devices_loading: bool,
    permission: AudioPermission,
    on_request_permission: Option<Action>,
    /// `Some` makes the device controlled (`Some(None)` is the default device).
    device_id: Option<Option<SharedString>>,
    on_device_change: Option<DeviceAction>,
    microphone_status: Option<MicrophoneStatus>,
    on_microphone_start: Option<Action>,
    on_microphone_stop: Option<Action>,
    system_status: Option<SystemAudioStatus>,
    system_supported: bool,
    on_system_change: Option<CheckedAction>,
    music_playing: Option<bool>,
    on_music_toggle: Option<Action>,
    track_index: Option<usize>,
    on_next_track: Option<Action>,
    sound_playback: Option<Vec<(SharedString, MixerSoundPlayback)>>,
    on_sound_trigger: Option<SoundAction>,
    style: StyleRefinement,
}

impl SystemAudioMixer {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            default_orientation: Orientation::Horizontal,
            sources: None,
            persist_key: None,
            on_output_change: None,
            tracks: Vec::new(),
            sounds: Vec::new(),
            mixer: None,
            meters: HashMap::new(),
            master_meter: None,
            devices: Vec::new(),
            devices_loading: false,
            permission: AudioPermission::Prompt,
            on_request_permission: None,
            device_id: None,
            on_device_change: None,
            microphone_status: None,
            on_microphone_start: None,
            on_microphone_stop: None,
            system_status: None,
            system_supported: true,
            on_system_change: None,
            music_playing: None,
            on_music_toggle: None,
            track_index: None,
            on_next_track: None,
            sound_playback: None,
            on_sound_trigger: None,
            style: StyleRefinement::default(),
        }
    }

    /// Rows or console strips. Default [`Orientation::Horizontal`].
    pub fn default_orientation(mut self, orientation: Orientation) -> Self {
        self.default_orientation = orientation;
        self
    }

    /// Which strips to show. Default all four, in microphone / system / music
    /// / sounds order. An empty list shows the empty state.
    pub fn sources(mut self, sources: impl IntoIterator<Item = MixerSourceId>) -> Self {
        self.sources = Some(sources.into_iter().collect());
        self
    }

    /// Storage key for a host that persists [`MixerState`]. Not written here.
    pub fn persist_key(mut self, key: impl Into<SharedString>) -> Self {
        self.persist_key = Some(key.into());
        self
    }

    /// Called when the set of live inputs changes. There is no `MediaStream`;
    /// see [`MixerActivity`].
    pub fn on_output_change(
        mut self,
        f: impl Fn(&MixerActivity, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_output_change = Some(Rc::new(f));
        self
    }

    /// Tracks for the music channel. Default none.
    pub fn tracks(mut self, tracks: impl IntoIterator<Item = MixerTrack>) -> Self {
        self.tracks = tracks.into_iter().collect();
        self
    }

    /// Sounds for the sounds channel. Default none.
    pub fn sounds(mut self, sounds: impl IntoIterator<Item = MixerSound>) -> Self {
        self.sounds = sounds.into_iter().collect();
        self
    }

    /// Mixer gain state. Omit to keep one internally.
    pub fn mixer(mut self, mixer: Entity<MixerStore>) -> Self {
        self.mixer = Some(mixer);
        self
    }

    /// Post-fader meter for one source. Omit to use the demo signal.
    pub fn meter(mut self, id: MixerSourceId, source: FrameSource<MeterFrame>) -> Self {
        self.meters.insert(id, source);
        self
    }

    /// Master meter. Omit to use the demo mix.
    pub fn master_meter(mut self, source: FrameSource<MeterFrame>) -> Self {
        self.master_meter = Some(source);
        self
    }

    /// Microphones listed in the settings popover.
    pub fn devices(mut self, devices: impl IntoIterator<Item = AudioDevice>) -> Self {
        self.devices = devices.into_iter().collect();
        self
    }

    /// Shows "Finding devices…" in the device picker. Default false.
    pub fn devices_loading(mut self, loading: bool) -> Self {
        self.devices_loading = loading;
        self
    }

    /// Microphone permission. Default [`AudioPermission::Prompt`].
    /// `Unsupported` is shown as denied, like the React block.
    pub fn permission(mut self, permission: AudioPermission) -> Self {
        self.permission = permission;
        self
    }

    pub fn on_request_permission(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_permission = Some(Rc::new(f));
        self
    }

    /// Controlled input device. `None` is the default device.
    pub fn device_id(mut self, id: Option<SharedString>) -> Self {
        self.device_id = Some(id);
        self
    }

    pub fn on_device_change(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_device_change = Some(Rc::new(f));
        self
    }

    /// Controlled microphone status. Omit and Start / Stop toggle a local one.
    pub fn microphone_status(mut self, status: MicrophoneStatus) -> Self {
        self.microphone_status = Some(status);
        self
    }

    pub fn on_microphone_start(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_microphone_start = Some(Rc::new(f));
        self
    }

    pub fn on_microphone_stop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_microphone_stop = Some(Rc::new(f));
        self
    }

    /// Controlled system-capture status.
    pub fn system_status(mut self, status: SystemAudioStatus) -> Self {
        self.system_status = Some(status);
        self
    }

    /// The platform can capture a screen or tab. Default true (the block does
    /// not probe). `false` forces [`SystemAudioStatus::Unsupported`].
    pub fn system_supported(mut self, supported: bool) -> Self {
        self.system_supported = supported;
        self
    }

    /// `checked` is the next switch state (`onCheckedChange`).
    pub fn on_system_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_system_change = Some(Rc::new(f));
        self
    }

    /// Controlled music transport. Omit and the built-in player drives it.
    pub fn music_playing(mut self, playing: bool) -> Self {
        self.music_playing = Some(playing);
        self
    }

    pub fn on_music_toggle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_music_toggle = Some(Rc::new(f));
        self
    }

    /// Controlled track index.
    pub fn track_index(mut self, index: usize) -> Self {
        self.track_index = Some(index);
        self
    }

    pub fn on_next_track(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next_track = Some(Rc::new(f));
        self
    }

    /// Host-driven pad playback, in sound id order. Omit and each pad is a
    /// simulated one-shot of [`MixerSound::duration`].
    pub fn sound_playback(
        mut self,
        playback: impl IntoIterator<Item = (impl Into<SharedString>, MixerSoundPlayback)>,
    ) -> Self {
        self.sound_playback = Some(
            playback
                .into_iter()
                .map(|(id, playback)| (id.into(), playback))
                .collect(),
        );
        self
    }

    /// A pad was triggered. The id is the [`MixerSound::id`].
    pub fn on_sound_trigger(
        mut self,
        f: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sound_trigger = Some(Rc::new(f));
        self
    }

    fn shown_sources(&self) -> Vec<MixerSourceId> {
        let sources = self.sources.as_deref().unwrap_or(&MixerSourceId::ALL);
        MixerSourceId::ALL
            .into_iter()
            .filter(|id| sources.contains(id))
            .collect()
    }
}

impl Styled for SystemAudioMixer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct MusicTransport {
    playing: bool,
    player: Option<Entity<crate::SimulatedPlayer>>,
}

struct MusicArgs<'a> {
    root: &'a str,
    tracks: &'a [MixerTrack],
    track_index: usize,
    music_playing: Option<bool>,
    handled_end: bool,
    track_controlled: bool,
    on_next: &'a Option<Action>,
}

fn music_transport(
    args: MusicArgs<'_>,
    ui: &Entity<MixerUi>,
    window: &mut Window,
    cx: &mut App,
) -> MusicTransport {
    let MusicArgs {
        root,
        tracks,
        track_index,
        music_playing,
        handled_end,
        track_controlled,
        on_next,
    } = args;
    let Some(track) = tracks.get(track_index) else {
        return MusicTransport {
            playing: music_playing.unwrap_or(false),
            player: None,
        };
    };
    if let Some(playing) = music_playing {
        return MusicTransport {
            playing,
            player: None,
        };
    }
    let player = use_simulated_player(
        format!("{root}/music"),
        track.duration,
        UseAudioPlayerOptions::src(track.src.clone()),
        window,
        cx,
    );
    let status = player.read(cx).state().status;
    let ended = status == AudioPlayerStatus::Ended;
    if ended && !handled_end {
        ui.update(cx, |ui, _| ui.handled_end = true);
        let ui = ui.clone();
        let on_next = on_next.clone();
        let count = tracks.len();
        window.defer(cx, move |window, cx| {
            if !track_controlled && count > 0 {
                ui.update(cx, |ui, cx| {
                    ui.track_index = (ui.track_index + 1) % count;
                    cx.notify();
                });
            }
            if let Some(on_next) = on_next {
                on_next(window, cx);
            }
        });
    } else if !ended && handled_end {
        ui.update(cx, |ui, _| ui.handled_end = false);
    }
    MusicTransport {
        playing: player.read(cx).state().playing,
        player: Some(player),
    }
}

struct PadView {
    pad: SoundPad,
    playing: bool,
}

fn sound_pads(
    root: &str,
    sounds: &[MixerSound],
    playback: &Option<Vec<(SharedString, MixerSoundPlayback)>>,
    on_trigger: &Option<SoundAction>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<PadView> {
    let external: Option<HashMap<&str, &MixerSoundPlayback>> = playback.as_ref().map(|list| {
        list.iter()
            .map(|(id, playback)| (id.as_ref(), playback))
            .collect()
    });
    let mut pads = Vec::with_capacity(sounds.len());
    for sound in sounds {
        let (playing, loading, progress, player) = if let Some(external) = &external {
            let playback = external.get(sound.id.as_ref()).copied();
            (
                playback.is_some_and(|playback| playback.playing),
                playback.is_some_and(|playback| playback.loading),
                playback.and_then(|playback| playback.progress.clone()),
                None,
            )
        } else {
            let player = use_simulated_player(
                format!("{root}/sound/{}", sound.id),
                sound.duration,
                UseAudioPlayerOptions::src(sound.src.clone()),
                window,
                cx,
            );
            let state = player.read(cx).state();
            (
                state.playing,
                state.status == AudioPlayerStatus::Loading,
                Some(player.read(cx).progress_source()),
                Some(player),
            )
        };
        let sound_id = sound.id.clone();
        let on_trigger = on_trigger.clone();
        let mut pad = SoundPad::new(format!("{root}/pad/{}", sound.id))
            .loading(loading)
            .on_trigger(move |window, cx| {
                if let Some(player) = &player {
                    player.seek(0.0, window, cx);
                    player.play(window, cx);
                }
                if let Some(on_trigger) = &on_trigger {
                    on_trigger(sound_id.clone(), window, cx);
                }
            })
            .playing(playing)
            .size(SoundPadSize::Sm)
            .child(SoundPadLabel::new().child(sound.label.clone()))
            .child(SoundPadShortcut::new());
        if let Some(hotkey) = sound.hotkey.clone() {
            pad = pad.hotkey(hotkey);
        }
        if let Some(accent) = sound.accent {
            pad = pad.accent(accent);
        }
        if let Some(progress) = progress {
            pad = pad.child(SoundPadProgress::new().source(progress));
        } else {
            pad = pad.child(SoundPadProgress::new());
        }
        pads.push(PadView { pad, playing });
    }
    pads
}

impl RenderOnce for SystemAudioMixer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root = self.id.clone();
        let initial_orientation = self.default_orientation;
        let ui =
            window.use_keyed_state(SharedString::from(format!("{root}/ui")), cx, move |_, _| {
                MixerUi {
                    orientation: initial_orientation,
                    mic_active: false,
                    system_active: false,
                    device_id: None,
                    track_index: 0,
                    handled_end: false,
                    emitted: None,
                }
            });
        let state = ui.read(cx).clone();
        let orientation = state.orientation;
        let console = orientation == Orientation::Vertical;
        let track_index = self.track_index.unwrap_or(state.track_index);
        let device_controlled = self.device_id.is_some();
        let device_id = match &self.device_id {
            Some(id) => id.clone(),
            None => state.device_id.clone(),
        };
        let mic_controlled = self.microphone_status.is_some();
        let mic_status = self.microphone_status.unwrap_or(if state.mic_active {
            MicrophoneStatus::Active
        } else {
            MicrophoneStatus::Idle
        });
        let system_status = if !self.system_supported {
            SystemAudioStatus::Unsupported
        } else {
            self.system_status.unwrap_or(if state.system_active {
                SystemAudioStatus::Active
            } else {
                SystemAudioStatus::Idle
            })
        };
        let system_controlled = self.system_status.is_some() || !self.system_supported;

        let mixer = if let Some(mixer) = self.mixer.clone() {
            mixer
        } else {
            use_mixer(
                SharedString::from(format!("{root}/mixer")),
                MixerOptions {
                    channels: default_channels(),
                    ..Default::default()
                },
                window,
                cx,
            )
        };

        let music = music_transport(
            MusicArgs {
                root: &root,
                tracks: &self.tracks,
                track_index,
                music_playing: self.music_playing,
                handled_end: state.handled_end,
                track_controlled: self.track_index.is_some(),
                on_next: &self.on_next_track,
            },
            &ui,
            window,
            cx,
        );
        let pads = sound_pads(
            &root,
            &self.sounds,
            &self.sound_playback,
            &self.on_sound_trigger,
            window,
            cx,
        );
        let sounds_playing = pads.iter().any(|pad| pad.playing);
        let activity = SourceActivity {
            microphone: mic_status == MicrophoneStatus::Active,
            system: system_status == SystemAudioStatus::Active,
            music: music.playing,
            sounds: sounds_playing,
        };

        let shown = self.shown_sources();
        let need_demo =
            self.master_meter.is_none() || shown.iter().any(|id| !self.meters.contains_key(id));
        let demo: Option<DemoMixerHandle> = need_demo.then(|| {
            let shadow = demo_meter_state(&mixer.read(cx).state, activity);
            let inputs = demo_inputs();
            use_demo_mixer(
                SharedString::from(format!("{root}/demo")),
                &inputs,
                &shadow,
                window,
                cx,
            )
        });
        let master_source = self
            .master_meter
            .clone()
            .or_else(|| demo.as_ref().map(|demo| demo.master.clone()));
        let meter_for = |id: MixerSourceId| {
            self.meters.get(&id).cloned().or_else(|| {
                demo.as_ref()
                    .and_then(|demo| demo.source(id.as_str()))
                    .or_else(|| master_source.clone())
            })
        };

        let output = MixerActivity {
            microphone_open: activity.microphone,
            system_open: activity.system,
            device_id: device_id.clone(),
            music_src: self.tracks.get(track_index).map(|track| track.src.clone()),
            sounds_playing,
        };
        if self.on_output_change.is_some() && state.emitted.as_ref() != Some(&output) {
            ui.update(cx, |ui, _| ui.emitted = Some(output.clone()));
            if let Some(callback) = self.on_output_change.clone() {
                window.defer(cx, move |window, cx| callback(&output, window, cx));
            }
        }

        let theme = cx.theme();
        let accents = (theme.chart_1, theme.chart_2, theme.chart_3, theme.chart_4);
        let audible = |id: &str| mixer.read(cx).is_audible(id);
        let present = |id: &str| mixer.read(cx).channel(id).is_some();
        let width = px(HEADER_WIDTH);

        let mut channels: Vec<AnyElement> = Vec::new();
        if shown.contains(&MixerSourceId::Microphone) && present("microphone") {
            let active = mic_status == MicrophoneStatus::Active;
            let on_start = self.on_microphone_start.clone();
            let on_stop = self.on_microphone_stop.clone();
            let ui_mic = ui.clone();
            let on_device = self.on_device_change.clone();
            let ui_device = ui.clone();
            let mut select = AudioDeviceSelect::new(
                SharedString::from(format!("{root}/devices")),
                self.devices.clone(),
            )
            .loading(self.devices_loading)
            .on_value_change(move |id, window, cx| {
                if let Some(on_device) = on_device.clone() {
                    on_device(id.clone(), window, cx);
                }
                if !device_controlled {
                    ui_device.update(cx, |ui, cx| {
                        ui.device_id = id;
                        cx.notify();
                    });
                }
            })
            .permission(match self.permission {
                AudioPermission::Unsupported => AudioPermission::Denied,
                permission => permission,
            })
            .value(device_id.clone());
            if let Some(on_request) = self.on_request_permission.clone() {
                select = select.on_request_permission(move |window, cx| on_request(window, cx));
            }
            let description: SharedString = if active {
                device_label(&self.devices, &device_id)
            } else {
                "Not listening".into()
            };
            let mut strip = MixerSourceStrip::new("microphone")
                .element_id(format!("{root}/microphone"))
                .title("Microphone")
                .description(description)
                .icon(Icon::new(IconName::Microphone))
                .accent(accents.1)
                .header_width(width)
                .mixer(mixer.clone())
                .status(microphone_badge(mic_status, audible("microphone")))
                .actions(
                    Popover::new(SharedString::from(format!("{root}/mic-settings")))
                        .trigger(
                            Button::new(SharedString::from(format!("{root}/mic-settings-btn")))
                                .aria_label("Microphone settings")
                                .icon(IconName::GearSix)
                                .size(ButtonSize::IconXs)
                                .variant(ButtonVariant::Ghost),
                        )
                        .content(PopoverContent::new().child(select)),
                )
                .actions(
                    Button::new(SharedString::from(format!("{root}/mic-toggle")))
                        .on_click(move |_, window, cx| {
                            if active {
                                if let Some(on_stop) = on_stop.clone() {
                                    on_stop(window, cx);
                                }
                                if !mic_controlled {
                                    ui_mic.update(cx, |ui, cx| {
                                        ui.mic_active = false;
                                        cx.notify();
                                    });
                                }
                            } else {
                                if let Some(on_start) = on_start.clone() {
                                    on_start(window, cx);
                                }
                                if !mic_controlled {
                                    ui_mic.update(cx, |ui, cx| {
                                        ui.mic_active = true;
                                        cx.notify();
                                    });
                                }
                            }
                        })
                        .size(ButtonSize::Xs)
                        .variant(if active {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Outline
                        })
                        .child(if active { "Stop" } else { "Start" }),
                );
            if let Some(source) = meter_for(MixerSourceId::Microphone) {
                strip = strip.meter(source);
            }
            if mic_status == MicrophoneStatus::Denied {
                strip = strip.notice(
                    ChannelStripNotice::new()
                        .variant(ChannelStripNoticeVariant::Destructive)
                        .child(
                            "Microphone access is blocked. Allow it in your browser's site settings.",
                        ),
                );
            }
            channels.push(strip.into_any_element());
        }

        if shown.contains(&MixerSourceId::System) && present("system") {
            let active = system_status == SystemAudioStatus::Active;
            let on_system = self.on_system_change.clone();
            let ui_system = ui.clone();
            let mut strip = MixerSourceStrip::new("system")
                .element_id(format!("{root}/system"))
                .title("System audio")
                .description(if active {
                    "Capturing"
                } else {
                    "Share a screen or tab with audio"
                })
                .icon(Icon::new(IconName::Desktop))
                .accent(accents.3)
                .header_width(width)
                .mixer(mixer.clone())
                .monitorable(false)
                .status(system_badge(system_status))
                .actions(
                    Switch::new(SharedString::from(format!("{root}/system-switch")))
                        .checked(active)
                        .disabled(system_status == SystemAudioStatus::Unsupported)
                        .on_checked_change(move |checked, window, cx| {
                            if let Some(on_system) = on_system.clone() {
                                on_system(checked, window, cx);
                            }
                            if !system_controlled {
                                ui_system.update(cx, |ui, cx| {
                                    ui.system_active = checked;
                                    cx.notify();
                                });
                            }
                        })
                        .size(SwitchSize::Sm),
                );
            if let Some(source) = meter_for(MixerSourceId::System) {
                strip = strip.meter(source);
            }
            if system_status == SystemAudioStatus::NoAudio {
                strip = strip.notice(
                    ChannelStripNotice::new()
                        .variant(ChannelStripNoticeVariant::Warning)
                        .child("Nothing to hear: tick \"Share audio\" in the browser's picker."),
                );
            }
            channels.push(strip.into_any_element());
        }

        if shown.contains(&MixerSourceId::Music) && present("music") {
            let track = self.tracks.get(track_index);
            let on_toggle = self.on_music_toggle.clone();
            let player = music.player.clone();
            let playing = music.playing;
            let on_next = self.on_next_track.clone();
            let ui_next = ui.clone();
            let track_controlled = self.track_index.is_some();
            let track_count = self.tracks.len();
            let mut strip = MixerSourceStrip::new("music")
                .element_id(format!("{root}/music-strip"))
                .title("Music")
                .description(
                    track
                        .map(|track| track.title.clone())
                        .unwrap_or_else(|| "No tracks".into()),
                )
                .icon(Icon::new(IconName::MusicNotes))
                .accent(accents.0)
                .header_width(width)
                .mixer(mixer.clone())
                .actions(
                    Button::new(SharedString::from(format!("{root}/play")))
                        .aria_label(if playing { "Pause music" } else { "Play music" })
                        .disabled(track.is_none())
                        .icon(if playing {
                            IconName::Pause
                        } else {
                            IconName::Play
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(on_toggle) = on_toggle.clone() {
                                on_toggle(window, cx);
                            }
                            if let Some(player) = player.clone() {
                                player.toggle(window, cx);
                            }
                        })
                        .size(ButtonSize::IconXs)
                        .variant(ButtonVariant::Ghost),
                )
                .actions(
                    Button::new(SharedString::from(format!("{root}/next")))
                        .aria_label("Next track")
                        .disabled(track_count < 2)
                        .icon(IconName::SkipForward)
                        .on_click(move |_, window, cx| {
                            if let Some(on_next) = on_next.clone() {
                                on_next(window, cx);
                            }
                            if !track_controlled && track_count > 0 {
                                ui_next.update(cx, |ui, cx| {
                                    ui.track_index = (ui.track_index + 1) % track_count;
                                    ui.handled_end = false;
                                    cx.notify();
                                });
                            }
                        })
                        .size(ButtonSize::IconXs)
                        .variant(ButtonVariant::Ghost),
                );
            if playing {
                strip = strip.status(MixerStripStatus::new(
                    ChannelStripStatusTone::Live,
                    "Playing",
                ));
            }
            if let Some(source) = meter_for(MixerSourceId::Music) {
                strip = strip.meter(source);
            }
            channels.push(strip.into_any_element());
        }

        if shown.contains(&MixerSourceId::Sounds) && present("sounds") {
            let pad_count = self.sounds.len();
            let mut grid = SoundPadGrid::new(SharedString::from(format!("{root}/pads")))
                .columns(4)
                .hotkeys(true)
                .pad_min_width(px(64.0));
            for pad in pads {
                grid = grid.child(pad.pad);
            }
            let mut strip = MixerSourceStrip::new("sounds")
                .element_id(format!("{root}/sounds"))
                .title("Sounds")
                .description(format!("{pad_count} pads"))
                .icon(Icon::new(IconName::Waveform))
                .accent(accents.2)
                .header_width(width)
                .mixer(mixer.clone())
                .actions(
                    Popover::new(SharedString::from(format!("{root}/pads-popover")))
                        .trigger(
                            Button::new(SharedString::from(format!("{root}/pads-btn")))
                                .aria_label("Sound pads")
                                .disabled(pad_count == 0)
                                .icon(IconName::SquaresFour)
                                .size(ButtonSize::IconXs)
                                .variant(ButtonVariant::Ghost),
                        )
                        .content(PopoverContent::new().w(px(320.0)).child(grid)),
                );
            if let Some(source) = meter_for(MixerSourceId::Sounds) {
                strip = strip.meter(source);
            }
            channels.push(strip.into_any_element());
        }

        let ui_tabs = ui.clone();
        let tabs = Tabs::new(SharedString::from(format!("{root}/layout")))
            .on_value_change(move |value, _, cx| {
                let orientation = orientation_from_value(&value);
                ui_tabs.update(cx, |ui, cx| {
                    ui.orientation = orientation;
                    cx.notify();
                });
            })
            .value(orientation_value(orientation))
            .child(
                TabsList::new()
                    .child(TabsTrigger::new("horizontal").child("Rows"))
                    .child(TabsTrigger::new("vertical").child("Console")),
            );
        let ui_reset = mixer.clone();
        let reset = Button::new(SharedString::from(format!("{root}/reset")))
            .aria_label("Reset mixer")
            .icon(IconName::ArrowCounterClockwise)
            .on_click(move |_, _, cx| {
                ui_reset.update(cx, |mixer, cx| {
                    mixer.reset();
                    cx.notify();
                });
            })
            .size(ButtonSize::IconSm)
            .variant(ButtonVariant::Ghost);

        let mut master = MixerMasterStrip::new(format!("{root}/master"))
            .header_width(width)
            .mixer(mixer);
        if let Some(source) = master_source {
            master = master.meter(source);
        }

        let mut mixer = Mixer::new(root)
            .orientation(orientation)
            .child(
                MixerHeader::new()
                    .child(MixerTitle::new("Audio mixer"))
                    .child(MixerActions::new().child(tabs).child(reset)),
            )
            // In the console the CSS grid stretches both areas to the row's
            // height; here they sit in flex columns, so they grow instead.
            .child(
                MixerChannels::new()
                    .children(channels)
                    .when(console, |el| el.flex_1()),
            )
            .child(MixerEmpty::new().child("No audio sources."))
            .child(MixerSeparator::new())
            .child(
                MixerMaster::new()
                    .child(master)
                    .when(console, |el| el.flex_1()),
            );
        mixer.style().refine(&self.style);
        mixer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_badges_match_the_react_helper() {
        assert_eq!(
            microphone_badge(MicrophoneStatus::Active, true)
                .label
                .as_ref(),
            "Live"
        );
        assert_eq!(
            microphone_badge(MicrophoneStatus::Active, false).tone,
            ChannelStripStatusTone::Muted
        );
        assert_eq!(
            microphone_badge(MicrophoneStatus::Denied, true)
                .label
                .as_ref(),
            "Blocked"
        );
        assert_eq!(
            microphone_badge(MicrophoneStatus::Idle, true)
                .label
                .as_ref(),
            "Off"
        );
        assert_eq!(
            microphone_badge(MicrophoneStatus::Error, false).tone,
            ChannelStripStatusTone::Default
        );
    }

    #[test]
    fn system_badges_match_the_react_helper() {
        assert_eq!(system_badge(SystemAudioStatus::Active).label.as_ref(), "On");
        assert_eq!(
            system_badge(SystemAudioStatus::Unsupported).tone,
            ChannelStripStatusTone::Warning
        );
        assert_eq!(
            system_badge(SystemAudioStatus::NoAudio).label.as_ref(),
            "Off"
        );
        assert_eq!(
            system_badge(SystemAudioStatus::Denied).tone,
            ChannelStripStatusTone::Default
        );
    }

    #[test]
    fn demo_meters_stay_silent_until_a_source_starts_and_duck_music() {
        let real = MixerState::new(MixerOptions {
            channels: default_channels(),
            ..Default::default()
        });
        let silent = demo_meter_state(&real, SourceActivity::default());
        assert!(silent.channel("microphone").unwrap().muted);
        assert!(silent.channel("music").unwrap().muted);
        assert!((silent.channel("music").unwrap().gain_db + 12.0).abs() < f32::EPSILON);

        let live = demo_meter_state(
            &real,
            SourceActivity {
                microphone: true,
                music: true,
                ..Default::default()
            },
        );
        assert!(!live.channel("microphone").unwrap().muted);
        assert!(!live.channel("music").unwrap().muted);
        assert!((live.channel("music").unwrap().gain_db + 24.0).abs() < f32::EPSILON);

        let mut muted_mic = real.clone();
        muted_mic.set_muted("microphone", true);
        let no_duck = demo_meter_state(
            &muted_mic,
            SourceActivity {
                microphone: true,
                music: true,
                ..Default::default()
            },
        );
        assert!((no_duck.channel("music").unwrap().gain_db + 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sources_keep_the_react_order() {
        let mixer =
            SystemAudioMixer::new("m").sources([MixerSourceId::Sounds, MixerSourceId::Microphone]);
        assert_eq!(
            mixer.shown_sources(),
            vec![MixerSourceId::Microphone, MixerSourceId::Sounds]
        );
        assert!(
            SystemAudioMixer::new("m")
                .sources([])
                .shown_sources()
                .is_empty()
        );
    }
}
