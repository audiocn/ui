//! Playground page for `SmoothWaveform`: wave/scope, stroke and paint/clear.

use audiocn::VisualFrame;
use audiocn::audio::frame_loop::now_ms;
use audiocn::components::{SmoothWaveform, SmoothWaveformActions, SmoothWaveformMode};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "smooth-waveform",
    component: "SmoothWaveform",
    source: SourceUse::Visual,
    controls,
    render,
};

const MODE: &[(&str, &str)] = &[
    ("wave", "SmoothWaveformMode::Wave"),
    ("scope", "SmoothWaveformMode::Scope"),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("mode", "Mode", 0, MODE)
            .group("Shape")
            .help("wave follows the bands. scope draws the signal."),
        Control::bool("loading", "Loading", false)
            .group("Shape")
            .help("A pulse along the line."),
        Control::num("sensitivity", "Sensitivity", 1.0, 0.1, 8.0, 0.1).group("Shape"),
        Control::num("line_width", "Line width", 2.0, 0.5, 16.0, 0.5).group("Shape"),
        Control::bool("fade_edges", "Fade edges", true).group("Shape"),
        Control::color("text_color", "Colour")
            .group("Look")
            .help("The line is drawn in the text colour."),
    ]
}

struct Handles {
    actions: SmoothWaveformActions,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let held = window.use_keyed_state(ctx.id("actions"), cx, |_, _| Handles {
        actions: SmoothWaveformActions::new(),
    });
    let actions = held.read(cx).actions.clone();

    let mut wave = SmoothWaveform::new(ctx.id("wave"))
        .actions(&actions)
        .mode(mode(p.choice("mode")))
        .loading(p.bool("loading"))
        .sensitivity(p.num("sensitivity"))
        .line_width(p.num("line_width"))
        .fade_edges(p.bool("fade_edges"))
        .w_full()
        .h(px(128.0));
    if let Some(source) = ctx.visual() {
        wave = wave.source(source);
    }
    if let Some(color) = p.color("text_color", cx) {
        wave = wave.text_color(color);
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

fn mode(choice: &str) -> SmoothWaveformMode {
    match choice {
        "scope" => SmoothWaveformMode::Scope,
        _ => SmoothWaveformMode::Wave,
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
