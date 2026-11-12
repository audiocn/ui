//! Port of `components/blocks/mic-setup/mic-setup.tsx`.
//!
//! # Audio seam
//!
//! The React block calls `useAudioDevices`, `useMicrophone`, `useGainNode` and
//! `useAudioAnalyser`, which open a real microphone. This one does not. Pass
//! the devices, the permission, the capture [`MicrophoneStatus`] and the
//! analyser frame sources yourself:
//!
//! ```ignore
//! MicSetup::new("mic")
//!     .devices(devices)
//!     .permission(AudioPermission::Granted)
//!     .status(MicrophoneStatus::Active)
//!     .meter_source(meter)
//!     .visual_source(visual)
//!     .on_start(|_, cx| open_microphone(cx))
//!     .on_device_change(|id, _, cx| select_device(id, cx))
//!     .on_gain_change(|db, _, cx| set_gain(db, cx))
//!     .on_muted_change(|muted, _, cx| set_muted(muted, cx))
//! ```
//!
//! Gain and mute are reported, not applied to a stream you passed in: shape
//! the frames before you emit them (React does this with a `GainNode` in front
//! of the analyser). Omit the sources and the status and, once "Turn on
//! microphone" is pressed (or [`MicSetup::auto_start`] is set), a speech
//! [`crate::signal::demo_signal::DemoSignal`] drives the waveform and the meter.
//! Its gain and mute follow the controls, which is what the gallery uses.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::system_audio_mixer::MicrophoneStatus;
use crate::audio::{FrameSource, MeterFrame, VisualFrame};
use crate::components::{
    AudioDevice, AudioDeviceSelect, AudioDeviceSelectContent, AudioDeviceSelectPreview,
    AudioDeviceSelectTrigger, AudioDeviceSelectValue, AudioPermission, DbReadout, LevelMeter,
    LevelMeterBar, LevelMeterChannel, LevelMeterChannels, LevelMeterClip, LevelMeterHold,
    LevelMeterScale, LevelMeterTrack, LiveWaveform, LiveWaveformMode, ParameterSlider,
    ParameterSliderControl, ParameterSliderHeader, ParameterSliderInput, ParameterSliderLabel,
    ParameterSliderReset,
};
use crate::motion::{Duration, Instant};
use crate::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use crate::theme::ActiveTheme;
use crate::ui::icons::{Icon, IconName};
use crate::ui::styles::TextSize;
use crate::ui::{
    Alert, AlertDescription, AlertTitle, AlertVariant, Button, ButtonSize, ButtonVariant, Card,
    CardContent, CardDescription, CardHeader, CardTitle, Field, FieldGroup, FieldLabel,
    FieldOrientation, Switch,
};

/// How long "Check level" listens before judging the loudest peak.
const CHECK_DURATION: Duration = Duration::from_millis(3000);
/// `useAudioAnalyser({ historySize: 120 })`.
const PREVIEW_HISTORY: usize = 120;
/// React `--font-heading` (Outfit). Bundled with the gallery.
const HEADING_FONT: &str = "Outfit";

type DeviceHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;
type GainHandler = Rc<dyn Fn(f32, &mut Window, &mut App) + 'static>;
type MutedHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// The three-second level check's verdict (`judge` in the React block).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheckResult {
    Good,
    Quiet,
    Loud,
    Silent,
}

struct Verdict {
    title: &'static str,
    description: &'static str,
    ok: bool,
}

fn judge(peak_db: f32) -> CheckResult {
    if peak_db < -55.0 {
        CheckResult::Silent
    } else if peak_db >= -1.0 {
        CheckResult::Loud
    } else if peak_db < -30.0 {
        CheckResult::Quiet
    } else {
        CheckResult::Good
    }
}

fn verdict_of(result: CheckResult) -> Verdict {
    match result {
        CheckResult::Good => Verdict {
            title: "Sounds good",
            description: "Your level sits in the right range.",
            ok: true,
        },
        CheckResult::Loud => Verdict {
            title: "Too loud",
            description: "Your voice clipped. Lower the gain.",
            ok: false,
        },
        CheckResult::Quiet => Verdict {
            title: "Too quiet",
            description: "Raise the gain or move closer.",
            ok: false,
        },
        CheckResult::Silent => Verdict {
            title: "No signal",
            description: "Check the device and that it is not muted.",
            ok: false,
        },
    }
}

fn loudest_peak(source: &FrameSource<MeterFrame>) -> Option<f32> {
    source.with_latest(|frame| {
        frame.and_then(|frame| {
            frame
                .channels
                .iter()
                .map(|level| level.peak_db)
                .reduce(f32::max)
        })
    })
}

fn child_id(id: &ElementId, suffix: &str) -> ElementId {
    ElementId::Name(format!("{id}-{suffix}").into())
}

struct MicState {
    started: bool,
    /// `on_start` has been delivered for this `started` transition.
    announced_start: bool,
    device_id: Option<SharedString>,
    gain_db: f32,
    muted: bool,
    checking: bool,
    check_started: Instant,
    peak_db: f32,
    result: Option<CheckResult>,
}

/// Choose and check a microphone: device picker, scrolling preview, meter,
/// gain, mute and a three-second level check.
///
/// Controlled the same way as React: [`.device_id`](Self::device_id),
/// [`.gain_db`](Self::gain_db) and [`.muted`](Self::muted) follow the prop and
/// only report changes. Leave them unset and the block keeps the value.
///
/// ```ignore
/// MicSetup::new("setup").auto_start(true).max_w(px(448.))
/// ```
#[derive(IntoElement)]
pub struct MicSetup {
    id: ElementId,
    /// `Some` makes the device controlled (`None` inside is "no device").
    device_id: Option<Option<SharedString>>,
    on_device_change: Option<DeviceHandler>,
    gain_db: Option<f32>,
    on_gain_change: Option<GainHandler>,
    muted: Option<bool>,
    on_muted_change: Option<MutedHandler>,
    auto_start: bool,
    devices: Vec<AudioDevice>,
    loading: bool,
    permission: AudioPermission,
    on_request_permission: Option<ActionHandler>,
    /// `Some` follows your capture. `None` is the demo: idle until started,
    /// then [`MicrophoneStatus::Active`].
    status: Option<MicrophoneStatus>,
    meter_source: Option<FrameSource<MeterFrame>>,
    visual_source: Option<FrameSource<VisualFrame>>,
    on_start: Option<ActionHandler>,
    style: StyleRefinement,
}

impl MicSetup {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            device_id: None,
            on_device_change: None,
            gain_db: None,
            on_gain_change: None,
            muted: None,
            on_muted_change: None,
            auto_start: false,
            devices: Vec::new(),
            loading: false,
            permission: AudioPermission::Granted,
            on_request_permission: None,
            status: None,
            meter_source: None,
            visual_source: None,
            on_start: None,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled selected device. `None` is no selection.
    pub fn device_id(mut self, id: Option<SharedString>) -> Self {
        self.device_id = Some(id);
        self
    }

    /// `onDeviceChange`. `None` is the cleared selection.
    pub fn on_device_change(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_device_change = Some(Rc::new(f));
        self
    }

    /// Controlled gain in dB. Default 0 when unset.
    pub fn gain_db(mut self, gain_db: f32) -> Self {
        self.gain_db = Some(gain_db);
        self
    }

    /// `onGainChange`.
    pub fn on_gain_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_gain_change = Some(Rc::new(f));
        self
    }

    /// Controlled mute. Default false when unset.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    /// `onMutedChange`.
    pub fn on_muted_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_muted_change = Some(Rc::new(f));
        self
    }

    /// Open on mount instead of waiting for "Turn on microphone". Default false.
    pub fn auto_start(mut self, auto_start: bool) -> Self {
        self.auto_start = auto_start;
        self
    }

    /// Microphones to list (`useAudioDevices().devices`). Default empty, so the
    /// trigger shows "Default microphone".
    pub fn devices(mut self, devices: impl IntoIterator<Item = AudioDevice>) -> Self {
        self.devices = devices.into_iter().collect();
        self
    }

    /// `useAudioDevices().isLoading`. Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// `useAudioDevices().permission`. `Unsupported` is shown as denied, as in
    /// the React block. Default [`AudioPermission::Granted`].
    pub fn permission(mut self, permission: AudioPermission) -> Self {
        self.permission = permission;
        self
    }

    /// `useAudioDevices().requestPermission`, shown while permission is `Prompt`.
    pub fn on_request_permission(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_permission = Some(Rc::new(f));
        self
    }

    /// Capture state. Unset, the block is idle until started and then active
    /// (the demo signal stands in for the microphone).
    pub fn status(mut self, status: MicrophoneStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Analyser meter (`useAudioAnalyser().meter`), already gain- and mute-shaped.
    pub fn meter_source(mut self, source: FrameSource<MeterFrame>) -> Self {
        self.meter_source = Some(source);
        self
    }

    /// Analyser visual (`useAudioAnalyser().visual`).
    pub fn visual_source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.visual_source = Some(source);
        self
    }

    /// "Turn on microphone" (and `autoStart`): open the device. Fires once,
    /// when capture goes from off to on — the same edge as `enabled` flipping
    /// true for `useMicrophone`.
    pub fn on_start(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_start = Some(Rc::new(f));
        self
    }
}

impl Styled for MicSetup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MicSetup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let auto_start = self.auto_start;
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, move |_, _| MicState {
            started: auto_start,
            announced_start: false,
            device_id: None,
            gain_db: 0.0,
            muted: false,
            checking: false,
            check_started: Instant::now(),
            peak_db: f32::NEG_INFINITY,
            result: None,
        });

        let announce = state.update(cx, |s, _| {
            if s.started && !s.announced_start {
                s.announced_start = true;
                true
            } else {
                false
            }
        });
        if announce && let Some(on_start) = self.on_start.clone() {
            window.defer(cx, move |window, cx| on_start(window, cx));
        }

        let (device_id, gain_db, muted) = {
            let s = state.read(cx);
            (
                match &self.device_id {
                    Some(id) => id.clone(),
                    None => s.device_id.clone(),
                },
                self.gain_db.unwrap_or(s.gain_db),
                self.muted.unwrap_or(s.muted),
            )
        };
        let started = state.read(cx).started;
        let demo_status = self.status.is_none();
        let status = self.status.unwrap_or(if started {
            MicrophoneStatus::Active
        } else {
            MicrophoneStatus::Idle
        });
        let active = status == MicrophoneStatus::Active;
        // Sources only while a stream would be open. With neither source nor a
        // controlled status, the speech demo stands in for the microphone and
        // follows the gain and mute (React's gain node in front of the analyser).
        let demo =
            (active && demo_status && self.meter_source.is_none() && self.visual_source.is_none())
                .then(|| {
                    use_demo_signal(
                        child_id(&self.id, "demo"),
                        DemoSignalOptions::kind(DemoSignalKind::Speech)
                            .with_history_size(PREVIEW_HISTORY)
                            .with_gain_db(if muted { f32::NEG_INFINITY } else { gain_db })
                            .with_playing(true),
                        window,
                        cx,
                    )
                });
        let meter = if active {
            self.meter_source
                .clone()
                .or_else(|| demo.as_ref().map(|signal| signal.meter.clone()))
        } else {
            None
        };
        let visual = if active {
            self.visual_source
                .clone()
                .or_else(|| demo.as_ref().map(|signal| signal.visual.clone()))
        } else {
            None
        };

        let (checking, result) = {
            let mut peak = state.read(cx).peak_db;
            let checking_now = state.read(cx).checking;
            let started_at = state.read(cx).check_started;
            if checking_now {
                if let Some(level) = meter.as_ref().and_then(loudest_peak) {
                    peak = peak.max(level);
                }
                let done = started_at.elapsed() >= CHECK_DURATION;
                state.update(cx, |s, _| {
                    s.peak_db = peak;
                    if done {
                        s.checking = false;
                        s.result = Some(judge(peak));
                    }
                });
                if !done {
                    window.request_animation_frame();
                }
            }
            let s = state.read(cx);
            (s.checking, s.result)
        };

        let permission = match self.permission {
            AudioPermission::Unsupported => AudioPermission::Denied,
            permission => permission,
        };
        let muted_foreground = cx.theme().muted_foreground;
        let id = self.id.clone();

        let state_device = state.clone();
        let on_device = self.on_device_change.clone();
        let state_gain = state.clone();
        let on_gain = self.on_gain_change.clone();
        let state_mute = state.clone();
        let on_muted = self.on_muted_change.clone();
        let state_turn_on = state.clone();
        let state_check = state.clone();

        let mut select = AudioDeviceSelect::new(child_id(&id, "device"), self.devices)
            .loading(self.loading)
            .permission(permission)
            .value(device_id)
            .on_value_change(move |next, window, cx| {
                state_device.update(cx, |s, cx| {
                    s.device_id = next.clone();
                    cx.notify();
                });
                if let Some(on_device) = on_device.as_ref() {
                    on_device(next, window, cx);
                }
            })
            .trigger(
                AudioDeviceSelectTrigger::new()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(muted_foreground)
                            .child(Icon::new(IconName::Microphone)),
                    )
                    .child(AudioDeviceSelectValue::new().placeholder("Default microphone")),
            )
            .content(AudioDeviceSelectContent::new());
        if let Some(on_request) = self.on_request_permission.clone() {
            select = select.on_request_permission(move |window, cx| on_request(window, cx));
        }

        let mut waveform = LiveWaveform::new(child_id(&id, "waveform"))
            .active(active && !muted)
            .bar_width(2.0)
            .mode(LiveWaveformMode::Scrolling)
            .h(px(40.0));
        if let Some(visual) = visual {
            waveform = waveform.source(visual);
        }
        let mut preview = AudioDeviceSelectPreview::new()
            .relative()
            .w_full()
            .child(waveform);
        if !active {
            preview = preview.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(child_id(&id, "turn-on"))
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Outline)
                            .on_click(move |_, _, cx| {
                                state_turn_on.update(cx, |s, cx| {
                                    if !s.started {
                                        s.started = true;
                                        cx.notify();
                                    }
                                });
                            })
                            .child("Turn on microphone"),
                    ),
            );
        }

        let mut meter_view = LevelMeter::new(child_id(&id, "meter")).aria_label("Microphone level");
        if let Some(meter) = meter.clone() {
            meter_view = meter_view.source(meter);
        }

        let mut card = Card::new()
            .child(
                CardHeader::new()
                    .child(CardTitle::new("Microphone").font_family(HEADING_FONT))
                    .child(CardDescription::new(
                        "Pick a microphone and check your level.",
                    )),
            )
            .child(
                CardContent::new().child(
                    FieldGroup::new()
                        .child(
                            Field::new()
                                .child(FieldLabel::new("Device"))
                                .child(select)
                                .child(preview),
                        )
                        .child(
                            Field::new()
                                .child(
                                    div()
                                        .flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .child(FieldLabel::new("Level"))
                                        .child({
                                            let mut readout = DbReadout::new(child_id(&id, "db"))
                                                .hold_ms(500.0)
                                                .text_xs_()
                                                .text_color(muted_foreground);
                                            if let Some(meter) = &meter {
                                                readout = readout.source(meter.clone());
                                            }
                                            readout
                                        }),
                                )
                                .child(
                                    meter_view
                                        .child(
                                            LevelMeterChannels::new()
                                                .child(
                                                    LevelMeterChannel::new().child(
                                                        LevelMeterTrack::new()
                                                            .child(LevelMeterBar::new())
                                                            .child(LevelMeterHold::new()),
                                                    ),
                                                )
                                                .child(LevelMeterScale::new()),
                                        )
                                        .child(LevelMeterClip::new()),
                                ),
                        )
                        .child(
                            ParameterSlider::new(child_id(&id, "gain"))
                                .min(-24.0)
                                .max(24.0)
                                .origin(0.0)
                                .reset_value(0.0)
                                .unit("dB")
                                .value(gain_db)
                                .on_value_change(move |next, window, cx| {
                                    state_gain.update(cx, |s, cx| {
                                        s.gain_db = next;
                                        cx.notify();
                                    });
                                    if let Some(on_gain) = on_gain.as_ref() {
                                        on_gain(next, window, cx);
                                    }
                                })
                                .child(
                                    ParameterSliderHeader::new()
                                        .child(ParameterSliderLabel::new("Gain"))
                                        .child(ParameterSliderReset::new())
                                        .child(ParameterSliderInput::new()),
                                )
                                .child(ParameterSliderControl::new()),
                        )
                        .child(
                            Field::new()
                                .orientation(FieldOrientation::Horizontal)
                                .child(
                                    Switch::new(child_id(&id, "mute"))
                                        .checked(muted)
                                        .on_checked_change(move |next, window, cx| {
                                            state_mute.update(cx, |s, cx| {
                                                s.muted = next;
                                                cx.notify();
                                            });
                                            if let Some(on_muted) = on_muted.as_ref() {
                                                on_muted(next, window, cx);
                                            }
                                        }),
                                )
                                .child(FieldLabel::new("Mute microphone")),
                        )
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .flex_col()
                                .gap(px(12.0))
                                .child(
                                    Button::new(child_id(&id, "check"))
                                        .variant(ButtonVariant::Outline)
                                        .self_start()
                                        .disabled(!active || checking)
                                        .on_click(move |_, _, cx| {
                                            state_check.update(cx, |s, cx| {
                                                s.result = None;
                                                s.checking = true;
                                                s.peak_db = f32::NEG_INFINITY;
                                                s.check_started = Instant::now();
                                                cx.notify();
                                            });
                                        })
                                        .child(if checking {
                                            "Listening… say a few words"
                                        } else {
                                            "Check level"
                                        }),
                                )
                                .when_some(result.map(verdict_of), |el, verdict| {
                                    el.child(
                                        Alert::new()
                                            .variant(if verdict.ok {
                                                AlertVariant::Default
                                            } else {
                                                AlertVariant::Destructive
                                            })
                                            .icon(if verdict.ok {
                                                IconName::CheckCircle
                                            } else {
                                                IconName::WarningCircle
                                            })
                                            .child(AlertTitle::new(verdict.title))
                                            .child(AlertDescription::new(verdict.description)),
                                    )
                                })
                                .when(status == MicrophoneStatus::Denied, |el| {
                                    el.child(
                                        Alert::new()
                                            .variant(AlertVariant::Destructive)
                                            .icon(IconName::WarningCircle)
                                            .child(AlertTitle::new("Microphone blocked"))
                                            .child(AlertDescription::new(
                                                "Allow microphone access in your browser's site settings.",
                                            )),
                                    )
                                }),
                        ),
                ),
            );
        card.style().refine(&self.style);
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_check_judges_the_loudest_peak() {
        assert_eq!(judge(f32::NEG_INFINITY), CheckResult::Silent);
        assert_eq!(judge(-60.0), CheckResult::Silent);
        assert_eq!(judge(-55.0), CheckResult::Quiet);
        assert_eq!(judge(-40.0), CheckResult::Quiet);
        assert_eq!(judge(-30.0), CheckResult::Good);
        assert_eq!(judge(-12.0), CheckResult::Good);
        assert_eq!(judge(-1.0), CheckResult::Loud);
        assert_eq!(judge(0.0), CheckResult::Loud);
    }
}
