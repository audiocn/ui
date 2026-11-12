//! Playground page for `BarVisualizer`: every builder, static levels, and the paint handle.

use audiocn::audio::bar_levels::BarIdle;
use audiocn::components::{BarVisualizer, BarVisualizerActions, BarVisualizerAlign};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{AudioConfig, Orientation};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "bar-visualizer",
    component: "BarVisualizer",
    source: SourceUse::Visual,
    controls,
    render,
};

const ALIGN: &[(&str, &str)] = &[
    ("center", "BarVisualizerAlign::Center"),
    ("start", "BarVisualizerAlign::Start"),
    ("end", "BarVisualizerAlign::End"),
];

const IDLE: &[(&str, &str)] = &[
    ("static", "BarIdle::Static"),
    ("pulse", "BarIdle::Pulse"),
    ("wave", "BarIdle::Wave"),
];

const ORIENTATION: &[(&str, &str)] = &[
    ("from config", ""),
    ("horizontal", "Orientation::Horizontal"),
    ("vertical", "Orientation::Vertical"),
];

const DATA: &[(&str, &str)] = &[
    ("source", ""),
    ("voice", ""),
    ("bass", ""),
    ("flat", ""),
    ("none", ""),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("data", "Levels", 0, DATA)
            .group("Data")
            .no_code()
            .help("source follows the signal picker. Presets call levels(). none is idle."),
        Control::num("bar_count", "Bar count", 24.0, 1.0, 64.0, 1.0)
            .group("Bars")
            .code("bar_count({} as usize)"),
        Control::choice("align", "Align", 0, ALIGN).group("Bars"),
        Control::bool("mirrored", "Mirrored", false).group("Bars"),
        Control::num("min_level", "Min level", 0.08, 0.0, 1.0, 0.01).group("Bars"),
        Control::choice("idle", "Idle", 0, IDLE)
            .group("Motion")
            .help("What the bars do with no signal."),
        Control::bool("loading", "Loading", false)
            .group("Motion")
            .help("Sweep, for connecting or thinking."),
        Control::choice("orientation", "Orientation", 0, ORIENTATION)
            .group("Layout")
            .help("from config uses the toolbar. Otherwise it overrides AudioConfig."),
        Control::px("bar_width", "Bar width", 6.0, 1.0, 32.0, 1.0).group("Layout"),
        Control::px("bar_gap", "Bar gap", 3.0, 0.0, 24.0, 1.0).group("Layout"),
        Control::px("bar_radius", "Bar radius", 9999.0, 0.0, 9999.0, 1.0)
            .group("Layout")
            .help("9999 is fully rounded. Type a small value for squarer bars."),
        Control::color("text_color", "Colour").group("Look"),
        Control::text("aria_label", "ARIA label", "Audio visualizer").group("Accessibility"),
        Control::bool("aria_hidden", "ARIA hidden", false).group("Accessibility"),
    ]
}

struct Handles {
    actions: BarVisualizerActions,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let held = window.use_keyed_state(ctx.id("actions"), cx, |_, _| Handles {
        actions: BarVisualizerActions::new(),
    });
    let actions = held.read(cx).actions.clone();
    let vertical = is_vertical(p.choice("orientation"), &ctx.config());
    let count = bar_count(p);

    let mut bars = BarVisualizer::new(ctx.id("bars"))
        .config(ctx.config())
        .actions(&actions)
        .bar_count(count)
        .align(align(p.choice("align")))
        .mirrored(p.bool("mirrored"))
        .min_level(p.num("min_level"))
        .idle(idle(p.choice("idle")))
        .loading(p.bool("loading"))
        .bar_width(px(p.num("bar_width")))
        .bar_gap(px(p.num("bar_gap")))
        .bar_radius(px(p.num("bar_radius")))
        .aria_label(p.text("aria_label"))
        .aria_hidden(p.bool("aria_hidden"));
    bars = match p.choice("orientation") {
        "horizontal" => bars.orientation(Orientation::Horizontal),
        "vertical" => bars.orientation(Orientation::Vertical),
        _ => bars,
    };
    if let Some(color) = p.color("text_color", cx) {
        bars = bars.text_color(color);
    }
    bars = match p.choice("data") {
        "voice" | "bass" | "flat" => bars.levels(preset_levels(p.choice("data"))),
        "none" => bars,
        _ => match ctx.visual() {
            Some(source) => bars.source(source),
            None => bars,
        },
    };
    bars = if vertical {
        bars.w(px(112.0)).h(px(280.0))
    } else {
        bars.w_full().h(px(112.0))
    };

    let burst = actions.clone();
    let floor = actions.clone();
    let active = actions.clone();
    let log_burst = ctx.clone();
    let log_floor = ctx.clone();
    let log_active = ctx.clone();

    let mut column = div().w_full().flex().flex_col().gap(px(12.0));
    column = if vertical {
        column.items_center()
    } else {
        column.items_stretch()
    };
    column
        .child(bars)
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(px(8.0))
                .child(action_button(
                    ctx.id("paint"),
                    "Paint burst",
                    move |_, _, cx| {
                        let levels = burst_levels(count);
                        burst.paint(&levels, cx);
                        log_burst.log("actions.paint", format!("{count} bars, burst"), cx);
                    },
                ))
                .child(action_button(
                    ctx.id("floor"),
                    "Paint floor",
                    move |_, _, cx| {
                        floor.paint(&vec![0.0; count], cx);
                        log_floor.log("actions.paint", format!("{count} bars, floor"), cx);
                    },
                ))
                .child(action_button(
                    ctx.id("active"),
                    "Log active",
                    move |_, _, cx| {
                        log_active.log("actions.is_active", active.is_active(cx).to_string(), cx);
                    },
                )),
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

fn bar_count(p: &Props) -> usize {
    p.num("bar_count").max(1.0) as usize
}

fn is_vertical(choice: &str, config: &AudioConfig) -> bool {
    match choice {
        "vertical" => true,
        "horizontal" => false,
        _ => config.orientation == Some(Orientation::Vertical),
    }
}

fn align(choice: &str) -> BarVisualizerAlign {
    match choice {
        "start" => BarVisualizerAlign::Start,
        "end" => BarVisualizerAlign::End,
        _ => BarVisualizerAlign::Center,
    }
}

fn idle(choice: &str) -> BarIdle {
    match choice {
        "pulse" => BarIdle::Pulse,
        "wave" => BarIdle::Wave,
        _ => BarIdle::Static,
    }
}

fn preset_levels(name: &str) -> Vec<f32> {
    match name {
        "bass" => (0..16).map(|i| (1.0 - i as f32 / 15.0).powi(2)).collect(),
        "flat" => vec![0.22; 12],
        _ => vec![
            0.12, 0.28, 0.62, 0.9, 0.48, 0.74, 0.33, 0.18, 0.55, 0.86, 0.41, 0.22,
        ],
    }
}

fn burst_levels(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (i as f32 / count.max(1) as f32 * std::f32::consts::PI).sin())
        .collect()
}
