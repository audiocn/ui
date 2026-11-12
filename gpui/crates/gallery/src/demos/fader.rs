//! Gallery page for `fader`. One variant per docs example: `components/examples/fader-demo.tsx`, `components/examples/fader-bipolar.tsx`, `components/examples/fader-silence.tsx`, `components/examples/fader-sizes.tsx`, `components/examples/fader-vertical.tsx`, `components/examples/fader-with-meter.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "fader",
    title: "Fader",
    description: "A volume fader in decibels, with tapers, detents, a scale, reset and an editable value.",
    section: Section::Components,
    variants: &[
        "default",
        "bipolar",
        "silence",
        "sizes",
        "vertical",
        "with-meter",
    ],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("fader", "default", cx),
        "bipolar" => pending("fader", "bipolar", cx),
        "silence" => pending("fader", "silence", cx),
        "sizes" => pending("fader", "sizes", cx),
        "vertical" => pending("fader", "vertical", cx),
        "with-meter" => pending("fader", "with-meter", cx),
        _ => return None,
    })
}
