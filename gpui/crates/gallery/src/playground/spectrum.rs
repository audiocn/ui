//! Playground page for `Spectrum`: range, scale, colours and axis parts.

use audiocn::components::{
    Spectrum, SpectrumCanvas, SpectrumFrequencyAxis, SpectrumLevelAxis, SpectrumScale,
    SpectrumVariant,
};
use gpui::{AnyElement, App, IntoElement, SharedString, Styled, Window, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "spectrum",
    component: "Spectrum",
    source: SourceUse::Visual,
    controls,
    render,
};

const VARIANT: &[(&str, &str)] = &[
    ("bars", "SpectrumVariant::Bars"),
    ("line", "SpectrumVariant::Line"),
    ("area", "SpectrumVariant::Area"),
];

const SCALE: &[(&str, &str)] = &[
    ("log", "SpectrumScale::Log"),
    ("linear", "SpectrumScale::Linear"),
];

const LEVEL_TICKS: &[(&str, &str)] = &[("default", ""), ("ends", ""), ("6 dB", "")];

const LEVEL_FORMAT: &[(&str, &str)] = &[("rounded", ""), ("dB", "")];

const FREQUENCY_TICKS: &[(&str, &str)] = &[
    ("default", ""),
    ("octaves", ""),
    ("speech", ""),
    ("sparse", ""),
];

const FREQUENCY_FORMAT: &[(&str, &str)] = &[("compact", ""), ("hertz", "")];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("variant", "Variant", 0, VARIANT).group("Plot"),
        Control::bool("peak_hold", "Peak hold", false).group("Plot"),
        Control::bool("grid", "Grid", true).group("Plot"),
        Control::choice("scale", "Scale", 0, SCALE).group("Range"),
        Control::num("min_db", "Min dB", -100.0, -140.0, 0.0, 1.0).group("Range"),
        Control::num("max_db", "Max dB", -30.0, -80.0, 12.0, 1.0).group("Range"),
        Control::num("min_hz", "Min Hz", 40.0, 10.0, 2000.0, 10.0)
            .group("Range")
            .help("Kept at least 1 Hz, and max is raised so the span is never zero."),
        Control::num("max_hz", "Max Hz", 16_000.0, 1000.0, 24_000.0, 100.0).group("Range"),
        Control::color("color", "Colour").group("Colour"),
        Control::color("peak_color", "Peak colour").group("Colour"),
        Control::color("grid_color", "Grid colour").group("Colour"),
        Control::text("aria_label", "ARIA label", "Frequency spectrum").group("Accessibility"),
        Control::bool("show_level", "Level axis", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_canvas", "Canvas", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_frequency", "Frequency axis", true)
            .group("Parts")
            .no_code(),
        Control::choice("level_ticks", "Level ticks", 0, LEVEL_TICKS)
            .group("Level axis")
            .no_code()
            .help("default is every 12 dB. ends uses min, mid and max."),
        Control::choice("level_format", "Level format", 0, LEVEL_FORMAT)
            .group("Level axis")
            .no_code(),
        Control::choice("frequency_ticks", "Frequency ticks", 0, FREQUENCY_TICKS)
            .group("Frequency axis")
            .no_code()
            .help("Ticks outside min/max Hz are hidden by the axis."),
        Control::choice("frequency_format", "Frequency format", 0, FREQUENCY_FORMAT)
            .group("Frequency axis")
            .no_code(),
        Control::color("canvas_color", "Canvas colour")
            .group("Canvas")
            .no_code()
            .help("Overrides the spectrum colour for this canvas."),
        Control::color("canvas_peak_color", "Canvas peak")
            .group("Canvas")
            .no_code(),
        Control::color("canvas_grid_color", "Canvas grid")
            .group("Canvas")
            .no_code(),
    ]
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let min_db = p.num("min_db");
    let max_db = p.num("max_db").max(min_db + 1.0);
    let min_hz = p.num("min_hz").max(1.0);
    let max_hz = p.num("max_hz").max(min_hz + 1.0);

    let mut spectrum = Spectrum::new(ctx.id("spectrum"))
        .variant(variant(p.choice("variant")))
        .scale(scale(p.choice("scale")))
        .min_db(min_db)
        .max_db(max_db)
        .min_hz(min_hz)
        .max_hz(max_hz)
        .peak_hold(p.bool("peak_hold"))
        .grid(p.bool("grid"))
        .aria_label(p.text("aria_label"))
        .w_full()
        .h(px(220.0));
    if let Some(source) = ctx.visual() {
        spectrum = spectrum.source(source);
    }
    if let Some(color) = p.color("color", cx) {
        spectrum = spectrum.color(color);
    }
    if let Some(color) = p.color("peak_color", cx) {
        spectrum = spectrum.peak_color(color);
    }
    if let Some(color) = p.color("grid_color", cx) {
        spectrum = spectrum.grid_color(color);
    }

    let show_level = p.bool("show_level");
    let show_canvas = p.bool("show_canvas");
    let show_frequency = p.bool("show_frequency");
    if show_level {
        spectrum = spectrum.child(level_axis(p, min_db, max_db));
    }
    if show_canvas {
        spectrum = spectrum.child(canvas(p, cx));
    }
    if show_frequency {
        spectrum = spectrum.child(frequency_axis(p));
    }
    if !(show_level || show_canvas || show_frequency) {
        // Empty children restore all three parts.
        spectrum = spectrum.child(SpectrumLevelAxis::new().ticks(Vec::<f32>::new()));
    }
    spectrum.into_any_element()
}

fn level_axis(p: &Props, min_db: f32, max_db: f32) -> SpectrumLevelAxis {
    let mut axis = SpectrumLevelAxis::new();
    if let Some(ticks) = level_ticks(p.choice("level_ticks"), min_db, max_db) {
        axis = axis.ticks(ticks);
    }
    if p.choice("level_format") == "dB" {
        axis = axis.format(|db| SharedString::from(format!("{db:.0} dB")));
    }
    axis
}

fn frequency_axis(p: &Props) -> SpectrumFrequencyAxis {
    let mut axis = SpectrumFrequencyAxis::new().ticks(frequency_ticks(p.choice("frequency_ticks")));
    if p.choice("frequency_format") == "hertz" {
        axis = axis.format(|hz| SharedString::from(format!("{hz:.0} Hz")));
    }
    axis
}

fn canvas(p: &Props, cx: &App) -> SpectrumCanvas {
    let mut canvas = SpectrumCanvas::new();
    if let Some(color) = p.color("canvas_color", cx) {
        canvas = canvas.color(color);
    }
    if let Some(color) = p.color("canvas_peak_color", cx) {
        canvas = canvas.peak_color(color);
    }
    if let Some(color) = p.color("canvas_grid_color", cx) {
        canvas = canvas.grid_color(color);
    }
    canvas
}

fn variant(choice: &str) -> SpectrumVariant {
    match choice {
        "line" => SpectrumVariant::Line,
        "area" => SpectrumVariant::Area,
        _ => SpectrumVariant::Bars,
    }
}

fn scale(choice: &str) -> SpectrumScale {
    match choice {
        "linear" => SpectrumScale::Linear,
        _ => SpectrumScale::Log,
    }
}

fn level_ticks(preset: &str, min_db: f32, max_db: f32) -> Option<Vec<f32>> {
    match preset {
        "ends" => Some(vec![max_db, (min_db + max_db) * 0.5, min_db]),
        "6 dB" => {
            let mut ticks = Vec::new();
            let mut db = max_db;
            for _ in 0..40 {
                if db < min_db - 0.01 {
                    break;
                }
                ticks.push(db);
                db -= 6.0;
            }
            Some(ticks)
        }
        _ => None,
    }
}

fn frequency_ticks(preset: &str) -> Vec<f32> {
    match preset {
        "octaves" => vec![
            50.0, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ],
        "speech" => vec![200.0, 500.0, 1_000.0, 2_000.0, 4_000.0],
        "sparse" => vec![100.0, 1_000.0],
        _ => vec![100.0, 1_000.0, 10_000.0],
    }
}
