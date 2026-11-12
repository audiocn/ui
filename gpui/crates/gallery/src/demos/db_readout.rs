//! Gallery page for `db-readout`. One variant per docs example:
//! `components/examples/db-readout-demo.tsx`, `db-readout-zones.tsx`.

use gpui::{AnyElement, App, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::signal::demo_signal::{DemoSignalKind, DemoSignalOptions, use_demo_signal};
use audiocn::theme::ActiveTheme;
use audiocn::{DbReadout, DbReadoutMeasure, MeterZoneName};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "db-readout",
    title: "dB Readout",
    description: "A numeric level label that updates at a readable rate and never shifts the layout.",
    section: Section::Components,
    variants: &["default", "zones"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        "zones" => zones(window, cx),
        _ => return None,
    })
}

/// `db-readout-demo.tsx`: a two-column grid (`grid-cols-[auto_auto]
/// gap-x-6 gap-y-2 text-sm`), as two columns of equal-height rows.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        id("db-readout", "default/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Speech),
        window,
        cx,
    );
    let muted = cx.theme().muted_foreground;
    let label = |text: &'static str| div().text_color(muted).child(text);
    let column = || div().flex().flex_col().gap(px(8.0));
    div()
        .flex()
        .items_center()
        .gap(px(24.0))
        .text_size(px(14.0))
        .line_height(px(20.0))
        .child(
            column()
                .child(label("Peak"))
                .child(label("RMS"))
                .child(label("Peak, held 1 s"))
                .child(label("Value")),
        )
        .child(
            column()
                .child(
                    DbReadout::new(id("db-readout", "default/peak")).source(signal.meter.clone()),
                )
                .child(
                    DbReadout::new(id("db-readout", "default/rms"))
                        .measure(DbReadoutMeasure::Rms)
                        .source(signal.meter.clone()),
                )
                .child(
                    DbReadout::new(id("db-readout", "default/held"))
                        .hold_ms(1000.0)
                        .source(signal.meter),
                )
                .child(DbReadout::new(id("db-readout", "default/value")).value(-6.0)),
        )
        .into_any_element()
}

/// `db-readout-zones.tsx`.
fn zones(window: &mut Window, cx: &mut App) -> AnyElement {
    let signal = use_demo_signal(
        id("db-readout", "zones/signal"),
        DemoSignalOptions::kind(DemoSignalKind::Music).with_seed(6),
        window,
        cx,
    );
    let theme = cx.theme();
    let (muted, muted_fg) = (theme.muted, theme.muted_foreground);
    let (clip, clip_fg, warn_fg) = (
        theme.meter_clip,
        theme.meter_clip_foreground,
        theme.meter_warn_foreground,
    );
    let radius = theme.radius_md();
    DbReadout::new(id("db-readout", "zones"))
        .text_size(px(24.0))
        .line_height(px(32.0))
        .font_weight(FontWeight::SEMIBOLD)
        .hold_ms(500.0)
        .source(signal.meter)
        .decorate(move |display, el| {
            let color = match display.zone {
                MeterZoneName::Clip => Some(clip_fg),
                MeterZoneName::Warn => Some(warn_fg),
                MeterZoneName::Ok if display.silent => Some(muted_fg),
                MeterZoneName::Ok => None,
            };
            let el = match color {
                Some(color) => el.text_color(color),
                None => el,
            };
            // `bg-muted/40 has-data-[zone=clip]:bg-meter-clip/15 rounded-md px-2 py-1`.
            div()
                .rounded(radius)
                .px(px(8.0))
                .py(px(4.0))
                .bg(if display.zone == MeterZoneName::Clip {
                    clip.opacity(0.15)
                } else {
                    muted.opacity(0.4)
                })
                .child(el)
                .into_any_element()
        })
        .into_any_element()
}
