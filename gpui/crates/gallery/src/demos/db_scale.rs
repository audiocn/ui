//! Gallery page for `db-scale`. One variant per docs example:
//! `components/examples/db-scale-demo.tsx`, `db-scale-vertical.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use audiocn::audio::taper::TaperInput;
use audiocn::{DbScale, DbScaleSide, DbScaleTick, Orientation};

use super::Demo;

pub const DEMO: Demo = Demo {
    slug: "db-scale",
    title: "dB Scale",
    description: "Tick marks and labels for a decibel range, shared by meters and faders.",
    variants: &["default", "vertical"],
    render,
};

fn render(variant: &str, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(),
        "vertical" => vertical(),
        _ => return None,
    })
}

/// `db-scale-demo.tsx`.
fn demo() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(448.0))
        .gap(px(32.0))
        .child(DbScale::new())
        .child(
            DbScale::new()
                .max_db(6.0)
                .min_db(-48.0)
                .taper(TaperInput::Audio),
        )
        .into_any_element()
}

/// `db-scale-vertical.tsx`.
fn vertical() -> AnyElement {
    div()
        .flex()
        .h(px(224.0))
        .gap(px(40.0))
        .child(DbScale::new().orientation(Orientation::Vertical))
        .child(
            DbScale::new()
                .max_db(12.0)
                .min_db(-60.0)
                .orientation(Orientation::Vertical)
                .side(DbScaleSide::Start)
                .child(DbScaleTick::new(12.0))
                .child(DbScaleTick::new(0.0).label("Unity"))
                .child(DbScaleTick::new(-10.0).major(false))
                .child(DbScaleTick::new(-20.0))
                .child(DbScaleTick::new(-30.0).major(false))
                .child(DbScaleTick::new(-60.0).label("−∞")),
        )
        .into_any_element()
}
