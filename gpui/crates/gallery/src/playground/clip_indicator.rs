//! Playground page for `ClipIndicator`: threshold, hold (and latching), count,
//! controlled state, custom content, the imperative actions, and logging for
//! `on_clipping_change` and click-to-reset.

use audiocn::components::{ClipIndicator, ClipIndicatorActions, LevelMeter};
use audiocn::theme::ActiveTheme;
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window,
    div, px,
};

use super::level_meter::{CLIP_STATES, action_button, action_row, clip_hold, clip_state};
use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "clip-indicator",
    component: "ClipIndicator",
    source: SourceUse::Meter,
    controls,
    render,
};

fn controls() -> Vec<Control> {
    vec![
        Control::choice(
            "data",
            "Data",
            0,
            &[("source (picker)", ""), ("none: actions.report only", "")],
        )
        .group("Detection")
        .no_code()
        .help("Pick Tone / Static at 0 dB in the source panel to clip."),
        Control::num("threshold_db", "Threshold dB", -1.0, -40.0, 6.0, 0.5).group("Detection"),
        Control::num("hold_ms", "Hold ms", 1500.0, 0.0, 10000.0, 50.0).group("Detection"),
        Control::bool("latching", "Latching (hold_ms = ∞)", false)
            .group("Detection")
            .no_code()
            .help("hold_ms(f64::INFINITY): stays lit until clicked or reset."),
        Control::choice("clipping", "Clipping (controlled)", 0, CLIP_STATES)
            .group("Detection")
            .help("Forced: the light shows this instead of what it detected."),
        Control::bool("show_count", "Show count", false).group("Look"),
        Control::choice(
            "content",
            "Content",
            0,
            &[
                ("light (default)", ""),
                ("text child", ""),
                ("light + text", ""),
            ],
        )
        .group("Look")
        .no_code()
        .help("Children replace the light."),
        Control::text("text", "Text", "Clip")
            .group("Look")
            .no_code(),
        Control::bool("meter", "Show a meter beside it", true)
            .group("Look")
            .no_code()
            .help("A LevelMeter on the same source, to see what it reacts to."),
        Control::bool("actions", "Action buttons", true)
            .group("Actions")
            .no_code()
            .help("ClipIndicatorActions report / reset / clipping / count."),
    ]
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, cx: &mut App) -> AnyElement {
    let actions = ClipIndicatorActions::new();
    let source = if p.index("data") == 0 {
        ctx.meter()
    } else {
        None
    };
    let log = ctx.clone();
    let mut clip = ClipIndicator::new(ctx.id("clip"))
        .source(source.clone())
        .threshold_db(p.num("threshold_db"))
        .hold_ms(clip_hold(p, "hold_ms", "latching"))
        .show_count(p.bool("show_count"))
        .actions(actions.clone())
        .on_clipping_change(move |clipping, _, cx| {
            log.log("on_clipping_change", clipping.to_string(), cx)
        });
    if let Some(clipping) = clip_state(p, "clipping") {
        clip = clip.clipping(clipping);
    }
    let text = p.text("text");
    clip = match p.index("content") {
        1 => clip.child(text),
        2 => clip
            .child(div().size(px(8.0)).rounded_full().bg(cx.theme().meter_clip))
            .child(text),
        _ => clip,
    };

    // Click (mouse up) resets; at mouse down the count is still the old one.
    let click_log = ctx.clone();
    let click_actions = actions.clone();
    let indicator = div()
        .flex_shrink_0()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let (clipping, count) = (click_actions.clipping(cx), click_actions.count(cx));
            click_log.log(
                "click → reset",
                format!("was clipping: {clipping}, count: {count}"),
                cx,
            );
        })
        .child(clip);

    let mut row = div()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .gap_3();
    if p.bool("meter") {
        row = row.child(
            div().flex_1().max_w(px(360.0)).child(
                LevelMeter::new(ctx.id("meter"))
                    .config(ctx.config())
                    .source(source),
            ),
        );
    }
    row = row.child(indicator);

    let mut stage = div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(row);
    if p.bool("actions") {
        let (a1, a2, a3, a4) = (
            actions.clone(),
            actions.clone(),
            actions.clone(),
            actions.clone(),
        );
        let (c1, c2, c3, c4) = (ctx.clone(), ctx.clone(), ctx.clone(), ctx.clone());
        stage = stage.child(action_row(vec![
            action_button(ctx.id("report-0"), "report 0 dB", move |window, cx| {
                a1.report(0.0, window, cx);
                c1.log("actions.report", "0 dB", cx);
            }),
            action_button(ctx.id("report-20"), "report −20 dB", move |window, cx| {
                a2.report(-20.0, window, cx);
                c2.log("actions.report", "−20 dB", cx);
            }),
            action_button(ctx.id("reset"), "reset", move |window, cx| {
                let count = a3.count(cx);
                a3.reset(window, cx);
                c3.log("actions.reset", format!("count was {count}"), cx);
            }),
            action_button(ctx.id("read"), "read state", move |_, cx| {
                let (clipping, count) = (a4.clipping(cx), a4.count(cx));
                c4.log(
                    "actions.read",
                    format!("clipping {clipping}, count {count}"),
                    cx,
                );
            }),
        ]));
    }
    stage.into_any_element()
}
