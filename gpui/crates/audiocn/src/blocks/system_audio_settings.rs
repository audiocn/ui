//! Port of `components/blocks/system-audio-settings/system-audio-settings.tsx`.
//!
//! # Audio seam
//!
//! The React block calls `useSystemAudio`, `useAudioContext`, `useGainNode` and
//! `useAudioAnalyser`, which open the browser's screen-share picker and hand
//! the parent a `MediaStream`. This one does not capture anything. Pass the
//! capture [`SystemAudioStatus`], whether capture is supported, and the meter
//! source (already run through your level control):
//!
//! ```ignore
//! SystemAudioSettings::new("system")
//!     .supported(true)
//!     .status(SystemAudioStatus::Active)
//!     .meter_source(meter)
//!     .on_enabled_change(|on, _, cx| set_capture(on, cx))
//!     .on_start(|_, cx| start_system_audio(cx))
//!     .on_stop(|_, cx| stop_system_audio(cx))
//!     .on_gain_change(|db, _, cx| set_level(db, cx))
//!     .on_stream_change(|live, _, cx| route_processed_stream(live, cx))
//! ```
//!
//! [`SystemAudioSettings::on_stream_change`] is React's `onStreamChange`:
//! `true` while a processed capture is live, `false` when it is not. There is
//! no `MediaStream` to hand over — apply the gain to the source you route.
//! Omit the status and the meter and the switch drives a stereo music
//! [`crate::signal::demo_signal::DemoSignal`] (the gallery has no system audio).

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce, StyleRefinement, Styled,
    Window, div, px,
};

use super::system_audio_mixer::SystemAudioStatus;
use crate::audio::{FrameSource, MeterFrame};
use crate::components::{
    LevelMeter, ParameterSlider, ParameterSliderControl, ParameterSliderHeader,
    ParameterSliderInput, ParameterSliderLabel, ParameterSliderReset,
};
use crate::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use crate::ui::icons::IconName;
use crate::ui::styles::TextSize;
use crate::ui::{
    Alert, AlertDescription, AlertTitle, AlertVariant, Badge, BadgeVariant, Card, CardContent,
    CardDescription, CardHeader, Field, FieldContent, FieldDescription, FieldGroup, FieldLabel,
    FieldOrientation, Switch,
};

/// Default level, and what reset restores. React `DEFAULT_GAIN_DB`.
const DEFAULT_GAIN_DB: f32 = -6.0;
/// React `--font-heading` (Outfit). Bundled with the gallery.
const HEADING_FONT: &str = "Outfit";

type BoolHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
type GainHandler = Rc<dyn Fn(f32, &mut Window, &mut App) + 'static>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// The badge next to "Capture system audio" (`STATE_LABELS`).
fn state_label(status: SystemAudioStatus) -> &'static str {
    match status {
        SystemAudioStatus::Active => "On",
        SystemAudioStatus::Denied => "Needs permission",
        SystemAudioStatus::Ended => "Ended",
        SystemAudioStatus::Idle => "Off",
        SystemAudioStatus::NoAudio => "No audio shared",
        SystemAudioStatus::Prompting => "Choose what to share…",
        SystemAudioStatus::Unsupported => "Unsupported",
    }
}

fn child_id(id: &ElementId, suffix: &str) -> ElementId {
    ElementId::Name(format!("{id}-{suffix}").into())
}

struct SystemState {
    enabled: bool,
    gain_db: f32,
    /// Status used when [`SystemAudioSettings::status`] is unset.
    status: SystemAudioStatus,
    /// Last `enabled` prop applied by the React effect. `None` means the prop
    /// itself was `None` (uncontrolled) the last time we looked.
    applied_enabled: Option<Option<bool>>,
    /// Last value delivered to `on_stream_change`.
    emitted_stream: Option<bool>,
}

/// Turn system-audio capture on, set its level, and see what is captured.
///
/// [`.enabled`](Self::enabled) and [`.gain_db`](Self::gain_db) are controlled
/// when set and internal otherwise, matching the React props. The level
/// defaults to −6 dB.
///
/// ```ignore
/// SystemAudioSettings::new("system").max_w(px(448.))
/// ```
#[derive(IntoElement)]
pub struct SystemAudioSettings {
    id: ElementId,
    enabled: Option<bool>,
    on_enabled_change: Option<BoolHandler>,
    gain_db: Option<f32>,
    on_gain_change: Option<GainHandler>,
    on_stream_change: Option<BoolHandler>,
    /// `Some` follows your capture. `None` is the demo: the switch toggles
    /// [`SystemAudioStatus::Idle`] and [`SystemAudioStatus::Active`].
    status: Option<SystemAudioStatus>,
    /// `useSystemAudio().isSupported`. Default true. `false` forces the
    /// unsupported status, disables the switch and swaps the notice.
    supported: Option<bool>,
    meter_source: Option<FrameSource<MeterFrame>>,
    on_start: Option<ActionHandler>,
    on_stop: Option<ActionHandler>,
    style: StyleRefinement,
}

impl SystemAudioSettings {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            enabled: None,
            on_enabled_change: None,
            gain_db: None,
            on_gain_change: None,
            on_stream_change: None,
            status: None,
            supported: None,
            meter_source: None,
            on_start: None,
            on_stop: None,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled capture switch. Unset, the switch keeps its own state.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// `onEnabledChange`.
    pub fn on_enabled_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_enabled_change = Some(Rc::new(f));
        self
    }

    /// Controlled level in dB. Default −6 when unset.
    pub fn gain_db(mut self, gain_db: f32) -> Self {
        self.gain_db = Some(gain_db);
        self
    }

    /// `onGainChange`.
    pub fn on_gain_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_gain_change = Some(Rc::new(f));
        self
    }

    /// React `onStreamChange`. `true` when the processed capture is live,
    /// `false` when it is not. A new callback does not repeat the same value.
    /// Apply [`gain_db`](Self::gain_db) on the source you route; this block
    /// has no `MediaStream`.
    pub fn on_stream_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_stream_change = Some(Rc::new(f));
        self
    }

    /// Capture state from your system-audio backend.
    pub fn status(mut self, status: SystemAudioStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Whether this host can capture system audio. Default true.
    pub fn supported(mut self, supported: bool) -> Self {
        self.supported = Some(supported);
        self
    }

    /// Stereo meter of the captured audio after the level control.
    pub fn meter_source(mut self, source: FrameSource<MeterFrame>) -> Self {
        self.meter_source = Some(source);
        self
    }

    /// Start capture. The switch calls this when it turns on and the `enabled`
    /// prop will not change on its own (uncontrolled, or already equal). The
    /// `enabled` prop also calls it, like the React effect, when it becomes
    /// true and capture is not already running.
    pub fn on_start(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_start = Some(Rc::new(f));
        self
    }

    /// Stop capture. Paired with [`Self::on_start`].
    pub fn on_stop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(f));
        self
    }
}

impl Styled for SystemAudioSettings {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn capturing(status: SystemAudioStatus) -> bool {
    matches!(
        status,
        SystemAudioStatus::Active | SystemAudioStatus::Prompting
    )
}

impl RenderOnce for SystemAudioSettings {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| SystemState {
            enabled: false,
            gain_db: DEFAULT_GAIN_DB,
            status: SystemAudioStatus::Idle,
            applied_enabled: None,
            emitted_stream: None,
        });

        let enabled_prop = self.enabled;
        let demo_mode = self.status.is_none();
        let host_status = self.status;
        let (run_start, run_stop) = state.update(cx, |s, _| {
            if s.applied_enabled == Some(enabled_prop) {
                return (false, false);
            }
            s.applied_enabled = Some(enabled_prop);
            let status = host_status.unwrap_or(s.status);
            match enabled_prop {
                Some(true) if !capturing(status) => {
                    if demo_mode {
                        s.enabled = true;
                        s.status = SystemAudioStatus::Active;
                    }
                    (true, false)
                }
                Some(false) => {
                    if demo_mode {
                        s.enabled = false;
                        s.status = SystemAudioStatus::Idle;
                    }
                    (false, true)
                }
                _ => (false, false),
            }
        });
        if run_start && let Some(on_start) = self.on_start.clone() {
            window.defer(cx, move |window, cx| on_start(window, cx));
        }
        if run_stop && let Some(on_stop) = self.on_stop.clone() {
            window.defer(cx, move |window, cx| on_stop(window, cx));
        }

        let supported = self.supported.unwrap_or(true);
        let gain_db = self.gain_db.unwrap_or_else(|| state.read(cx).gain_db);
        let raw_status = self.status.unwrap_or(state.read(cx).status);
        // `useSystemAudio` reports `unsupported` whenever the host cannot capture.
        let status = if supported {
            raw_status
        } else {
            SystemAudioStatus::Unsupported
        };
        let active = status == SystemAudioStatus::Active;
        let prompting = status == SystemAudioStatus::Prompting;

        let demo = (active && demo_mode && self.meter_source.is_none()).then(|| {
            use_demo_signal(
                child_id(&self.id, "demo"),
                DemoSignalOptions::kind(DemoSignalKind::Music)
                    .with_channels(2)
                    .with_gain_db(gain_db)
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

        // React emits the processed stream (or null) when that identity changes,
        // including once on mount.
        if self.on_stream_change.is_some() {
            let live = active;
            let changed = state.update(cx, |s, _| {
                let changed = s.emitted_stream != Some(live);
                if changed {
                    s.emitted_stream = Some(live);
                }
                changed
            });
            if changed && let Some(on_stream) = self.on_stream_change.clone() {
                window.defer(cx, move |window, cx| on_stream(live, window, cx));
            }
        }

        let id = self.id.clone();
        let state_switch = state.clone();
        let on_enabled = self.on_enabled_change.clone();
        let on_start = self.on_start.clone();
        let on_stop = self.on_stop.clone();
        let state_gain = state.clone();
        let on_gain = self.on_gain_change.clone();

        let level = ParameterSlider::new(child_id(&id, "level"))
            .disabled(!active)
            .min(-24.0)
            .max(12.0)
            .origin(0.0)
            .reset_value(DEFAULT_GAIN_DB)
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
                    .child(ParameterSliderLabel::new("Level"))
                    .child(ParameterSliderReset::new())
                    .child(ParameterSliderInput::new()),
            )
            .child(ParameterSliderControl::new());

        let mut group = FieldGroup::new().child(
            Field::new()
                .orientation(FieldOrientation::Horizontal)
                .items_start()
                .child(
                    FieldContent::new().min_w_0().child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.0))
                            .child(FieldLabel::new("Capture system audio"))
                            .child(
                                Badge::new()
                                    .variant(if active {
                                        BadgeVariant::Default
                                    } else {
                                        BadgeVariant::Secondary
                                    })
                                    .child(state_label(status)),
                            ),
                    ).child(FieldDescription::new(
                        "Your browser asks what to share. Choose a screen or tab and turn on its audio.",
                    )),
                )
                .child(
                    Switch::new(child_id(&id, "enabled"))
                        .checked(active || prompting)
                        .disabled(!supported)
                        .flex_shrink_0()
                        .on_checked_change(move |next, window, cx| {
                            if let Some(on_enabled) = on_enabled.as_ref() {
                                on_enabled(next, window, cx);
                            }
                            // Controlled: the effect above acts once the parent
                            // passes the new value. When the prop already says
                            // so (capture ended while `enabled` stayed true),
                            // nothing else will change, so act here.
                            let prop_will_change = enabled_prop.is_some() && enabled_prop != Some(next);
                            if prop_will_change {
                                return;
                            }
                            state_switch.update(cx, |s, cx| {
                                s.enabled = next;
                                if demo_mode {
                                    s.status = if next {
                                        SystemAudioStatus::Active
                                    } else {
                                        SystemAudioStatus::Idle
                                    };
                                }
                                cx.notify();
                            });
                            if next {
                                if let Some(on_start) = on_start.as_ref() {
                                    on_start(window, cx);
                                }
                            } else if let Some(on_stop) = on_stop.as_ref() {
                                on_stop(window, cx);
                            }
                        }),
                ),
        );
        group.extend([level.into_any_element()]);
        if active {
            let mut meter_view = LevelMeter::new(child_id(&id, "meter"))
                .channel_count(2)
                .aria_label("System audio level");
            if let Some(meter) = meter {
                meter_view = meter_view.source(meter);
            }
            group.extend([meter_view.into_any_element()]);
        }
        if supported {
            group.extend([Alert::new()
                .icon(IconName::Info)
                .child(AlertTitle::new("What gets captured"))
                .child(AlertDescription::new(
                    "Everything the shared screen, window or tab plays. Some browsers only offer tab audio.",
                ))
                .into_any_element()]);
        } else {
            group.extend([Alert::new()
                .variant(AlertVariant::Destructive)
                .icon(IconName::Warning)
                .child(AlertTitle::new("Not available in this browser"))
                .child(AlertDescription::new(
                    "This browser cannot capture system audio. Try a Chromium-based browser.",
                ))
                .into_any_element()]);
        }
        if status == SystemAudioStatus::Denied {
            group.extend([Alert::new()
                .variant(AlertVariant::Destructive)
                .icon(IconName::Warning)
                .child(AlertTitle::new("Capture was not started"))
                .child(AlertDescription::new(
                    "No screen or tab was shared. Turn capture on again and approve the browser's sharing request. If access is blocked, allow screen sharing in your browser or system settings.",
                ))
                .into_any_element()]);
        }
        if status == SystemAudioStatus::NoAudio {
            group.extend([Alert::new()
                .variant(AlertVariant::Destructive)
                .icon(IconName::Warning)
                .child(AlertTitle::new("No audio was shared"))
                .child(AlertDescription::new(
                    "Turn capture on again and tick the option to share audio.",
                ))
                .into_any_element()]);
        }

        let mut card = Card::new()
            .child(
                CardHeader::new()
                    .child(
                        // `CardTitle` seeds its label as a child, so the icon
                        // row uses the same type styles (`font-heading text-base
                        // font-medium`) around the desktop icon.
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_base_()
                            .font_family(HEADING_FONT)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(IconName::Desktop)
                            .child("System audio"),
                    )
                    .child(CardDescription::new(
                        "Add the sound from your computer to your recording or stream.",
                    )),
            )
            .child(CardContent::new().child(group));
        card.style().refine(&self.style);
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_badge_uses_the_react_labels() {
        assert_eq!(state_label(SystemAudioStatus::Idle), "Off");
        assert_eq!(state_label(SystemAudioStatus::Active), "On");
        assert_eq!(
            state_label(SystemAudioStatus::Prompting),
            "Choose what to share…"
        );
        assert_eq!(state_label(SystemAudioStatus::NoAudio), "No audio shared");
        assert_eq!(state_label(SystemAudioStatus::Ended), "Ended");
        assert_eq!(state_label(SystemAudioStatus::Denied), "Needs permission");
        assert_eq!(state_label(SystemAudioStatus::Unsupported), "Unsupported");
    }
}
