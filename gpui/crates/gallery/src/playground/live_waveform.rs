//! Playground page for `LiveWaveform`: modes, variants, bar and line props, paint/clear.

use audiocn::VisualFrame;
use audiocn::audio::frame_loop::now_ms;
use audiocn::components::{
    LiveWaveform, LiveWaveformActions, LiveWaveformMode, LiveWaveformVariant,
};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "live-waveform",
    component: "LiveWaveform",
    source: SourceUse::Visual,
    controls,
    render,
};

const MODE: &[(&str, &str)] = &[
    ("static", "LiveWaveformMode::Static"),
    ("scrolling", "LiveWaveformMode::Scrolling"),
];

const VARIANT: &[(&str, &str)] = &[
    ("bars", "LiveWaveformVariant::Bars"),
    ("line", "LiveWaveformVariant::Line"),
    ("mirror", "LiveWaveformVariant::Mirror"),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("mode", "Mode", 0, MODE)
            .group("Shape")
            .help("scrolling shows level history. static shows the current frame."),
        Control::choice("variant", "Variant", 0, VARIANT).group("Shape"),
        Control::bool("active", "Active", true)
            .group("Shape")
            .help("Off draws an idle dotted line."),
        Control::num("sensitivity", "Sensitivity", 1.0, 0.1, 8.0, 0.1).group("Shape"),
        Control::num("bar_width", "Bar width", 3.0, 1.0, 24.0, 0.5).group("Bars"),
        Control::num("bar_gap", "Bar gap", 1.0, 0.0, 16.0, 0.5).group("Bars"),
        Control::num("bar_radius", "Bar radius", 1.5, 0.0, 16.0, 0.5).group("Bars"),
        Control::num("min_bar_height", "Min bar height", 4.0, 0.0, 48.0, 1.0).group("Bars"),
        Control::num("line_width", "Line width", 1.5, 0.5, 12.0, 0.5).group("Line"),
        Control::bool("fade_edges", "Fade edges", true).group("Fade"),
        Control::num("fade_width", "Fade width", 24.0, 0.0, 120.0, 1.0).group("Fade"),
        Control::color("text_color", "Colour")
            .group("Look")
            .help("Drawn in the text colour."),
    ]
}

struct Handles {
    actions: LiveWaveformActions,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let held = window.use_keyed_state(ctx.id("actions"), cx, |_, _| Handles {
        actions: LiveWaveformActions::new(),
    });
    let actions = held.read(cx).actions.clone();

    let mut wave = LiveWaveform::new(ctx.id("wave"))
        .actions(&actions)
        .mode(mode(p.choice("mode")))
        .variant(variant(p.choice("variant")))
        .active(p.bool("active"))
        .sensitivity(p.num("sensitivity"))
        .bar_width(p.num("bar_width"))
        .bar_gap(p.num("bar_gap"))
        .bar_radius(p.num("bar_radius"))
        .min_bar_height(p.num("min_bar_height"))
        .line_width(p.num("line_width"))
        .fade_edges(p.bool("fade_edges"))
        .fade_width(p.num("fade_width"))
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
                    log_clear.log("actions.clear", "canvas cleared", cx);
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

fn mode(choice: &str) -> LiveWaveformMode {
    match choice {
        "scrolling" => LiveWaveformMode::Scrolling,
        _ => LiveWaveformMode::Static,
    }
}

fn variant(choice: &str) -> LiveWaveformVariant {
    match choice {
        "line" => LiveWaveformVariant::Line,
        "mirror" => LiveWaveformVariant::Mirror,
        _ => LiveWaveformVariant::Bars,
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
