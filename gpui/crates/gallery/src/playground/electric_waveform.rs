//! Playground page for `ElectricWaveform`: mode, intensity, colour and paint/clear.

use audiocn::VisualFrame;
use audiocn::audio::frame_loop::now_ms;
use audiocn::components::{ElectricWaveform, ElectricWaveformActions, ElectricWaveformMode};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "electric-waveform",
    component: "ElectricWaveform",
    source: SourceUse::Visual,
    controls,
    render,
};

const MODE: &[(&str, &str)] = &[
    ("wave", "ElectricWaveformMode::Wave"),
    ("scope", "ElectricWaveformMode::Scope"),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("mode", "Mode", 0, MODE)
            .group("Shape")
            .help("wave follows the bands. scope draws the signal."),
        Control::bool("loading", "Loading", false)
            .group("Shape")
            .help("A pulse along the line."),
        Control::num("intensity", "Intensity", 0.6, 0.0, 1.0, 0.05).group("Shape"),
        Control::bool("arcs", "Arcs", true).group("Shape"),
        Control::bool("sparks", "Sparks", true).group("Shape"),
        Control::num("sensitivity", "Sensitivity", 1.0, 0.1, 8.0, 0.1).group("Shape"),
        Control::num("line_width", "Line width", 3.0, 0.5, 16.0, 0.5).group("Shape"),
        Control::bool("fade_edges", "Fade edges", true).group("Shape"),
        Control::color("text_color", "Text colour")
            .group("Colour")
            .help("Fallback line colour when Electric is default."),
        Control::color("electric", "Electric").group("Colour"),
        Control::color("electric_core", "Core").group("Colour"),
        Control::color("electric_glow", "Glow").group("Colour"),
        Control::bool("custom_heat", "Custom heat", false)
            .group("Colour")
            .no_code()
            .help("On calls electric_heat. Off keeps the theme (0 light, 0.7 dark)."),
        Control::num("electric_heat", "Heat", 0.7, 0.0, 1.0, 0.05)
            .group("Colour")
            .no_code()
            .help("Applied only while Custom heat is on."),
        Control::num("electric_glow_size", "Glow size", 8.0, 0.0, 40.0, 1.0).group("Colour"),
        Control::text("aria_label", "ARIA label", "Audio waveform").group("Accessibility"),
    ]
}

struct Handles {
    actions: ElectricWaveformActions,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let held = window.use_keyed_state(ctx.id("actions"), cx, |_, _| Handles {
        actions: ElectricWaveformActions::new(),
    });
    let actions = held.read(cx).actions.clone();

    let mut wave = ElectricWaveform::new(ctx.id("wave"))
        .source(ctx.visual())
        .actions(actions.clone())
        .mode(mode(p.choice("mode")))
        .loading(p.bool("loading"))
        .intensity(p.num("intensity"))
        .arcs(p.bool("arcs"))
        .sparks(p.bool("sparks"))
        .sensitivity(p.num("sensitivity"))
        .line_width(p.num("line_width"))
        .fade_edges(p.bool("fade_edges"))
        .electric_glow_size(p.num("electric_glow_size"))
        .aria_label(p.text("aria_label"))
        .w_full()
        .h(px(128.0));
    if let Some(color) = p.color("text_color", cx) {
        wave = wave.text_color(color);
    }
    if let Some(color) = p.color("electric", cx) {
        wave = wave.electric(color);
    }
    if let Some(color) = p.color("electric_core", cx) {
        wave = wave.electric_core(color);
    }
    if let Some(color) = p.color("electric_glow", cx) {
        wave = wave.electric_glow(color);
    }
    if p.bool("custom_heat") {
        wave = wave.electric_heat(p.num("electric_heat"));
    }

    let paint = actions.clone();
    let clear = actions.clone();
    let log_paint = ctx.clone();
    let log_clear = ctx.clone();
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(wave)
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(px(8.0))
                .child(action_button(ctx.id("paint"), "Paint", move |_, _, cx| {
                    paint.paint(sample_frame());
                    log_paint.log("actions.paint", "burst frame", cx);
                }))
                .child(action_button(ctx.id("clear"), "Clear", move |_, _, cx| {
                    clear.clear();
                    log_clear.log("actions.clear", "line falls flat", cx);
                })),
        )
        .into_any_element()
}

fn action_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .size(ButtonSize::Sm)
        .variant(ButtonVariant::Outline)
        .on_click(on_click)
        .child(label)
}

fn mode(choice: &str) -> ElectricWaveformMode {
    match choice {
        "scope" => ElectricWaveformMode::Scope,
        _ => ElectricWaveformMode::Wave,
    }
}

fn sample_frame() -> VisualFrame {
    let bands: Vec<f32> = (0..24)
        .map(|i| (i as f32 * 0.55).sin().abs() * 0.85 + 0.05)
        .collect();
    let time_domain: Vec<f32> = (0..256)
        .map(|i| (i as f32 * 0.17).sin() * 0.65 + (i as f32 * 0.05).sin() * 0.2)
        .collect();
    let history: Vec<f32> = (0..48).map(|i| (i as f32 * 0.35).sin().abs()).collect();
    let history_length = history.len();
    VisualFrame {
        bands,
        history,
        history_length,
        history_start: 0,
        history_updated_at: Some(now_ms()),
        history_interval_ms: Some(40.0),
        history_previous_level: Some(0.2),
        time_domain: Some(time_domain),
        peak_db: -9.0,
    }
}
