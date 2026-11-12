//! Gallery page for `electric-waveform`. One variant per docs example: `components/examples/electric-waveform-demo.tsx`, `components/examples/electric-waveform-intensity.tsx`, `components/examples/electric-waveform-microphone.tsx`, `components/examples/electric-waveform-modes.tsx`, `components/examples/electric-waveform-states.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "electric-waveform",
    title: "Electric Waveform",
    description: "One electric line across the width, with a crackling white-hot core, a tall glow, forks off the peaks and sparks.",
    section: Section::Components,
    variants: &["default", "intensity", "microphone", "modes", "states"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("electric-waveform", "default", cx),
        "intensity" => pending("electric-waveform", "intensity", cx),
        "microphone" => pending("electric-waveform", "microphone", cx),
        "modes" => pending("electric-waveform", "modes", cx),
        "states" => pending("electric-waveform", "states", cx),
        _ => return None,
    })
}
