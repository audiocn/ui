//! Port of `components/ui/db-scale.tsx`: tick marks and labels for a decibel
//! range, shared by meters and faders.
//!
//! The React scale lays ticks out as absolutely positioned DOM nodes and hides
//! labels that collide after layout (`thinDbScaleLabels`). Here the scale is a
//! box with the same size and text style, and a canvas that places each tick
//! exactly as the CSS does, measures its label and thins them on every paint,
//! so a resize re-thins for free.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Bounds, FontFeatures, Hsla, IntoElement, ParentElement, Pixels, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, TextAlign, Window, canvas, div, fill, point,
    prelude::FluentBuilder, px, size,
};

use crate::audio::decibels::{DEFAULT_MAX_DB, DEFAULT_MIN_DB, FormatDbOptions, format_db};
use crate::audio::taper::{AnyTaper, TaperInput, resolve_taper};
use crate::audio::types::{Orientation, Taper};
use crate::config::AudioConfig;
use crate::theme::ActiveTheme;

const DEFAULT_TICKS: [f32; 12] = [
    12.0, 6.0, 0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0, -72.0, -90.0,
];
const EDGE: f32 = 0.02;
/// Space kept between two labels, in pixels.
const LABEL_GAP: f32 = 3.0;
const MAJOR_STEP: f32 = 12.0;
/// `text-[0.625rem] leading-none`.
const LABEL_SIZE: f32 = 10.0;
/// `gap-0.5` between a mark and its label.
const MARK_GAP: f32 = 2.0;

/// Which side of the tick marks the labels sit on (`side`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DbScaleSide {
    Start,
    #[default]
    End,
}

/// Formats a tick value into its label.
pub type DbScaleFormat = Rc<dyn Fn(f32) -> SharedString>;

/// The default tick values inside a range (`DEFAULT_TICKS` filtered).
pub fn default_db_scale_ticks(min_db: f32, max_db: f32) -> Vec<f32> {
    DEFAULT_TICKS
        .iter()
        .copied()
        .filter(|tick| *tick >= min_db && *tick <= max_db)
        .collect()
}

/// The default label: whole decibels, no unit ("−6", "0", "+6").
pub fn default_db_scale_format(db: f32) -> SharedString {
    format_db(
        db,
        FormatDbOptions {
            decimals: 0,
            unit: false,
            ..Default::default()
        },
    )
    .into()
}

/// One label laid out along a scale, for [`thin_db_scale_labels`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DbScaleLabelSpan {
    /// The tick value in dB.
    pub value: f32,
    /// Where the label starts along the scale (left, or top when vertical).
    pub start: f32,
    /// Where it ends (right, or bottom when vertical).
    pub end: f32,
}

/// `thinDbScaleLabels`: hides labels that would touch a more important one, so
/// narrow scales stay readable: 0 dB first, then the ends, then multiples of
/// 12, then the rest. Returns one "hidden" flag per span, in input order.
/// Spans with no extent (not laid out) are never hidden.
pub fn thin_db_scale_labels(spans: &[DbScaleLabelSpan]) -> Vec<bool> {
    let mut hidden = vec![false; spans.len()];
    if spans.is_empty() {
        return hidden;
    }
    let top = spans
        .iter()
        .map(|s| s.value)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = spans.iter().map(|s| s.value).fold(f32::INFINITY, f32::min);
    let rank = |value: f32| {
        if value == 0.0 {
            0
        } else if value == top || value == bottom {
            1
        } else if value % MAJOR_STEP == 0.0 {
            2
        } else {
            3
        }
    };
    let mut order: Vec<usize> = (0..spans.len()).collect();
    // Stable, like Array.prototype.sort: rank, then the louder value first.
    order.sort_by(|&a, &b| {
        rank(spans[a].value).cmp(&rank(spans[b].value)).then(
            spans[b]
                .value
                .partial_cmp(&spans[a].value)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    let mut kept: Vec<(f32, f32)> = Vec::new();
    for index in order {
        let DbScaleLabelSpan { start, end, .. } = spans[index];
        // Not laid out, so there is nothing to compare.
        if start == end && start == 0.0 {
            continue;
        }
        let collides = kept
            .iter()
            .any(|&(ks, ke)| start < ke + LABEL_GAP && end > ks - LABEL_GAP);
        if collides {
            hidden[index] = true;
        } else {
            kept.push((start, end));
        }
    }
    hidden
}

/// One tick of a [`DbScale`] (`DbScaleTick`): a mark at `value` and, when the
/// scale shows labels, its label (the formatted value unless [`label`](Self::label) is set).
///
/// ```ignore
/// DbScale::new().orientation(Orientation::Vertical)
///     .child(DbScaleTick::new(0.0).label("Unity"))
///     .child(DbScaleTick::new(-10.0).major(false))
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DbScaleTick {
    value: f32,
    major: bool,
    label: Option<SharedString>,
}

impl DbScaleTick {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            major: true,
            label: None,
        }
    }

    /// Major ticks are longer. Default true.
    pub fn major(mut self, major: bool) -> Self {
        self.major = major;
        self
    }

    /// The label text (React `children`). Default: the scale's `format(value)`.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

/// Tick marks and labels for a decibel range, shared by meters and faders.
///
/// ```ignore
/// DbScale::new()                                         // −60..0, horizontal
/// DbScale::new().min_db(-48.0).max_db(6.0).taper(TaperInput::Audio)
/// DbScale::new().orientation(Orientation::Vertical).side(DbScaleSide::Start)
/// ```
#[derive(IntoElement)]
pub struct DbScale {
    min_db: Option<f32>,
    max_db: Option<f32>,
    ticks: Option<Vec<f32>>,
    taper: TaperInput,
    orientation: Option<Orientation>,
    side: DbScaleSide,
    labels: bool,
    marks: bool,
    format: Option<DbScaleFormat>,
    config: AudioConfig,
    children: Vec<DbScaleTick>,
    style: StyleRefinement,
}

impl Default for DbScale {
    fn default() -> Self {
        Self::new()
    }
}

impl DbScale {
    pub fn new() -> Self {
        Self {
            min_db: None,
            max_db: None,
            ticks: None,
            taper: TaperInput::Linear,
            orientation: None,
            side: DbScaleSide::End,
            labels: true,
            marks: true,
            format: None,
            config: AudioConfig::default(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Bottom of the range. Default −60, or the config's range.
    pub fn min_db(mut self, min_db: f32) -> Self {
        self.min_db = Some(min_db);
        self
    }

    /// Top of the range. Default 0, or the config's range.
    pub fn max_db(mut self, max_db: f32) -> Self {
        self.max_db = Some(max_db);
        self
    }

    /// Tick values. Default: common values inside the range.
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = Some(ticks.into_iter().collect());
        self
    }

    /// Position law, so ticks line up with a fader or meter using the same taper.
    pub fn taper(mut self, taper: impl Into<TaperInput>) -> Self {
        self.taper = taper.into();
        self
    }

    /// Default horizontal, or the config's orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Which side of the tick marks the labels sit on. Default `End`.
    pub fn side(mut self, side: DbScaleSide) -> Self {
        self.side = side;
        self
    }

    /// Show labels. Default true.
    pub fn labels(mut self, labels: bool) -> Self {
        self.labels = labels;
        self
    }

    /// Draw the tick marks. Default true. (GPUI extra: React hides them with
    /// `[&_[data-slot=db-scale-mark]]:hidden`, as the LCD meter does.)
    pub fn marks(mut self, marks: bool) -> Self {
        self.marks = marks;
        self
    }

    /// Label text for a tick value. Default: whole dB without a unit.
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// Range and orientation from a surrounding meter, strip or mixer.
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    /// A custom tick (React children). Custom ticks replace the default ones.
    pub fn child(mut self, tick: DbScaleTick) -> Self {
        self.children.push(tick);
        self
    }

    /// Custom ticks (React children).
    pub fn children(mut self, ticks: impl IntoIterator<Item = DbScaleTick>) -> Self {
        self.children.extend(ticks);
        self
    }
}

impl Styled for DbScale {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// `font-variant-numeric: tabular-nums`.
pub(crate) fn tabular_nums() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Snaps a length to device pixels, like the browser does for backgrounds.
fn snap(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

struct TickLayout {
    mark: Option<Bounds<Pixels>>,
    label: Option<(gpui::ShapedLine, gpui::Point<Pixels>)>,
}

#[allow(clippy::too_many_arguments)]
fn layout_ticks(
    bounds: Bounds<Pixels>,
    ticks: &[DbScaleTick],
    taper: &AnyTaper,
    horizontal: bool,
    side: DbScaleSide,
    labels: bool,
    marks: bool,
    format: &DbScaleFormat,
    window: &mut Window,
) -> Vec<TickLayout> {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let line_height = text_style.line_height_in_pixels(window.rem_size());
    let scale = window.scale_factor();
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let reversed = side == DbScaleSide::Start;
    let lh = f32::from(line_height);

    let mut layouts = Vec::with_capacity(ticks.len());
    let mut spans = Vec::with_capacity(ticks.len());
    for tick in ticks {
        let position = taper.to_position(tick.value);
        let (mark_long, mark_short) = if tick.major { (6.0, 1.0) } else { (4.0, 1.0) };
        let shaped = labels.then(|| {
            let text = tick.label.clone().unwrap_or_else(|| format(tick.value));
            let run = text_style.to_run(text.len());
            window
                .text_system()
                .shape_line(text, font_size, &[run], None)
        });
        let label_w = shaped.as_ref().map_or(0.0, |line| f32::from(line.width));

        if horizontal {
            // A column: the mark, then the label (reversed: label above).
            let box_w = if labels {
                label_w.max(mark_short)
            } else {
                mark_short
            };
            let x = position * w;
            let left = if position < EDGE {
                x
            } else if position > 1.0 - EDGE {
                x - box_w
            } else {
                x - box_w / 2.0
            };
            let mark_x = left + (box_w - mark_short) / 2.0;
            let (mark_y, label_y) = if reversed {
                (h - mark_long, h - mark_long - MARK_GAP - lh)
            } else {
                (0.0, mark_long + MARK_GAP)
            };
            let label_x = left + (box_w - label_w) / 2.0;
            spans.push(DbScaleLabelSpan {
                value: tick.value,
                start: label_x,
                end: label_x + label_w,
            });
            layouts.push(TickLayout {
                mark: marks.then(|| {
                    Bounds::new(
                        point(px(ox + snap(mark_x, scale)), px(oy + mark_y)),
                        size(px(mark_short), px(mark_long)),
                    )
                }),
                label: shaped.map(|line| (line, point(px(ox + label_x), px(oy + label_y)))),
            });
        } else {
            // A row: the mark, then the label filling the rest, right-aligned.
            let box_h = if labels {
                lh.max(mark_short)
            } else {
                mark_short
            };
            let y_bottom = h - position * h;
            let top = if position < EDGE {
                y_bottom - box_h
            } else if position > 1.0 - EDGE {
                y_bottom
            } else {
                y_bottom - box_h / 2.0
            };
            let mark_y = top + (box_h - mark_short) / 2.0;
            let (mark_x, label_end) = if reversed {
                (w - mark_long, w - mark_long - MARK_GAP)
            } else {
                (0.0, w)
            };
            let label_y = top + (box_h - lh) / 2.0;
            spans.push(DbScaleLabelSpan {
                value: tick.value,
                start: label_y,
                end: label_y + lh,
            });
            layouts.push(TickLayout {
                mark: marks.then(|| {
                    Bounds::new(
                        point(px(ox + mark_x), px(oy + snap(mark_y, scale))),
                        size(px(mark_long), px(mark_short)),
                    )
                }),
                label: shaped
                    .map(|line| (line, point(px(ox + label_end - label_w), px(oy + label_y)))),
            });
        }
    }

    if labels {
        for (layout, hidden) in layouts.iter_mut().zip(thin_db_scale_labels(&spans)) {
            if hidden {
                layout.label = None;
            }
        }
    }
    layouts
}

impl RenderOnce for DbScale {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let min_db = self.min_db.or(self.config.min_db).unwrap_or(DEFAULT_MIN_DB);
        let max_db = self.max_db.or(self.config.max_db).unwrap_or(DEFAULT_MAX_DB);
        let orientation = self
            .orientation
            .or(self.config.orientation)
            .unwrap_or_default();
        let horizontal = orientation == Orientation::Horizontal;
        let taper = resolve_taper(&self.taper, min_db, max_db);
        let format: DbScaleFormat = self
            .format
            .clone()
            .unwrap_or_else(|| Rc::new(default_db_scale_format));
        let ticks: Vec<DbScaleTick> = if self.children.is_empty() {
            self.ticks
                .clone()
                .unwrap_or_else(|| default_db_scale_ticks(min_db, max_db))
                .into_iter()
                .map(DbScaleTick::new)
                .collect()
        } else {
            self.children
        };
        let (side, labels, marks) = (self.side, self.labels, self.marks);
        let mark_color: Hsla = theme.border;

        let mut root = div()
            .relative()
            .flex_shrink_0()
            .text_size(px(LABEL_SIZE))
            .line_height(px(LABEL_SIZE))
            .font_features(tabular_nums())
            .text_color(theme.muted_foreground)
            .when(horizontal, |el| el.h(px(16.0)).w_full())
            .when(!horizontal, |el| el.h_full().w(px(28.0)));
        root.style().refine(&self.style);

        root.child(
            canvas(
                move |bounds, window, _| {
                    layout_ticks(
                        bounds, &ticks, &taper, horizontal, side, labels, marks, &format, window,
                    )
                },
                move |_, layouts, window, cx| {
                    let line_height = window.text_style().line_height_in_pixels(window.rem_size());
                    for layout in layouts {
                        if let Some(mark) = layout.mark {
                            window.paint_quad(fill(mark, mark_color));
                        }
                        if let Some((line, origin)) = layout.label {
                            line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                                .ok();
                        }
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_common_ticks_inside_the_range() {
        let labels: Vec<String> = default_db_scale_ticks(-24.0, 0.0)
            .into_iter()
            .map(|db| default_db_scale_format(db).to_string())
            .collect();
        assert_eq!(labels, ["0", "−6", "−12", "−18", "−24"]);
    }

    #[test]
    fn default_ticks_cover_the_default_range() {
        assert_eq!(
            default_db_scale_ticks(DEFAULT_MIN_DB, DEFAULT_MAX_DB),
            [0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0]
        );
        assert_eq!(default_db_scale_format(6.0).as_ref(), "+6");
    }

    #[test]
    fn hides_labels_that_would_touch_0_db_or_the_ends() {
        let span = |value: f32, start: f32, end: f32| DbScaleLabelSpan { value, start, end };
        let spans = [
            span(0.0, 100.0, 110.0),
            span(-6.0, 92.0, 102.0),
            span(-12.0, 70.0, 80.0),
            span(-54.0, 8.0, 18.0),
            span(-60.0, 0.0, 10.0),
        ];
        let hidden: Vec<f32> = thin_db_scale_labels(&spans)
            .into_iter()
            .zip(spans)
            .filter_map(|(hidden, s)| hidden.then_some(s.value))
            .collect();
        assert_eq!(hidden, [-6.0, -54.0]);
    }

    #[test]
    fn skips_labels_that_are_not_laid_out() {
        let spans = [
            DbScaleLabelSpan {
                value: 0.0,
                start: 0.0,
                end: 0.0,
            },
            DbScaleLabelSpan {
                value: -6.0,
                start: 0.0,
                end: 0.0,
            },
        ];
        assert_eq!(thin_db_scale_labels(&spans), [false, false]);
    }
}
