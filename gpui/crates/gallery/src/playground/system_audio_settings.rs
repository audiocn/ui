//! Playground page for `SystemAudioSettings`: capture status and support,
//! the meter seam, controlled and uncontrolled switch / level, and every
//! callback.

use audiocn::{SystemAudioSettings, SystemAudioStatus};
use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use super::mic_setup::{REAL_AUDIO, later, set_choice};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "system-audio-settings",
    component: "SystemAudioSettings",
    source: SourceUse::Parts,
    controls,
    render,
};

/// `SystemAudioStatus` choices; index 0 leaves the prop unset (the block's demo).
pub(super) const SYSTEM_STATUSES: &[(&str, &str)] = &[
    ("unset (block demo)", ""),
    ("idle", "SystemAudioStatus::Idle"),
    ("prompting", "SystemAudioStatus::Prompting"),
    ("active", "SystemAudioStatus::Active"),
    ("no audio", "SystemAudioStatus::NoAudio"),
    ("denied", "SystemAudioStatus::Denied"),
    ("ended", "SystemAudioStatus::Ended"),
    ("unsupported", "SystemAudioStatus::Unsupported"),
];
pub(super) const SYSTEM_IDLE: usize = 1;
pub(super) const SYSTEM_PROMPTING: usize = 2;
pub(super) const SYSTEM_ACTIVE: usize = 3;

pub(super) fn system_status(label: &str) -> Option<SystemAudioStatus> {
    Some(match label {
        "idle" => SystemAudioStatus::Idle,
        "prompting" => SystemAudioStatus::Prompting,
        "active" => SystemAudioStatus::Active,
        "no audio" => SystemAudioStatus::NoAudio,
        "denied" => SystemAudioStatus::Denied,
        "ended" => SystemAudioStatus::Ended,
        "unsupported" => SystemAudioStatus::Unsupported,
        _ => return None,
    })
}

fn controls() -> Vec<Control> {
    let mut controls = vec![
        // Audio seam
        Control::choice("status", "Status", 0, SYSTEM_STATUSES)
            .group("Audio seam")
            .help("Unset: the switch toggles off / on and a stereo music demo plays."),
        Control::bool("supported", "Supported", true)
            .group("Audio seam")
            .help("Off forces \"unsupported\": switch disabled, notice swapped."),
        Control::bool("use_source", "Use playground source", false)
            .group("Audio seam")
            .code(".meter_source(signal.meter.clone())")
            .help("meter_source from the Source panel, shown while the status is active."),
        Control::bool("simulate_host", "Simulate host", true)
            .group("Audio seam")
            .no_code()
            .help("With a status set, on_start goes prompting -> active, on_stop idle."),
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: the block keeps the switch and level (remounts)."),
        Control::bool("enabled", "Enabled", false).group("Value"),
        Control::num("gain_db", "Level (dB)", -6.0, -24.0, 12.0, 0.5).group("Value"),
        // Layout
        Control::px("max_width", "Max width (px)", 448.0, 280.0, 900.0, 8.0)
            .group("Layout")
            .method("max_w"),
    ];
    if REAL_AUDIO {
        controls.insert(
            0,
            Control::bool("real_audio", "Real audio", false)
                .group("Audio seam")
                .no_code()
                .remount()
                .help("Capture the default output's monitor (audiocn::io::system_audio)."),
        );
    }
    controls
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let mut block = SystemAudioSettings::new(ctx.id("system-audio-settings"));
    if !p.bool("supported") {
        block = block.supported(false);
    }
    let status = system_status(p.choice("status"));
    if let Some(status) = status {
        block = block.status(status);
    }
    if p.bool("use_source")
        && let Some(meter) = ctx.meter()
    {
        block = block.meter_source(meter);
    }
    if controlled {
        block = block.enabled(p.bool("enabled")).gain_db(p.num("gain_db"));
    }
    let simulate = p.bool("simulate_host") && status.is_some();
    let (c_enabled, c_gain, c_stream, c_start, c_stop) = (
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
        ctx.clone(),
    );
    block = block
        .on_enabled_change(move |on, _, cx| {
            if controlled {
                c_enabled.set_bool("enabled", on, cx);
            }
            c_enabled.log("on_enabled_change", on.to_string(), cx);
        })
        .on_gain_change(move |db, _, cx| {
            if controlled {
                c_gain.set_num("gain_db", db, cx);
            }
            c_gain.log("on_gain_change", format!("{db:.1} dB"), cx);
        })
        .on_stream_change(move |live, _, cx| {
            c_stream.log("on_stream_change", if live { "live" } else { "none" }, cx);
        })
        .on_start(move |_, cx| {
            c_start.log("on_start", "start capture", cx);
            if simulate {
                set_choice(&c_start, "status", SYSTEM_PROMPTING, cx);
                let c = c_start.clone();
                later(cx, 900, move |cx| {
                    set_choice(&c, "status", SYSTEM_ACTIVE, cx)
                });
            }
        })
        .on_stop(move |_, cx| {
            c_stop.log("on_stop", "stop capture", cx);
            if simulate {
                set_choice(&c_stop, "status", SYSTEM_IDLE, cx);
            }
        });
    if REAL_AUDIO && p.bool("real_audio") {
        block = real::wire(block, p, ctx, window, cx);
    }
    block
        .w_full()
        .max_w(px(p.num("max_width")))
        .into_any_element()
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod real {
    use audiocn::SystemAudioSettings;
    use gpui::{App, Window};

    use super::super::{Ctx, Props};

    pub fn wire(
        block: SystemAudioSettings,
        _p: &Props,
        _ctx: &Ctx,
        _window: &mut Window,
        _cx: &mut App,
    ) -> SystemAudioSettings {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod real {
    use audiocn::SystemAudioSettings;
    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{AnalyserChannels, AnalyserTapOptions, SystemAudioOptions, use_system_audio};
    use gpui::{App, Window};

    use super::super::{Ctx, Props};

    struct Seen {
        supported: bool,
        gain_db: f32,
    }

    /// `useSystemAudio()` + the level in front of a stereo analyser, like the
    /// gallery demo. Start / stop are logged as well as run.
    pub fn wire(
        block: SystemAudioSettings,
        p: &Props,
        ctx: &Ctx,
        window: &mut Window,
        cx: &mut App,
    ) -> SystemAudioSettings {
        let seen = window.use_keyed_state(ctx.id("real-seen"), cx, |_, _| Seen {
            supported: is_supported(),
            gain_db: -6.0,
        });
        let controlled = p.bool("controlled");
        let (supported, gain_db) = {
            let s = seen.read(cx);
            (
                s.supported,
                if controlled {
                    p.num("gain_db")
                } else {
                    s.gain_db
                },
            )
        };
        let system = use_system_audio(
            ctx.id("real-system"),
            SystemAudioOptions {
                analyser: AnalyserTapOptions {
                    channels: AnalyserChannels::Stereo,
                    ..Default::default()
                },
                ..Default::default()
            },
            window,
            cx,
        );
        if let Some(stream) = system.stream() {
            stream.set_gain_db(gain_db);
        }
        let (start, stop) = (system.clone(), system.clone());
        let (c_gain, c_start, c_stop) = (ctx.clone(), ctx.clone(), ctx.clone());
        block
            .supported(supported)
            .status(system.status().into())
            .meter_source(system.meter_source())
            .on_gain_change(move |db, _, cx| {
                if controlled {
                    c_gain.set_num("gain_db", db, cx);
                }
                c_gain.log("on_gain_change", format!("{db:.1} dB"), cx);
                seen.update(cx, |s, cx| {
                    s.gain_db = db;
                    cx.notify();
                });
            })
            .on_start(move |window, cx| {
                c_start.log("on_start", "real: capturing", cx);
                start.start();
                window.refresh();
            })
            .on_stop(move |window, cx| {
                c_stop.log("on_stop", "real: stopped", cx);
                stop.stop();
                window.refresh();
            })
    }
}
