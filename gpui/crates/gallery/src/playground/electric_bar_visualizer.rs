//! Playground page for `ElectricBarVisualizer`: layout, motion, colour and the paint handle.

use audiocn::audio::bar_levels::BarIdle;
use audiocn::components::{ElectricBarAlign, ElectricBarVisualizer, ElectricBarVisualizerActions};
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{AudioConfig, Orientation};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "electric-bar-visualizer",
    component: "ElectricBarVisualizer",
    source: SourceUse::Visual,
    controls,
    render,
};

const ALIGN: &[(&str, &str)] = &[
    ("center", "ElectricBarAlign::Center"),
    ("start", "ElectricBarAlign::Start"),
    ("end", "ElectricBarAlign::End"),
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
        Control::num("bar_count", "Bar count", 16.0, 1.0, 64.0, 1.0)
            .group("Bars")
            .code("bar_count({} as usize)"),
        Control::choice("align", "Align", 0, ALIGN).group("Bars"),
        Control::bool("mirrored", "Mirrored", false).group("Bars"),
        Control::num("min_level", "Min level", 0.08, 0.0, 1.0, 0.01).group("Bars"),
        Control::choice("idle", "Idle", 0, IDLE).group("Motion"),
        Control::bool("loading", "Loading", false)
            .group("Motion")
            .help("An arc along the bars, for connecting or thinking."),
        Control::num("intensity", "Intensity", 0.6, 0.0, 1.0, 0.05).group("Motion"),
        Control::bool("arcs", "Arcs", true).group("Motion"),
        Control::bool("sparks", "Sparks", true).group("Motion"),
        Control::choice("orientation", "Orientation", 0, ORIENTATION)
            .group("Layout")
            .help("from config uses the toolbar. Otherwise it overrides AudioConfig."),
        Control::num("bar_width", "Bar width", 6.0, 1.0, 32.0, 1.0).group("Layout"),
        Control::num("bar_gap", "Bar gap", 4.0, 0.0, 24.0, 1.0).group("Layout"),
        Control::color("text_color", "Text colour")
            .group("Colour")
            .help("Fallback filament colour when Electric is default."),
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
        Control::text("aria_label", "ARIA label", "Audio visualizer").group("Accessibility"),
    ]
}

struct Handles {
    actions: ElectricBarVisualizerActions,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let held = window.use_keyed_state(ctx.id("actions"), cx, |_, _| Handles {
        actions: ElectricBarVisualizerActions::new(),
    });
    let actions = held.read(cx).actions.clone();
    let vertical = is_vertical(p.choice("orientation"), &ctx.config());
    let count = bar_count(p);

    let mut bars = ElectricBarVisualizer::new(ctx.id("bars"))
        .config(ctx.config())
        .actions(actions.clone())
        .bar_count(count)
        .align(align(p.choice("align")))
        .mirrored(p.bool("mirrored"))
        .min_level(p.num("min_level"))
        .idle(idle(p.choice("idle")))
        .loading(p.bool("loading"))
        .intensity(p.num("intensity"))
        .arcs(p.bool("arcs"))
        .sparks(p.bool("sparks"))
        .bar_width(p.num("bar_width"))
        .bar_gap(p.num("bar_gap"))
        .electric_glow_size(p.num("electric_glow_size"))
        .aria_label(p.text("aria_label"));
    bars = match p.choice("orientation") {
        "horizontal" => bars.orientation(Orientation::Horizontal),
        "vertical" => bars.orientation(Orientation::Vertical),
        _ => bars,
    };
    if let Some(color) = p.color("text_color", cx) {
        bars = bars.text_color(color);
    }
    if let Some(color) = p.color("electric", cx) {
        bars = bars.electric(color);
    }
    if let Some(color) = p.color("electric_core", cx) {
        bars = bars.electric_core(color);
    }
    if let Some(color) = p.color("electric_glow", cx) {
        bars = bars.electric_glow(color);
    }
    if p.bool("custom_heat") {
        bars = bars.electric_heat(p.num("electric_heat"));
    }
    bars = match p.choice("data") {
        "voice" | "bass" | "flat" => bars.levels(preset_levels(p.choice("data"))),
        "none" => bars,
        _ => bars.source(ctx.visual()),
    };
    bars = if vertical {
        bars.w(px(112.0)).h(px(280.0))
    } else {
        bars.w_full().h(px(112.0))
    };

    let burst = actions.clone();
    let floor = actions.clone();
    let log_burst = ctx.clone();
    let log_floor = ctx.clone();
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
                        burst.paint(&burst_levels(count));
                        log_burst.log("actions.paint", format!("{count} bars, burst"), cx);
                    },
                ))
                .child(action_button(
                    ctx.id("floor"),
                    "Paint floor",
                    move |_, _, cx| {
                        floor.paint(&vec![0.0; count]);
                        log_floor.log("actions.paint", format!("{count} bars, floor"), cx);
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

fn align(choice: &str) -> ElectricBarAlign {
    match choice {
        "start" => ElectricBarAlign::Start,
        "end" => ElectricBarAlign::End,
        _ => ElectricBarAlign::Center,
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
