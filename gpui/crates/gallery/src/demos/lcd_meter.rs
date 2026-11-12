//! Gallery page for `lcd-meter`. One variant per docs example: `components/examples/lcd-meter-demo.tsx`, `components/examples/lcd-meter-inks.tsx`, `components/examples/lcd-meter-microphone.tsx`, `components/examples/lcd-meter-segments.tsx`.
//! (pending: replace `pending(...)` with the real port of each example.)

use gpui::{AnyElement, App, Window};

use super::{Demo, Section, pending};

pub const DEMO: Demo = Demo {
    slug: "lcd-meter",
    title: "LCD Meter",
    description: "A level meter on an LCD Display's screen, with block segments, a held peak, readouts, a printed scale and a CLIP flag.",
    section: Section::Components,
    variants: &["default", "inks", "microphone", "segments"],
    render,
};

fn render(variant: &str, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => pending("lcd-meter", "default", cx),
        "inks" => pending("lcd-meter", "inks", cx),
        "microphone" => pending("lcd-meter", "microphone", cx),
        "segments" => pending("lcd-meter", "segments", cx),
        _ => return None,
    })
}
