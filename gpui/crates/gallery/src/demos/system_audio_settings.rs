//! Gallery page for `system-audio-settings`. One variant per docs example: `components/examples/system-audio-settings-demo.tsx`.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::SystemAudioSettings;

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "system-audio-settings",
    title: "System Audio Settings",
    description: "Turn system audio capture on, set its level and see what the browser captures.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        // `system-audio-settings-demo.tsx`: `<SystemAudioSettings className="w-full max-w-md" />`.
        // Capture stays off until the switch; a stereo demo signal then stands
        // in for the shared screen.
        "default" => SystemAudioSettings::new(id("system-audio-settings", "default"))
            .w_full()
            .max_w(px(448.0))
            .into_any_element(),
        _ => return None,
    })
}
