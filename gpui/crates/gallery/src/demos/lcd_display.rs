//! Gallery page for `lcd-display`. One variant per docs example:
//! `components/examples/lcd-display-demo.tsx`,
//! `components/examples/lcd-display-font.tsx`,
//! `components/examples/lcd-display-variants.tsx`.

use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

use audiocn::theme::ActiveTheme;
use audiocn::{
    LcdDisplay, LcdDisplayLabel, LcdDisplayRow, LcdDisplaySelect, LcdDisplaySelectContent,
    LcdDisplaySelectItem, LcdDisplaySelectTrigger, LcdDisplaySeparator, LcdDisplayStatus,
    LcdDisplayStatusState, LcdDisplayValue, LcdDisplayVariant,
};

use super::{Demo, id};

pub const DEMO: Demo = Demo {
    slug: "lcd-display",
    title: "LCD Display",
    description: "A dot-matrix screen for labelled readouts and settings, backlit in the light theme and glowing in the dark one, with dropdown values.",
    variants: &["default", "font", "variants"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "font" => font(cx),
        "variants" => variants(),
        _ => return None,
    })
}

const MODELS: [(&str, &str); 3] = [
    ("opus", "Opus 5.5"),
    ("sonnet", "Sonnet 5.5"),
    ("haiku", "Haiku 5.5"),
];

const EFFORTS: [(&str, &str); 4] = [
    ("low", "Low"),
    ("med", "Med"),
    ("high", "High"),
    ("ultra", "Ultra"),
];

/// A controlled select over `items`, writing its value into `slot` of the demo state.
fn select(
    select_id: &str,
    items: &'static [(&'static str, &'static str)],
    value: SharedString,
    on_change: impl Fn(SharedString, &mut App) + 'static,
) -> LcdDisplaySelect {
    LcdDisplaySelect::new(SharedString::from(select_id.to_string()))
        .items(items.iter().copied())
        .value(value)
        .on_value_change(move |next, _, cx| on_change(next, cx))
        .child(LcdDisplaySelectTrigger::new())
        .child(
            LcdDisplaySelectContent::new().children(
                items
                    .iter()
                    .map(|(value, label)| LcdDisplaySelectItem::new(*value, *label)),
            ),
        )
}

/// `lcd-display-demo.tsx`: model and effort selects, a separator and the status pixel.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("lcd-display", "default");
    let state = window.use_keyed_state(base.clone(), cx, |_, _| {
        (SharedString::from("opus"), SharedString::from("high"))
    });
    let (model, effort) = state.read(cx).clone();
    let (s1, s2) = (state.clone(), state);
    LcdDisplay::new(base)
        .w_full()
        .max_w(px(320.0))
        .child(
            LcdDisplayRow::new()
                .child(LcdDisplayLabel::new("Model"))
                .child(select("model", &MODELS, model, move |next, cx| {
                    s1.update(cx, |s, cx| {
                        s.0 = next;
                        cx.notify();
                    })
                })),
        )
        .child(LcdDisplaySeparator::new())
        .child(
            LcdDisplayRow::new()
                .child(LcdDisplayLabel::new("Effort"))
                .child(select("effort", &EFFORTS, effort, move |next, cx| {
                    s2.update(cx, |s, cx| {
                        s.1 = next;
                        cx.notify();
                    })
                })),
        )
        .child(LcdDisplayStatus::new())
        .into_any_element()
}

/// `lcd-display-font.tsx`: Doto is a dot-matrix face, so the screen's own dot
/// mask is turned off.
fn font(cx: &mut App) -> AnyElement {
    LcdDisplay::new(id("lcd-display", "font"))
        .font(cx.theme().font_doto.clone())
        // audiocn.dev loads Doto at its heaviest weight only, so the dots fill out.
        .font_weight(FontWeight::BLACK)
        .matrix(false)
        .w_full()
        .max_w(px(320.0))
        .child(
            LcdDisplayRow::new()
                .child(LcdDisplayLabel::new("Model"))
                .child(LcdDisplayValue::new("model", "Opus 5.5")),
        )
        .child(LcdDisplaySeparator::new())
        .child(
            LcdDisplayRow::new()
                .child(LcdDisplayLabel::new("Effort"))
                .child(LcdDisplayValue::new("effort", "High")),
        )
        .child(LcdDisplayStatus::new().state(LcdDisplayStatusState::Busy))
        .into_any_element()
}

const READINGS: [(&str, &str, LcdDisplayVariant); 4] = [
    ("Tempo", "128.0", LcdDisplayVariant::Amber),
    ("Input", "-12 dB", LcdDisplayVariant::Green),
    ("Sync", "MIDI", LcdDisplayVariant::Cyan),
    ("Rec", "00:42", LcdDisplayVariant::Red),
];

/// `lcd-display-variants.tsx`: the four hardware inks in a two-column grid
/// (`grid w-full max-w-xl gap-4 sm:grid-cols-2`).
fn variants() -> AnyElement {
    div()
        .grid()
        .grid_cols(2)
        .gap(px(16.0))
        .w_full()
        .max_w(px(576.0))
        .children(READINGS.iter().map(|(label, value, variant)| {
            LcdDisplay::new(SharedString::from(format!("lcd-display/variants/{label}")))
                .variant(*variant)
                .child(
                    LcdDisplayRow::new()
                        .child(LcdDisplayLabel::new(*label))
                        .child(LcdDisplayValue::new("value", *value)),
                )
        }))
        .into_any_element()
}
