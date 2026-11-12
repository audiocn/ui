//! Playground page for `Fader`: every prop, every taper (with custom
//! parameters), detent presets, both orientations and every part.

use audiocn::audio::taper::{AudioTaperOptions, TaperInput, audio_taper};
use audiocn::{
    AudioSize, DbScaleSide, Fader, FaderLabel, FaderRange, FaderReset, FaderScale, FaderThumb,
    FaderTrack, FaderValue, FaderVariant, LevelMeter, Orientation,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "fader",
    component: "Fader",
    source: SourceUse::Parts,
    controls,
    render,
};

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

/// The component's default `min`; the origin control at this value means
/// "unset" (the fill starts at `min`).
const DEFAULT_MIN: f32 = -60.0;

fn controls() -> Vec<Control> {
    vec![
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::num("value", "Value (dB)", -6.0, -120.0, 24.0, 0.1).group("Value"),
        Control::num(
            "default_value",
            "Default value (dB)",
            -6.0,
            -120.0,
            24.0,
            0.1,
        )
        .group("Value")
        .remount(),
        Control::num("reset_value", "Reset value (dB)", 0.0, -120.0, 24.0, 0.1)
            .group("Value")
            .help("Restored by double-clicking the thumb and by FaderReset."),
        // Range
        Control::num("min", "Min (dB)", DEFAULT_MIN, -120.0, 0.0, 1.0).group("Range"),
        Control::num("max", "Max (dB)", 6.0, -12.0, 24.0, 1.0).group("Range"),
        Control::num("origin", "Origin (dB)", DEFAULT_MIN, -120.0, 24.0, 0.5)
            .group("Range")
            .help("Where the fill starts. -60 = unset (min); 0 = bipolar gain."),
        Control::bool("silence_at_min", "Silence at min", false)
            .group("Range")
            .help("The bottom position reports -inf (\"Silent\")."),
        Control::choice(
            "detents",
            "Detents",
            0,
            &[
                ("[0] (default)", "vec![0.0]"),
                ("none", "Vec::<f32>::new()"),
                ("[-12, -6, 0]", "vec![-12.0, -6.0, 0.0]"),
                ("[-24, -12, 0, 6]", "vec![-24.0, -12.0, 0.0, 6.0]"),
                (
                    "every 6 dB",
                    "(-10..=1).map(|i| i as f32 * 6.0).collect::<Vec<_>>()",
                ),
            ],
        )
        .group("Range")
        .help("Values the thumb snaps to while dragging."),
        // Taper
        Control::choice(
            "taper",
            "Taper",
            0,
            &[
                ("linear", "TaperInput::Linear"),
                ("audio", "TaperInput::Audio"),
                ("log (needs min > 0)", "TaperInput::Log"),
                (
                    "audio_taper (custom unity)",
                    "audio_taper(AudioTaperOptions { min_db, max_db, unity_position })",
                ),
                (
                    "custom power curve",
                    "TaperInput::custom(to_position, to_value)",
                ),
            ],
        )
        .group("Taper"),
        Control::num("unity_position", "Unity position", 0.75, 0.0, 1.0, 0.01)
            .group("Taper")
            .no_code()
            .help("audio_taper: where 0 dB sits on the travel."),
        Control::num("curve_exponent", "Curve exponent", 2.0, 0.25, 4.0, 0.05)
            .group("Taper")
            .no_code()
            .help("Custom power curve: position = t^(1/exp)."),
        // Steps
        Control::num("step", "Step", 0.5, 0.01, 12.0, 0.01).group("Steps"),
        Control::num("large_step", "Large step", 6.0, 0.1, 24.0, 0.1).group("Steps"),
        Control::num("fine_step", "Fine step", 0.1, 0.01, 6.0, 0.01).group("Steps"),
        // Interaction
        Control::bool("allow_wheel", "Allow wheel", false)
            .group("Interaction")
            .help("The wheel adjusts the value while focused."),
        Control::bool("disabled", "Disabled", false).group("Interaction"),
        // Look
        Control::choice(
            "orientation",
            "Orientation",
            0,
            &[
                ("unset (config)", ""),
                ("horizontal", "Orientation::Horizontal"),
                ("vertical", "Orientation::Vertical"),
            ],
        )
        .group("Look"),
        Control::choice(
            "variant",
            "Variant",
            0,
            &[
                ("default", "FaderVariant::Default"),
                ("console", "FaderVariant::Console"),
            ],
        )
        .group("Look"),
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::choice(
            "format",
            "Format",
            0,
            &[
                ("default (Silent / dB)", ""),
                ("1 decimal + dB", "|db| format!(\"{db:.1} dB\")"),
                ("whole dB", "|db| format!(\"{db:.0}\")"),
                (
                    "gain multiplier",
                    "|db| format!(\"×{:.2}\", 10f32.powf(db / 20.0))",
                ),
            ],
        )
        .group("Look"),
        Control::text("aria_label", "Aria label", "").group("Look"),
        Control::num("length", "Length (px)", 320.0, 120.0, 720.0, 10.0)
            .group("Look")
            .no_code()
            .help("Max width when horizontal, height when vertical."),
        // Parts
        Control::bool("parts", "Compose parts", true)
            .group("Parts")
            .no_code()
            .help("Off: no children (the fader draws track, range and thumb)."),
        Control::text("label", "FaderLabel", "Microphone")
            .group("Parts")
            .no_code()
            .help("Empty hides the label."),
        Control::bool("show_reset", "FaderReset", true)
            .group("Parts")
            .no_code(),
        Control::text("reset_text", "Reset text", "")
            .group("Parts")
            .no_code()
            .help("Custom FaderReset children (empty = \"Reset\")."),
        Control::bool("show_value", "FaderValue", true)
            .group("Parts")
            .no_code(),
        Control::bool("editable", "Editable value", true)
            .group("Parts")
            .no_code()
            .help("Click the value to type \"-6\", \"3 dB\" or \"-inf\"."),
        Control::bool("show_range", "FaderRange", true)
            .group("Parts")
            .no_code(),
        Control::color("range_color", "Range colour")
            .group("Parts")
            .no_code(),
        Control::bool("show_thumb", "FaderThumb", true)
            .group("Parts")
            .no_code(),
        Control::bool("track_meter", "LevelMeter in track", false)
            .group("Parts")
            .no_code()
            .help("Reads the source picker's meter frames."),
        // Scale
        Control::bool("show_scale", "FaderScale", true)
            .group("Scale")
            .no_code(),
        Control::choice(
            "scale_ticks",
            "Ticks",
            0,
            &[
                ("default", ""),
                ("[6, 0, -10, -20, -40, -80]", ""),
                ("[-24, -12, 0, 12, 24]", ""),
                ("every 6 dB", ""),
            ],
        )
        .group("Scale")
        .no_code(),
        Control::choice("scale_side", "Side", 1, &[("start", ""), ("end", "")])
            .group("Scale")
            .no_code(),
        Control::bool("scale_labels", "Labels", true)
            .group("Scale")
            .no_code(),
        Control::bool("scale_marks", "Marks", true)
            .group("Scale")
            .no_code(),
    ]
}

fn taper(p: &Props) -> TaperInput {
    let (min, max) = (p.num("min"), p.num("max"));
    match p.index("taper") {
        1 => TaperInput::Audio,
        2 => TaperInput::Log,
        3 => audio_taper(AudioTaperOptions {
            min_db: min,
            max_db: max,
            unity_position: p.num("unity_position"),
        })
        .into(),
        4 => {
            let exp = p.num("curve_exponent").max(0.01);
            let span = (max - min).max(f32::EPSILON);
            TaperInput::custom(
                move |db| {
                    if db.is_nan() || db <= min {
                        0.0
                    } else {
                        ((db - min) / span).clamp(0.0, 1.0).powf(1.0 / exp)
                    }
                },
                move |pos| min + pos.clamp(0.0, 1.0).powf(exp) * span,
            )
        }
        _ => TaperInput::Linear,
    }
}

fn detents(p: &Props) -> Vec<f32> {
    match p.index("detents") {
        1 => Vec::new(),
        2 => vec![-12.0, -6.0, 0.0],
        3 => vec![-24.0, -12.0, 0.0, 6.0],
        4 => (-10..=1).map(|i| i as f32 * 6.0).collect(),
        _ => vec![0.0],
    }
}

fn scale(p: &Props) -> FaderScale {
    let mut scale = FaderScale::new()
        .side(if p.choice("scale_side") == "start" {
            DbScaleSide::Start
        } else {
            DbScaleSide::End
        })
        .labels(p.bool("scale_labels"))
        .marks(p.bool("scale_marks"));
    scale = match p.index("scale_ticks") {
        1 => scale.ticks([6.0, 0.0, -10.0, -20.0, -40.0, -80.0]),
        2 => scale.ticks([-24.0, -12.0, 0.0, 12.0, 24.0]),
        3 => scale.ticks((-20..=4).map(|i| i as f32 * 6.0)),
        _ => scale,
    };
    scale
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let orientation = match p.choice("orientation") {
        "horizontal" => Some(Orientation::Horizontal),
        "vertical" => Some(Orientation::Vertical),
        _ => None,
    };
    let vertical =
        orientation.or(ctx.config().orientation).unwrap_or_default() == Orientation::Vertical;

    let mut fader = Fader::new(ctx.id("fader"))
        .min(p.num("min"))
        .max(p.num("max"))
        .step(p.num("step"))
        .large_step(p.num("large_step"))
        .fine_step(p.num("fine_step"))
        .reset_value(p.num("reset_value"))
        .taper(taper(p))
        .detents(detents(p))
        .silence_at_min(p.bool("silence_at_min"))
        .allow_wheel(p.bool("allow_wheel"))
        .disabled(p.bool("disabled"))
        .variant(match p.choice("variant") {
            "console" => FaderVariant::Console,
            _ => FaderVariant::Default,
        })
        .config(ctx.config());
    if p.num("origin") != DEFAULT_MIN {
        fader = fader.origin(p.num("origin"));
    }
    if let Some(o) = orientation {
        fader = fader.orientation(o);
    }
    fader = match p.choice("size") {
        "sm" => fader.size(AudioSize::Sm),
        "default" => fader.size(AudioSize::Default),
        "lg" => fader.size(AudioSize::Lg),
        _ => fader,
    };
    fader = match p.index("format") {
        1 => fader.format(|db| format!("{db:.1} dB")),
        2 => fader.format(|db| format!("{db:.0}")),
        3 => fader.format(|db| format!("×{:.2}", 10f32.powf(db / 20.0))),
        _ => fader,
    };
    let aria = p.text("aria_label");
    if !aria.is_empty() {
        fader = fader.aria_label(aria);
    }
    fader = if controlled {
        fader.value(p.num("value"))
    } else {
        fader.default_value(p.num("default_value"))
    };

    let set = ctx.clone();
    let commit = ctx.clone();
    fader = fader
        .on_value_change_with_details(move |db, details, _, cx| {
            if controlled {
                set.set_num("value", db, cx);
            }
            set.log(
                "on_value_change",
                format!("{db:.2} dB ({:?})", details.reason),
                cx,
            );
        })
        .on_value_committed(move |db, _, cx| {
            commit.log("on_value_committed", format!("{db:.2} dB"), cx)
        });

    let length = px(p.num("length"));
    if !p.bool("parts") {
        let fader = if vertical {
            fader.h(length)
        } else {
            fader.w_full().max_w(length)
        };
        return fader.into_any_element();
    }

    // The track: range + thumb, optionally over a level meter.
    let mut track = FaderTrack::new();
    if p.bool("track_meter") {
        track = if vertical {
            track.w(px(24.0))
        } else {
            track.h(px(24.0))
        };
        track = track.bg(gpui::transparent_black()).child(
            LevelMeter::new(ctx.id("meter"))
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .min_db(p.num("min"))
                .max_db(p.num("max"))
                .orientation(if vertical {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                })
                .size(AudioSize::Sm)
                .source(ctx.meter()),
        );
    }
    if p.bool("show_range") {
        let mut range = FaderRange::new();
        if let Some(color) = p.color("range_color", cx) {
            range = range.bg(color);
        }
        track = track.child(range);
    }
    if p.bool("show_thumb") {
        track = track.child(FaderThumb::new());
    }

    let label = p.text("label");
    let reset = p.bool("show_reset").then(|| {
        let text = p.text("reset_text");
        let reset = FaderReset::new();
        if text.is_empty() {
            reset
        } else {
            reset.child(text)
        }
    });
    let value = p
        .bool("show_value")
        .then(|| FaderValue::new().editable(p.bool("editable")));
    let scale = p.bool("show_scale").then(|| scale(p));

    if vertical {
        // `fader-vertical.tsx`: value on top, track + scale, label below.
        let mut middle = div().flex().min_h_0().flex_1().gap_1().child(track);
        if let Some(scale) = scale {
            middle = middle.child(scale);
        }
        fader = fader.flex_col().items_center().h(length);
        if let Some(value) = value {
            fader = fader.child(value);
        }
        fader = fader.child(middle);
        if !label.is_empty() {
            fader = fader.child(
                FaderLabel::new(label)
                    .text_size(px(12.0))
                    .line_height(px(16.0)),
            );
        }
        if let Some(reset) = reset {
            fader = fader.child(reset);
        }
    } else {
        // `fader-demo.tsx`: header row, track, scale.
        let mut header = div().flex().items_center().gap_2();
        header = if label.is_empty() {
            header.child(div().mr_auto())
        } else {
            header.child(FaderLabel::new(label).mr_auto())
        };
        if let Some(reset) = reset {
            header = header.child(reset);
        }
        if let Some(value) = value {
            header = header.child(value);
        }
        fader = fader.w_full().max_w(length).child(header).child(track);
        if let Some(scale) = scale {
            fader = fader.child(scale);
        }
    }
    fader.into_any_element()
}
