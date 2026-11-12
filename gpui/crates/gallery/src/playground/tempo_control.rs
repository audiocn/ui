//! Playground page for `TempoControl`: every prop, controlled and
//! uncontrolled, default or custom children (input + tap), the sound-pad tap
//! trigger, and every change reason in the event log.

use audiocn::theme::ActiveTheme;
use audiocn::{
    AudioSize, SoundPad, SoundPadLabel, TempoControl, TempoControlInput, TempoControlTap,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "tempo-control",
    component: "TempoControl",
    source: SourceUse::None,
    controls,
    render,
};

const SIZES: &[(&str, &str)] = &[
    ("unset", ""),
    ("sm", "AudioSize::Sm"),
    ("default", "AudioSize::Default"),
    ("lg", "AudioSize::Lg"),
];

fn controls() -> Vec<Control> {
    vec![
        // Value
        Control::bool("controlled", "Controlled", true)
            .group("Value")
            .no_code()
            .remount()
            .help("Off: uncontrolled with default_value (remounts)."),
        Control::num("value", "Value (BPM)", 120.0, 1.0, 999.0, 1.0).group("Value"),
        Control::num(
            "default_value",
            "Default value (BPM)",
            120.0,
            1.0,
            999.0,
            1.0,
        )
        .group("Value")
        .remount(),
        // Range
        Control::num("min", "Min (BPM)", 30.0, 1.0, 300.0, 1.0)
            .group("Range")
            .help("Rounded up. Must be positive."),
        Control::num("max", "Max (BPM)", 600.0, 60.0, 999.0, 1.0)
            .group("Range")
            .help("Rounded down."),
        Control::num("large_step", "Large step", 10.0, 1.0, 100.0, 1.0)
            .group("Range")
            .help("Shift+arrow and Page Up/Down."),
        Control::num(
            "tap_timeout",
            "Tap timeout (ms)",
            2000.0,
            250.0,
            10_000.0,
            50.0,
        )
        .group("Range")
        .help("Idle time before the next tap starts a new sequence."),
        // State
        Control::bool("disabled", "Disabled", false).group("State"),
        Control::bool("read_only", "Read-only", false)
            .group("State")
            .help("Focusable input, no edits, tap disabled."),
        Control::choice("size", "Size", 0, SIZES).group("Look"),
        Control::text("aria_label", "Aria label", "Tempo").group("Look"),
        Control::text("name", "Name", "")
            .group("Look")
            .help("Form field name (kept for parity; GPUI has no forms)."),
        // Parts
        Control::choice(
            "layout",
            "Children",
            0,
            &[
                ("default (no children)", ""),
                ("custom input + tap", ""),
                ("input only", ""),
                ("tap only", ""),
                ("sound pad tap (pad demo)", ""),
            ],
        )
        .group("Parts")
        .no_code(),
        Control::text(
            "input_aria_label",
            "Input aria label",
            "Tempo in beats per minute",
        )
        .group("Parts")
        .no_code(),
        Control::text("tap_text", "Tap text", "Tap tempo")
            .group("Parts")
            .no_code()
            .help("TempoControlTap children (empty = \"Tap\")."),
        Control::text("tap_aria_label", "Tap aria label", "Tap tempo")
            .group("Parts")
            .no_code(),
        Control::bool("tap_disabled", "Tap disabled", false)
            .group("Parts")
            .no_code(),
        Control::bool("prevent_tap", "Tap on_click prevents default", false)
            .group("Parts")
            .no_code()
            .help("TapClick::prevent_default cancels the tap."),
        Control::bool("show_beat", "Beat duration readout", false)
            .group("Parts")
            .no_code(),
    ]
}

fn tap(p: &Props, ctx: &Ctx, with_text: bool) -> TempoControlTap {
    let prevent = p.bool("prevent_tap");
    let log = ctx.clone();
    let mut tap = TempoControlTap::new()
        .aria_label(p.text("tap_aria_label"))
        .disabled(p.bool("tap_disabled"))
        .on_click(move |click, _, cx| {
            if prevent {
                click.prevent_default();
            }
            log.log(
                "tap.on_click",
                if prevent { "prevented" } else { "tap" },
                cx,
            );
        });
    let text = p.text("tap_text");
    if with_text && !text.is_empty() {
        tap = tap.child(text);
    }
    tap
}

fn input(p: &Props, ctx: &Ctx) -> TempoControlInput {
    let log = ctx.clone();
    TempoControlInput::new()
        .aria_label(p.text("input_aria_label"))
        .on_change(move |text, _, cx| log.log("input.on_change", format!("{text:?}"), cx))
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let controlled = p.bool("controlled");
    let mut tempo = TempoControl::new(ctx.id("tempo"))
        .min(p.num("min"))
        .max(p.num("max"))
        .large_step(p.num("large_step"))
        .tap_timeout(p.num("tap_timeout") as f64)
        .disabled(p.bool("disabled"))
        .read_only(p.bool("read_only"))
        .aria_label(p.text("aria_label"))
        .config(ctx.config());
    let name = p.text("name");
    if !name.is_empty() {
        tempo = tempo.name(name);
    }
    tempo = match p.choice("size") {
        "sm" => tempo.size(AudioSize::Sm),
        "default" => tempo.size(AudioSize::Default),
        "lg" => tempo.size(AudioSize::Lg),
        _ => tempo,
    };
    tempo = if controlled {
        tempo.value(p.num("value"))
    } else {
        tempo.default_value(p.num("default_value"))
    };

    let set = ctx.clone();
    let commit = ctx.clone();
    tempo = tempo
        .on_value_change(move |bpm, details, _, cx| {
            if controlled {
                set.set_num("value", bpm, cx);
            }
            set.log(
                "on_value_change",
                format!("{bpm} BPM ({:?})", details.reason),
                cx,
            );
        })
        .on_value_committed(move |bpm, _, cx| {
            commit.log("on_value_committed", format!("{bpm} BPM"), cx)
        });

    tempo = match p.index("layout") {
        1 => tempo.child(input(p, ctx)).child(tap(p, ctx, true)),
        2 => tempo.child(input(p, ctx)),
        3 => tempo.child(tap(p, ctx, true)),
        4 => {
            // `tempo-control-pad.tsx`: a sound pad as the only tap trigger.
            let label = p.text("tap_text");
            let pad_id = ctx.id("pad");
            let shown = if controlled {
                format!("{} BPM", p.num("value") as i64)
            } else {
                "— BPM (uncontrolled)".to_string()
            };
            tempo
                .w(px(144.0))
                .flex_col()
                .child(
                    div()
                        .font_family(cx.theme().font_mono.clone())
                        .text_size(px(18.0))
                        .line_height(px(28.0))
                        .child(shown),
                )
                .child(
                    tap(p, ctx, false)
                        .child(SoundPadLabel::new().child(if label.is_empty() {
                            "Tap".into()
                        } else {
                            label
                        }))
                        .render(move |props, _, _| {
                            let tap = props.tap.clone();
                            SoundPad::new(pad_id)
                                .disabled(props.disabled)
                                .on_trigger(move |window, cx| tap(window, cx))
                                .w_full()
                                .children(props.children)
                                .into_any_element()
                        }),
                )
        }
        _ => tempo,
    };

    if !p.bool("show_beat") {
        return tempo.into_any_element();
    }
    let beat = if controlled {
        format!(
            "Beat duration: {} ms",
            (60_000.0 / p.num("value").max(1.0)).round() as i64
        )
    } else {
        "Beat duration: (controlled only)".to_string()
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.0))
        .child(tempo)
        .child(
            div()
                .text_size(px(14.0))
                .text_color(cx.theme().muted_foreground)
                .child(beat),
        )
        .into_any_element()
}
