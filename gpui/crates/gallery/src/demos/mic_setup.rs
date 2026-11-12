//! Gallery page for `mic-setup`. One variant per docs example: `components/examples/mic-setup-demo.tsx`.

use gpui::{AnyElement, App, IntoElement, Styled, Window, px};

use audiocn::MicSetup;

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "mic-setup",
    title: "Mic Setup",
    description: "Choose and check a microphone, with a live preview, a meter, gain, mute and a level check.",
    section: Section::Blocks,
    variants: &["default"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        // `mic-setup-demo.tsx`: `<MicSetup className="w-full max-w-md" />`.
        // No microphone here: the block's demo signal starts when the preview
        // button is pressed. The idle card is what the docs show.
        "default" => MicSetup::new(id("mic-setup", "default"))
            .w_full()
            .max_w(px(448.0))
            .into_any_element(),
        _ => return None,
    })
}
