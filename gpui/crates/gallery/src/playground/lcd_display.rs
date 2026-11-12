//! Playground page for `LcdDisplay`: variants, appearance, size, matrix,
//! scanlines, every `--lcd-display-*` builder (and `vars`), rows with labels
//! and values, the separator, the status pixel and the LCD select (items,
//! controlled / uncontrolled value and open state, trigger and content props).

use audiocn::components::{
    LcdDisplay, LcdDisplayAppearance, LcdDisplayLabel, LcdDisplayRow, LcdDisplaySelect,
    LcdDisplaySelectAlign, LcdDisplaySelectContent, LcdDisplaySelectItem, LcdDisplaySelectSide,
    LcdDisplaySelectTrigger, LcdDisplaySeparator, LcdDisplaySize, LcdDisplayStatus,
    LcdDisplayStatusState, LcdDisplayValue, LcdDisplayVariant, LcdDisplayVars,
};
use audiocn::theme::ActiveTheme;
use gpui::{
    AnyElement, App, BoxShadow, FontWeight, IntoElement, ParentElement, Styled, Window, hsla,
    point, px,
};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "lcd-display",
    component: "LcdDisplay",
    source: SourceUse::None,
    controls,
    render,
};

// ---------------------------------------------------------------------------
// Shared with the LcdMeter page

/// Variant, appearance, size, matrix, scanlines and pitch, in `group`.
pub(super) fn look_controls(group: &'static str, code: bool) -> Vec<Control> {
    let c = |control: Control| {
        let control = control.group(group);
        if code { control } else { control.no_code() }
    };
    vec![
        c(Control::choice(
            "variant",
            "Variant",
            0,
            &[
                ("primary", "LcdDisplayVariant::Primary"),
                ("amber", "LcdDisplayVariant::Amber"),
                ("green", "LcdDisplayVariant::Green"),
                ("cyan", "LcdDisplayVariant::Cyan"),
                ("red", "LcdDisplayVariant::Red"),
            ],
        )),
        c(Control::choice(
            "appearance",
            "Appearance",
            0,
            &[
                ("auto", "LcdDisplayAppearance::Auto"),
                ("light", "LcdDisplayAppearance::Light"),
                ("dark", "LcdDisplayAppearance::Dark"),
            ],
        )),
        c(Control::choice(
            "size",
            "Size",
            1,
            &[
                ("sm", "LcdDisplaySize::Sm"),
                ("default", "LcdDisplaySize::Default"),
                ("lg", "LcdDisplaySize::Lg"),
            ],
        )),
        c(Control::bool("matrix", "Matrix (dot mask)", true)),
        c(Control::bool("scanlines", "Scanlines", true)),
        c(Control::num(
            "pitch",
            "Pitch px (0 = default 2)",
            0.0,
            0.0,
            6.0,
            0.5,
        )),
    ]
}

/// Applies [`look_controls`].
pub(super) fn apply_look(mut display: LcdDisplay, p: &Props) -> LcdDisplay {
    display = display
        .variant(match p.choice("variant") {
            "amber" => LcdDisplayVariant::Amber,
            "green" => LcdDisplayVariant::Green,
            "cyan" => LcdDisplayVariant::Cyan,
            "red" => LcdDisplayVariant::Red,
            _ => LcdDisplayVariant::Primary,
        })
        .appearance(match p.choice("appearance") {
            "light" => LcdDisplayAppearance::Light,
            "dark" => LcdDisplayAppearance::Dark,
            _ => LcdDisplayAppearance::Auto,
        })
        .size(match p.choice("size") {
            "sm" => LcdDisplaySize::Sm,
            "lg" => LcdDisplaySize::Lg,
            _ => LcdDisplaySize::Default,
        })
        .matrix(p.bool("matrix"))
        .scanlines(p.bool("scanlines"));
    if p.num("pitch") > 0.0 {
        display = display.pitch(px(p.num("pitch")));
    }
    display
}

/// The status pixel's state controls, in `group`.
pub(super) fn status_controls(group: &'static str) -> Vec<Control> {
    vec![
        Control::bool("status", "Status pixel", true)
            .group(group)
            .no_code(),
        Control::choice(
            "status_state",
            "Status state",
            0,
            &[("idle", ""), ("busy", ""), ("off", "")],
        )
        .group(group)
        .no_code(),
    ]
}

pub(super) fn status(p: &Props) -> Option<LcdDisplayStatus> {
    p.bool("status").then(|| {
        LcdDisplayStatus::new().state(match p.choice("status_state") {
            "busy" => LcdDisplayStatusState::Busy,
            "off" => LcdDisplayStatusState::Off,
            _ => LcdDisplayStatusState::Idle,
        })
    })
}

// ---------------------------------------------------------------------------
// Controls

const COLOR_VARS: &[(&str, &str)] = &[
    ("ink", "Ink"),
    ("lit", "Lit"),
    ("ink_dim", "Ink dim (labels)"),
    ("ink_unlit", "Ink unlit (dots)"),
    ("glass", "Glass"),
    ("glow", "Glow"),
    ("bezel", "Bezel"),
    ("bezel_edge", "Bezel edge"),
    ("bezel_ring", "Bezel ring"),
    ("scanline", "Scanline"),
    ("sheen", "Sheen"),
];

const ITEM_SETS: &[(&str, &str)] = &[
    ("3 models", ""),
    ("6 efforts, one disabled", ""),
    ("1 item", ""),
    ("empty", ""),
];

fn items(index: usize) -> Vec<(&'static str, &'static str, bool)> {
    match index {
        1 => vec![
            ("min", "Min", false),
            ("low", "Low", false),
            ("med", "Med", false),
            ("high", "High", false),
            ("ultra", "Ultra", true),
            ("max", "Max", false),
        ],
        2 => vec![("only", "Only one", false)],
        3 => Vec::new(),
        _ => vec![
            ("opus", "Opus 5.5", false),
            ("sonnet", "Sonnet 5.5", false),
            ("haiku", "Haiku 5.5", false),
        ],
    }
}

fn controls() -> Vec<Control> {
    let mut out = look_controls("Look", true);
    // Every `--lcd-display-*` colour builder.
    out.extend(
        COLOR_VARS
            .iter()
            .map(|(key, label)| Control::color(key, label).group("Vars: colours")),
    );
    out.extend([
        Control::choice(
            "bezel_shadow",
            "Bezel shadow",
            0,
            &[
                ("default", ""),
                ("none", "vec![]"),
                ("deep", "vec![BoxShadow { offset: point(px(0.), px(8.)), blur_radius: px(24.), .. }]"),
            ],
        )
        .group("Vars: other"),
        Control::choice(
            "recess",
            "Recess (inset)",
            0,
            &[
                ("default", ""),
                ("none", "vec![]"),
                ("deep", "vec![BoxShadow { inset: true, blur_radius: px(10.), .. }]"),
            ],
        )
        .group("Vars: other"),
        Control::choice(
            "font",
            "Font",
            0,
            &[
                ("default (theme mono)", ""),
                ("Doto", "cx.theme().font_doto.clone()"),
                ("Geist Mono", "cx.theme().font_mono.clone()"),
                ("Geist (sans)", "cx.theme().font_sans.clone()"),
            ],
        )
        .group("Vars: other")
        .help("Doto is dot-matrix: turn Matrix off with it."),
        Control::choice(
            "font_weight",
            "Font weight",
            0,
            &[
                ("default", ""),
                ("normal", "FontWeight::NORMAL"),
                ("medium", "FontWeight::MEDIUM"),
                ("bold", "FontWeight::BOLD"),
                ("black", "FontWeight::BLACK"),
            ],
        )
        .group("Vars: other"),
        Control::choice(
            "vars",
            "vars(..) preset",
            0,
            &[
                ("none", ""),
                ("night: blue glass, white ink", "LcdDisplayVars { ink: Some(..), glass: Some(..), glow: Some(..), ..Default::default() }"),
            ],
        )
        .group("Vars: other")
        .help("Sets every variable at once; the builders above still override."),
        // Rows
        Control::num("rows", "Value rows", 1.0, 0.0, 4.0, 1.0)
            .group("Rows")
            .no_code()
            .help("Label + value rows (the first uses the texts below)."),
        Control::text("row_label", "First label", "Tempo")
            .group("Rows")
            .no_code(),
        Control::text("row_value", "First value (flickers on change)", "128.0")
            .group("Rows")
            .no_code(),
        Control::bool("value_live", "Value live", false)
            .group("Rows")
            .no_code(),
        Control::color("label_color", "Label colour")
            .group("Rows")
            .no_code(),
        Control::color("value_color", "Value colour")
            .group("Rows")
            .no_code(),
        Control::bool("separator", "Separator", true)
            .group("Rows")
            .no_code(),
    ]);
    out.extend(status_controls("Rows"));
    out.extend([
        // Select
        Control::bool("select", "Select row", true)
            .group("Select")
            .no_code(),
        Control::text("select_label", "Row label", "Model")
            .group("Select")
            .no_code(),
        Control::choice("select_items", "Items", 0, ITEM_SETS)
            .group("Select")
            .no_code()
            .remount(),
        Control::bool("select_controlled", "Controlled value", true)
            .group("Select")
            .no_code()
            .remount()
            .help("Off: default_value (remounts)."),
        Control::text("select_value", "Value", "opus")
            .group("Select")
            .no_code()
            .remount(),
        Control::bool("select_disabled", "Disabled", false)
            .group("Select")
            .no_code(),
        Control::bool("select_open_controlled", "Controlled open", false)
            .group("Select")
            .no_code()
            .remount(),
        Control::bool("select_open", "Open", false)
            .group("Select")
            .no_code(),
        Control::bool("select_default_open", "Default open", false)
            .group("Select")
            .no_code()
            .remount(),
        Control::text("trigger_placeholder", "Trigger placeholder", "Pick one")
            .group("Select: parts")
            .no_code(),
        Control::text("trigger_text", "Trigger text override", "")
            .group("Select: parts")
            .no_code(),
        Control::text("trigger_aria_label", "Trigger aria label", "")
            .group("Select: parts")
            .no_code(),
        Control::choice(
            "content_side",
            "Content side",
            2,
            &[("top", ""), ("right", ""), ("bottom", ""), ("left", "")],
        )
        .group("Select: parts")
        .no_code(),
        Control::choice(
            "content_align",
            "Content align",
            2,
            &[("start", ""), ("center", ""), ("end", "")],
        )
        .group("Select: parts")
        .no_code(),
        Control::num("content_side_offset", "Side offset", 8.0, -16.0, 48.0, 1.0)
            .group("Select: parts")
            .no_code(),
        Control::num(
            "content_align_offset",
            "Align offset",
            0.0,
            -48.0,
            48.0,
            1.0,
        )
        .group("Select: parts")
        .no_code(),
    ]);
    out
}

// ---------------------------------------------------------------------------
// Render

fn shadow(color: gpui::Hsla, y: f32, blur: f32, inset: bool) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.0), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
        inset,
    }
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let mut display = LcdDisplay::new(ctx.id("display")).w_full();
    if p.index("vars") == 1 {
        display = display.vars(LcdDisplayVars {
            ink: Some(hsla(0.0, 0.0, 0.97, 1.0)),
            glass: Some(hsla(0.62, 0.55, 0.16, 1.0)),
            glow: Some(hsla(0.58, 0.9, 0.7, 0.6)),
            bezel: Some(hsla(0.62, 0.2, 0.25, 1.0)),
            bezel_edge: Some(hsla(0.62, 0.2, 0.12, 1.0)),
            ..Default::default()
        });
    }
    display = apply_look(display, p);

    // `--lcd-display-*` colours.
    macro_rules! color_var {
        ($($key:ident),*) => {$(
            if let Some(color) = p.color(stringify!($key), cx) {
                display = display.$key(color);
            }
        )*};
    }
    color_var!(
        ink, lit, ink_dim, ink_unlit, glass, glow, bezel, bezel_edge, bezel_ring, scanline, sheen
    );
    let black = hsla(0.0, 0.0, 0.0, 1.0);
    display = match p.index("bezel_shadow") {
        1 => display.bezel_shadow(Vec::new()),
        2 => display.bezel_shadow(vec![
            shadow(black.opacity(0.35), 8.0, 24.0, false),
            shadow(black.opacity(0.2), 2.0, 4.0, false),
        ]),
        _ => display,
    };
    display = match p.index("recess") {
        1 => display.recess(Vec::new()),
        2 => display.recess(vec![shadow(black.opacity(0.6), 2.0, 10.0, true)]),
        _ => display,
    };
    let theme = cx.theme();
    display = match p.index("font") {
        1 => display.font(theme.font_doto.clone()),
        2 => display.font(theme.font_mono.clone()),
        3 => display.font(theme.font_sans.clone()),
        _ => display,
    };
    display = match p.choice("font_weight") {
        "normal" => display.font_weight(FontWeight::NORMAL),
        "medium" => display.font_weight(FontWeight::MEDIUM),
        "bold" => display.font_weight(FontWeight::BOLD),
        "black" => display.font_weight(FontWeight::BLACK),
        _ => display,
    };

    // Rows.
    let (label_color, value_color) = (p.color("label_color", cx), p.color("value_color", cx));
    let mut rows: Vec<AnyElement> = Vec::new();
    if p.bool("select") {
        rows.push(select_row(p, ctx).into_any_element());
    }
    const MORE: [(&str, &str); 3] = [("Key", "F♯ min"), ("Rate", "48 kHz"), ("Bits", "24")];
    for i in 0..p.num("rows").max(0.0) as usize {
        let (label, value) = if i == 0 {
            (p.text("row_label"), p.text("row_value"))
        } else {
            let (l, v) = MORE[(i - 1) % MORE.len()];
            (l.into(), v.into())
        };
        let mut label = LcdDisplayLabel::new(label);
        if let Some(color) = label_color {
            label = label.color(color);
        }
        let mut value =
            LcdDisplayValue::new(ctx.id(&format!("value-{i}")), value).live(p.bool("value_live"));
        if let Some(color) = value_color {
            value = value.color(color);
        }
        rows.push(
            LcdDisplayRow::new()
                .child(label)
                .child(value)
                .into_any_element(),
        );
    }
    for (i, row) in rows.into_iter().enumerate() {
        if i > 0 && p.bool("separator") {
            display = display.child(LcdDisplaySeparator::new());
        }
        display = display.child(row);
    }
    if let Some(status) = status(p) {
        display = display.child(status);
    }
    display.into_any_element()
}

fn select_row(p: &Props, ctx: &Ctx) -> LcdDisplayRow {
    let items = items(p.index("select_items"));
    let controlled = p.bool("select_controlled");
    let open_controlled = p.bool("select_open_controlled");
    let (on_value, on_open) = (ctx.clone(), ctx.clone());
    let mut select = LcdDisplaySelect::new(ctx.id("select"))
        .items(items.iter().map(|(value, label, _)| (*value, *label)))
        .disabled(p.bool("select_disabled"))
        .on_value_change(move |value, _, cx| {
            if controlled {
                on_value.set_text("select_value", value.clone(), cx);
            }
            on_value.log("on_value_change", value, cx);
        })
        .on_open_change(move |open, _, cx| {
            if open_controlled {
                on_open.set_bool("select_open", open, cx);
            }
            on_open.log("on_open_change", open.to_string(), cx);
        });
    let value = p.text("select_value");
    select = if controlled {
        select.value(value)
    } else {
        select.default_value(value)
    };
    select = if open_controlled {
        select.open(p.bool("select_open"))
    } else {
        select.default_open(p.bool("select_default_open"))
    };

    let mut trigger = LcdDisplaySelectTrigger::new();
    let placeholder = p.text("trigger_placeholder");
    if !placeholder.is_empty() {
        trigger = trigger.placeholder(placeholder);
    }
    let text = p.text("trigger_text");
    if !text.is_empty() {
        trigger = trigger.child(text);
    }
    let aria = p.text("trigger_aria_label");
    if !aria.is_empty() {
        trigger = trigger.aria_label(aria);
    }
    let content = LcdDisplaySelectContent::new()
        .side(match p.choice("content_side") {
            "top" => LcdDisplaySelectSide::Top,
            "right" => LcdDisplaySelectSide::Right,
            "left" => LcdDisplaySelectSide::Left,
            _ => LcdDisplaySelectSide::Bottom,
        })
        .align(match p.choice("content_align") {
            "start" => LcdDisplaySelectAlign::Start,
            "center" => LcdDisplaySelectAlign::Center,
            _ => LcdDisplaySelectAlign::End,
        })
        .side_offset(p.num("content_side_offset"))
        .align_offset(p.num("content_align_offset"))
        .children(items.iter().map(|(value, label, disabled)| {
            LcdDisplaySelectItem::new(*value, *label).disabled(*disabled)
        }));
    select = select.child(trigger).child(content);

    let mut row = LcdDisplayRow::new();
    let label = p.text("select_label");
    if !label.is_empty() {
        row = row.child(LcdDisplayLabel::new(label));
    }
    row.child(select)
}
