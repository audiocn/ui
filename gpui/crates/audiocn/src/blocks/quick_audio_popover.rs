//! Port of `components/blocks/quick-audio-popover/quick-audio-popover.tsx`.
//!
//! # Audio seam
//!
//! React's popover calls `useAudioDevices`, `useMicrophone`, `useAudioAnalyser`
//! and `useSystemAudio`, which open real devices. This one does not. Pass the
//! device list, and drive the microphone and system-audio switches through
//! callbacks:
//!
//! ```ignore
//! QuickAudioPopover::new("audio")
//!     .devices(devices)
//!     .device_id(selected.clone())
//!     .microphone_active(live)
//!     .source(analyser_visual)
//!     .system_supported(true)
//!     .system_active(sharing)
//!     .on_microphone(|on, _, cx| if on { open_mic(cx) } else { close_mic(cx) })
//!     .on_system_audio(|on, _, cx| if on { start_loopback(cx) } else { stop_loopback(cx) })
//!     .on_device_change(|id, _, cx| select_input(id, cx))
//! ```
//!
//! Leave the microphone uncontrolled and pass no [`QuickAudioPopover::source`]
//! and "Turn on" feeds the mini bar visualizer with a speech
//! [`crate::signal::demo_signal::DemoSignal`] (the gallery has no microphone).
//! The level is shown only while the microphone is on and not muted, matching
//! `source={live ? analyser.visual : null}`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, px,
};

use crate::audio::FrameSource;
use crate::audio::types::VisualFrame;
use crate::components::{AudioDevice, AudioDeviceSelect, AudioPermission, BarVisualizer};
use crate::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use crate::theme::ActiveTheme;
use crate::ui::icons::IconName;
use crate::ui::{
    Align, Button, ButtonSize, ButtonVariant, Field, FieldGroup, FieldLabel, FieldOrientation,
    Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, Separator, Side,
    Switch,
};

type DeviceHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;
type BoolHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

fn child_id(id: &ElementId, suffix: &str) -> ElementId {
    ElementId::Name(format!("{id}/{suffix}").into())
}

/// Devices shown when the host does not pass its own list: a stand-in for
/// `useAudioDevices` after permission is granted.
pub fn demo_microphones() -> Vec<AudioDevice> {
    vec![
        AudioDevice::new("default", "MacBook Pro Microphone").with_default(true),
        AudioDevice::new("usb", "Shure MV7+").with_description("USB"),
    ]
}

struct PopoverState {
    device_id: Option<SharedString>,
    microphone: bool,
    muted: bool,
    system: bool,
}

/// A toolbar button that shows the microphone state and a live level, and
/// opens microphone and system-audio controls.
///
/// ```ignore
/// QuickAudioPopover::new("quick-audio")
///     .side(Side::Bottom)
///     .align(Align::Center)
///     .child(Button::new("more").child("More audio settings"))
/// ```
#[derive(IntoElement)]
pub struct QuickAudioPopover {
    id: ElementId,
    side: Side,
    align: Align,
    devices: Option<Vec<AudioDevice>>,
    /// Controlled device id. `None` as the inner value is no selection.
    device_id: Option<Option<SharedString>>,
    on_device_change: Option<DeviceHandler>,
    loading: bool,
    permission: AudioPermission,
    on_request_permission: Option<ActionHandler>,
    /// Controlled microphone. `None` uses the internal on/off toggle.
    microphone_active: Option<bool>,
    on_microphone: Option<BoolHandler>,
    muted: Option<bool>,
    on_muted_change: Option<BoolHandler>,
    /// Analyser visual. Used only while the microphone is on and not muted.
    /// `None` with an uncontrolled microphone uses the speech demo signal.
    source: Option<FrameSource<VisualFrame>>,
    system_supported: bool,
    system_active: Option<bool>,
    default_system_active: bool,
    on_system_audio: Option<BoolHandler>,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<BoolHandler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl QuickAudioPopover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            side: Side::Bottom,
            align: Align::Center,
            devices: None,
            device_id: None,
            on_device_change: None,
            loading: false,
            permission: AudioPermission::Granted,
            on_request_permission: None,
            microphone_active: None,
            on_microphone: None,
            muted: None,
            on_muted_change: None,
            source: None,
            system_supported: true,
            system_active: None,
            default_system_active: false,
            on_system_audio: None,
            open: None,
            default_open: false,
            on_open_change: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Which side of the trigger the panel sits on. Default `Bottom`.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Alignment along that edge. Default `Center`.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Input devices (`useAudioDevices().devices`). Default: [`demo_microphones`].
    pub fn devices(mut self, devices: Vec<AudioDevice>) -> Self {
        self.devices = Some(devices);
        self
    }

    /// Controlled selected device. `None` is no selection.
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

    /// "Finding devices…". Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Microphone permission. Default `Granted` (the trigger reads "Select a device").
    pub fn permission(mut self, permission: AudioPermission) -> Self {
        self.permission = permission;
        self
    }

    pub fn on_request_permission(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_permission = Some(Rc::new(f));
        self
    }

    /// Controlled microphone (`useMicrophone` status `active`).
    pub fn microphone_active(mut self, active: bool) -> Self {
        self.microphone_active = Some(active);
        self
    }

    /// Turn on (`true`) or off (`false`).
    pub fn on_microphone(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_microphone = Some(Rc::new(f));
        self
    }

    /// Controlled mute.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = Some(muted);
        self
    }

    pub fn on_muted_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_muted_change = Some(Rc::new(f));
        self
    }

    /// Live bar levels (`useAudioAnalyser().visual`). Shown only while live.
    pub fn source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.source = Some(source);
        self
    }

    /// `useSystemAudio().isSupported`. Default true, so the switch is enabled
    /// the way the docs screenshot shows it. Pass `false` when display capture
    /// is unavailable.
    pub fn system_supported(mut self, supported: bool) -> Self {
        self.system_supported = supported;
        self
    }

    /// Controlled system-audio capture (`status == "active"`).
    pub fn system_active(mut self, active: bool) -> Self {
        self.system_active = Some(active);
        self
    }

    /// Uncontrolled seed for the system-audio switch. Default false.
    pub fn default_system_active(mut self, active: bool) -> Self {
        self.default_system_active = active;
        self
    }

    /// System audio switched on (`true`) or off.
    pub fn on_system_audio(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_system_audio = Some(Rc::new(f));
        self
    }

    /// Controlled popover open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Uncontrolled seed. Default false (closed), like the React popover.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    pub fn on_open_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(f));
        self
    }
}

impl ParentElement for QuickAudioPopover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for QuickAudioPopover {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for QuickAudioPopover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default_system = self.default_system_active;
        let state =
            window.use_keyed_state(child_id(&self.id, "state"), cx, move |_, _| PopoverState {
                device_id: None,
                microphone: false,
                muted: false,
                system: default_system,
            });
        let (device_id, microphone, muted, system) = {
            let stored = state.read(cx);
            (
                match &self.device_id {
                    Some(id) => id.clone(),
                    None => stored.device_id.clone(),
                },
                self.microphone_active.unwrap_or(stored.microphone),
                self.muted.unwrap_or(stored.muted),
                self.system_active.unwrap_or(stored.system),
            )
        };
        let live = microphone && !muted;
        let demo_mic = self.microphone_active.is_none() && self.source.is_none();

        let mut bars = BarVisualizer::new(child_id(&self.id, "bars"))
            .aria_hidden(true)
            .bar_count(5)
            .min_level(0.15)
            .bar_gap(px(2.0))
            .bar_width(px(3.0))
            .h(px(16.0))
            .w(px(32.0))
            .flex_none()
            .text_color(cx.theme().foreground);
        if live {
            if let Some(source) = self.source.clone() {
                bars = bars.source(source);
            } else if demo_mic {
                let signal = use_demo_signal(
                    child_id(&self.id, "signal"),
                    DemoSignalOptions::kind(DemoSignalKind::Speech),
                    window,
                    cx,
                );
                bars = bars.source(signal.visual);
            }
        }

        let label: SharedString = if live {
            "Audio: microphone live".into()
        } else {
            "Audio settings".into()
        };
        let icon = if live {
            IconName::Microphone
        } else {
            IconName::MicrophoneSlash
        };

        let devices = self.devices.clone().unwrap_or_else(demo_microphones);
        let (state_device, state_mic, state_mute, state_system) =
            (state.clone(), state.clone(), state.clone(), state.clone());
        let device_controlled = self.device_id.is_some();
        let mic_controlled = self.microphone_active.is_some();
        let mute_controlled = self.muted.is_some();
        let system_controlled = self.system_active.is_some();
        let on_device = self.on_device_change.clone();
        let on_mic = self.on_microphone.clone();
        let on_mute = self.on_muted_change.clone();
        let on_system = self.on_system_audio.clone();

        let mut select = AudioDeviceSelect::new(child_id(&self.id, "device"), devices)
            .loading(self.loading)
            .permission(self.permission)
            .value(device_id)
            .on_value_change(move |id, window, cx| {
                if !device_controlled {
                    state_device.update(cx, |s, cx| {
                        s.device_id = id.clone();
                        cx.notify();
                    });
                }
                if let Some(handler) = &on_device {
                    handler(id, window, cx);
                }
            });
        if let Some(handler) = self.on_request_permission.clone() {
            select = select.on_request_permission(move |window, cx| handler(window, cx));
        }

        let mut popover = Popover::new(self.id.clone())
            .default_open(self.default_open)
            .trigger(
                Button::new(child_id(&self.id, "trigger"))
                    .variant(ButtonVariant::Outline)
                    .aria_label(label)
                    .icon_start(icon)
                    .child(bars),
            )
            .content(
                PopoverContent::new()
                    .side(self.side)
                    .align(self.align)
                    .w(px(320.0))
                    .child(
                        PopoverHeader::new()
                            .child(PopoverTitle::new("Audio"))
                            .child(PopoverDescription::new("Microphone and system audio.")),
                    )
                    .child(
                        FieldGroup::new()
                            .child(
                                Field::new()
                                    .child(FieldLabel::new("Microphone"))
                                    .child(select),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .w_full()
                                    .child(
                                        Button::new(child_id(&self.id, "power"))
                                            .flex_1()
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Outline)
                                            .on_click(move |_, window, cx| {
                                                let next = !microphone;
                                                if !mic_controlled {
                                                    state_mic.update(cx, |s, cx| {
                                                        s.microphone = next;
                                                        cx.notify();
                                                    });
                                                }
                                                if let Some(handler) = &on_mic {
                                                    handler(next, window, cx);
                                                }
                                            })
                                            .child(if microphone { "Turn off" } else { "Turn on" }),
                                    )
                                    .child(
                                        Button::new(child_id(&self.id, "mute"))
                                            .flex_1()
                                            .size(ButtonSize::Sm)
                                            .variant(if muted {
                                                ButtonVariant::Destructive
                                            } else {
                                                ButtonVariant::Outline
                                            })
                                            .disabled(!microphone)
                                            .on_click(move |_, window, cx| {
                                                let next = !muted;
                                                if !mute_controlled {
                                                    state_mute.update(cx, |s, cx| {
                                                        s.muted = next;
                                                        cx.notify();
                                                    });
                                                }
                                                if let Some(handler) = &on_mute {
                                                    handler(next, window, cx);
                                                }
                                            })
                                            .child(if muted { "Unmute" } else { "Mute" }),
                                    ),
                            )
                            .child(Separator::new())
                            .child(
                                Field::new()
                                    .orientation(FieldOrientation::Horizontal)
                                    .child(FieldLabel::new("System audio").flex_1())
                                    .child(
                                        Switch::new(child_id(&self.id, "system"))
                                            .checked(system)
                                            .disabled(!self.system_supported)
                                            .on_checked_change(move |on, window, cx| {
                                                if !system_controlled {
                                                    state_system.update(cx, |s, cx| {
                                                        s.system = on;
                                                        cx.notify();
                                                    });
                                                }
                                                if let Some(handler) = &on_system {
                                                    handler(on, window, cx);
                                                }
                                            }),
                                    ),
                            )
                            .children(self.children),
                    ),
            );
        if let Some(open) = self.open {
            popover = popover.open(open);
        }
        if let Some(handler) = self.on_open_change {
            popover = popover.on_open_change(move |open, window, cx| handler(open, window, cx));
        }
        let mut root = div().child(popover);
        root.style().refine(&self.style);
        root
    }
}
