//! Gallery page for `smooth-waveform`. One variant per docs example: `components/examples/smooth-waveform-demo.tsx`, `components/examples/smooth-waveform-microphone.tsx`, `components/examples/smooth-waveform-modes.tsx`, `components/examples/smooth-waveform-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "smooth-waveform",
    title: "Smooth Waveform",
    description: "One clean line across the width that follows the sound, as a flowing wave or an oscilloscope trace.",
    section: Section::Components,
    variants: &["default", "microphone", "modes", "states"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("smooth-waveform", "default", cx),
        "microphone" => pending("smooth-waveform", "microphone", cx),
        "modes" => pending("smooth-waveform", "modes", cx),
        "states" => pending("smooth-waveform", "states", cx),
        _ => return None,
    })
}
