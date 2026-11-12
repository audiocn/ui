//! Gallery page for `system-audio-settings`. One variant per docs example: `components/examples/system-audio-settings-demo.tsx`.
//!
//! With the `device` feature (native) the switch captures the system output
//! (`audiocn::io::system_audio`, the default output's monitor source) and the
//! level is applied before the stereo meter. Without it (the wasm build)
//! capture stays off until the switch, then a stereo demo signal stands in for
//! the shared screen.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::SystemAudioSettings;

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "system-audio-settings",
    title: "System Audio Settings",
    description: "Turn system audio capture on, set its level and see what the browser captures.",
    variants: &["default"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        // `system-audio-settings-demo.tsx`: `<SystemAudioSettings className="w-full max-w-md" />`.
        "default" => device::wire(
            SystemAudioSettings::new(id("system-audio-settings", "default")),
            window,
            cx,
        )
        .w_full()
        .max_w(px(448.0))
        .into_any_element(),
        _ => return None,
    })
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod device {
    use gpui::{App, Window};

    use audiocn::SystemAudioSettings;

    /// No capture: the block's demo signal.
    pub fn wire(
        block: SystemAudioSettings,
        _window: &mut Window,
        _cx: &mut App,
    ) -> SystemAudioSettings {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod device {
    use gpui::{App, Window};

    use audiocn::SystemAudioSettings;
    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{AnalyserChannels, AnalyserTapOptions, SystemAudioOptions, use_system_audio};

    use super::id;

    struct CaptureUi {
        /// `isSupported`, listed once (it enumerates the input devices).
        supported: bool,
        gain_db: f32,
    }

    /// `useSystemAudio()` + the gain node in front of
    /// `useAudioAnalyser(..., { channels: "stereo" })`.
    pub fn wire(
        block: SystemAudioSettings,
        window: &mut Window,
        cx: &mut App,
    ) -> SystemAudioSettings {
        let base = id("system-audio-settings", "default");
        let ui = window.use_keyed_state(format!("{base}/device-ui"), cx, |_, _| CaptureUi {
            supported: is_supported(),
            gain_db: -6.0,
        });
        let (supported, gain_db) = {
            let ui = ui.read(cx);
            (ui.supported, ui.gain_db)
        };
        let system = use_system_audio(
            format!("{base}/system"),
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
        block
            .supported(supported)
            .status(system.status().into())
            .meter_source(system.meter_source())
            .gain_db(gain_db)
            .on_gain_change(move |db, _, cx| {
                ui.update(cx, |ui, cx| {
                    ui.gain_db = db;
                    cx.notify();
                });
            })
            .on_start(move |window, _| {
                start.start();
                window.refresh();
            })
            .on_stop(move |window, _| {
                stop.stop();
                window.refresh();
            })
    }
}
