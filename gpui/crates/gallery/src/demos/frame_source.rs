//! Gallery page for `frame-source`. One variant per docs example: `components/examples/frame-source-demo.tsx`.
//!
//! The "Microphone" row shows a hand-made frame source: `create_frame_emitter`
//! fed from a timer task (standing in for an audio thread or a socket), read
//! with `use_level` exactly like the demo signal row.

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use std::time::Duration;

use audiocn::audio::decibels::{FormatDbOptions, format_db};
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::frame_loop::now_ms;
use audiocn::audio::frame_source::FrameSource;
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::audio::frame_source::{FrameEmitter, create_frame_emitter};
use audiocn::audio::types::{MeterFrame, MeterZoneName};
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use audiocn::signal::demo_signal::DemoSignal;
use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::signal::level::{LevelOptions, LevelState, use_level};
use audiocn::theme::ActiveTheme;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
use gpui::Task;
use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "frame-source",
    title: "Frame Source",
    description: "Push frames from any engine: a socket, a worker, an audio thread or a test.",
    variants: &["default"],
    render,
};

/// How often the stand-in engine pushes a frame.
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => {
            #[cfg(all(feature = "device", not(target_family = "wasm")))]
            {
                demo_device(window, cx)
            }
            #[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
            {
                demo(window, cx)
            }
        }
        _ => return None,
    })
}

fn zone_name(zone: MeterZoneName) -> &'static str {
    match zone {
        MeterZoneName::Ok => "ok",
        MeterZoneName::Warn => "warn",
        MeterZoneName::Clip => "clip",
    }
}

fn db(value: f32) -> String {
    format_db(
        value,
        FormatDbOptions {
            floor_db: -90.0,
            ..Default::default()
        },
    )
}

/// `LevelRow`.
fn level_row(
    key: String,
    label: &'static str,
    source: FrameSource<MeterFrame>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let LevelState {
        peak_db,
        rms_db,
        zone,
    } = use_level(
        key,
        Some(source),
        LevelOptions {
            interval_ms: 100.0,
            ..Default::default()
        },
        window,
        cx,
    );
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .font_family(theme.font_mono.clone())
        .text_size(px(14.0))
        .line_height(px(20.0))
        .child(
            div()
                .w(px(128.0))
                .flex_none()
                .font_family(theme.font_sans.clone())
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(format!("peak {}", db(peak_db))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(format!("rms {}", db(rms_db.unwrap_or(f32::NEG_INFINITY)))),
        )
        .child(
            div()
                .w(px(64.0))
                .flex_none()
                .text_color(theme.muted_foreground)
                .child(zone_name(zone)),
        )
}

/// The microphone stand-in: an engine that pushes meter frames into an
/// emitter from a timer while "active".
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
struct Microphone {
    meter: FrameEmitter<MeterFrame>,
    task: Option<Task<()>>,
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
impl Microphone {
    fn status(&self) -> &'static str {
        if self.task.is_some() {
            "active"
        } else {
            "idle"
        }
    }

    fn start(&mut self, cx: &mut gpui::Context<Self>) {
        let emitter = self.meter.clone();
        let mut engine =
            DemoSignal::new(DemoSignalOptions::kind(DemoSignalKind::Speech).with_seed(5));
        let source = engine.meter_source();
        self.task = Some(cx.spawn(async move |_, cx| {
            loop {
                engine.tick(now_ms());
                if let Some(frame) = source.latest() {
                    emitter.emit(frame);
                }
                cx.background_executor().timer(FRAME_INTERVAL).await;
            }
        }));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut gpui::Context<Self>) {
        self.task = None;
        cx.notify();
    }
}

/// `frame-source-demo.tsx` with a real microphone.
#[cfg(all(feature = "device", not(target_family = "wasm")))]
fn demo_device(window: &mut Window, cx: &mut App) -> AnyElement {
    use audiocn::io::{MicrophoneOptions, MicrophoneStatus, use_microphone};

    let base = id("frame-source", "default");
    let demo = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    let mic = use_microphone(
        format!("{base}/mic"),
        MicrophoneOptions::default(),
        window,
        cx,
    );
    let listening = mic.status == MicrophoneStatus::Active;
    let status = match mic.status {
        MicrophoneStatus::Idle => "idle",
        MicrophoneStatus::Acquiring => "acquiring",
        MicrophoneStatus::Active => "active",
        MicrophoneStatus::Denied => "denied",
        MicrophoneStatus::Unavailable => "unavailable",
        MicrophoneStatus::Error => "error",
    };
    let status_text: SharedString = format!("Microphone: {status}").into();
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(576.0))
        .gap(px(16.0))
        .child(level_row(
            format!("{base}/demo-level"),
            "Demo signal",
            demo.meter,
            window,
            cx,
        ))
        .child(level_row(
            format!("{base}/mic-level"),
            "Microphone",
            mic.meter.clone(),
            window,
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.0))
                .child(
                    Button::new(format!("{base}/toggle"))
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .child(if listening {
                            "Stop microphone"
                        } else {
                            "Use my microphone"
                        })
                        .on_click(move |_, _, cx| {
                            if listening {
                                mic.stop(cx);
                            } else {
                                mic.start(cx);
                            }
                        }),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(status_text),
                ),
        )
        .into_any_element()
}

/// `frame-source-demo.tsx`. The microphone row is a demo-signal engine.
#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("frame-source", "default");
    let demo = use_demo_signal(
        format!("{base}/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    let microphone = window.use_keyed_state(format!("{base}/microphone"), cx, |_, _| Microphone {
        meter: create_frame_emitter(),
        task: None,
    });
    let (mic_meter, status) = {
        let mic = microphone.read(cx);
        (mic.meter.source(), mic.status())
    };
    let listening = status == "active";
    let status_text: SharedString = format!("Microphone: {status}").into();

    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(576.0))
        .gap(px(16.0))
        .child(level_row(
            format!("{base}/demo-level"),
            "Demo signal",
            demo.meter,
            window,
            cx,
        ))
        .child(level_row(
            format!("{base}/mic-level"),
            "Microphone",
            mic_meter,
            window,
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.0))
                .child(
                    Button::new(format!("{base}/toggle"))
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .child(if listening {
                            "Stop microphone"
                        } else {
                            "Use my microphone"
                        })
                        .on_click(move |_, _, cx| {
                            microphone.update(cx, |mic, cx| {
                                if mic.task.is_some() {
                                    mic.stop(cx);
                                } else {
                                    mic.start(cx);
                                }
                            });
                        }),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .child(status_text),
                ),
        )
        .into_any_element()
}
