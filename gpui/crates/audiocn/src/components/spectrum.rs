//! Port of `components/ui/spectrum.tsx`.
//!
//! A frequency spectrum analyser with axes, a grid and peak hold. React
//! shares settings with its parts through context; here [`Spectrum`] owns its
//! parts ([`SpectrumLevelAxis`], [`SpectrumCanvas`], [`SpectrumFrequencyAxis`])
//! as children and places them on the same 2×2 grid
//! (`grid-cols-[auto_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] gap-1`).

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ElementId, FontFeatures, Hsla, InteractiveElement, IntoElement,
    ParentElement, PathBuilder, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, canvas, div, fill, point,
    prelude::FluentBuilder, px, relative, size,
};

use crate::audio::decibels::clamp;
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::taper::{AnyTaper, linear_taper, log_taper};
use crate::audio::types::{Taper, VisualFrame};
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::ActiveTheme;

const PEAK_RELEASE_PER_FRAME: f64 = 0.985;
/// The frame length `PEAK_RELEASE_PER_FRAME` is tuned for.
const FRAME_MS: f64 = 1000.0 / 60.0;
const REDUCED_MOTION_INTERVAL_MS: f64 = 250.0;
const LEVEL_TICK_DB: f32 = 12.0;
const DEFAULT_FREQUENCY_TICKS: [f32; 3] = [100.0, 1000.0, 10_000.0];
/// `gap-1`.
const GRID_GAP: f32 = 4.0;
/// `w-7`.
const LEVEL_AXIS_WIDTH: f32 = 28.0;
/// `h-4`.
const FREQUENCY_AXIS_HEIGHT: f32 = 16.0;
/// `h-40`.
const DEFAULT_HEIGHT: f32 = 160.0;
/// `text-[0.625rem]`.
const AXIS_TEXT_SIZE: f32 = 10.0;
/// The inherited `line-height: 1.5`.
const AXIS_LINE_HEIGHT: f32 = 1.5;

/// How the bands are drawn (`variant`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpectrumVariant {
    /// One bar per band. The default.
    #[default]
    Bars,
    /// A line through the band tops.
    Line,
    /// The line with a translucent fill under it.
    Area,
}

/// Frequency axis law (`scale`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpectrumScale {
    /// Logarithmic. The default.
    #[default]
    Log,
    Linear,
}

/// Formats a tick label.
pub type SpectrumFormat = Rc<dyn Fn(f32) -> SharedString>;

/// `formatHz`: `100`, `1k`, `10k`, `2.5k`.
pub fn format_hz(hz: f32) -> SharedString {
    if hz >= 1000.0 {
        format!("{}k", hz as f64 / 1000.0).into()
    } else {
        format!("{hz}").into()
    }
}

/// `formatLevel`: the dB value rounded like `Math.round`.
pub fn format_level(db: f32) -> SharedString {
    // `+ 0.0` turns −0 into 0, like `String(-0)`.
    format!("{}", (db as f64 + 0.5).floor() + 0.0).into()
}

/// The default level ticks: every 12 dB from `max_db` down, inside the range.
pub fn default_level_ticks(min_db: f32, max_db: f32) -> Vec<f32> {
    let count = ((max_db - min_db) / LEVEL_TICK_DB).floor() as i64 + 1;
    (0..count.max(0))
        .map(|index| max_db - index as f32 * LEVEL_TICK_DB)
        .collect()
}

/// Colour overrides: the `--spectrum`, `--spectrum-peak` and `--spectrum-grid` variables.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SpectrumColors {
    line: Option<Hsla>,
    peak: Option<Hsla>,
    grid: Option<Hsla>,
}

impl SpectrumColors {
    fn or(self, fallback: Self) -> Self {
        Self {
            line: self.line.or(fallback.line),
            peak: self.peak.or(fallback.peak),
            grid: self.grid.or(fallback.grid),
        }
    }
}

/// What every part reads from the [`Spectrum`] (the React context value).
#[derive(Clone)]
struct SpectrumSettings {
    min_db: f32,
    max_db: f32,
    min_hz: f32,
    max_hz: f32,
    frequency_taper: AnyTaper,
    variant: SpectrumVariant,
    peak_hold: bool,
    grid: bool,
}

impl PartialEq for SpectrumSettings {
    fn eq(&self, other: &Self) -> bool {
        self.min_db == other.min_db
            && self.max_db == other.max_db
            && self.min_hz == other.min_hz
            && self.max_hz == other.max_hz
            && self.frequency_taper == other.frequency_taper
            && self.variant == other.variant
            && self.peak_hold == other.peak_hold
            && self.grid == other.grid
    }
}

/// One canvas's paint state: what it last drew, its held peaks, and whether
/// it needs another paint (a new frame, or peaks still falling).
#[derive(Clone, Debug, Default)]
struct SpectrumPainter {
    peaks: Vec<f32>,
    last_paint_ms: Option<f64>,
    /// Peaks are still falling.
    dirty: bool,
    painted_version: Option<u64>,
    /// The bands drawn by the last paint.
    bands: Option<Vec<f32>>,
}

impl SpectrumPainter {
    fn new() -> Self {
        Self {
            dirty: true,
            ..Self::default()
        }
    }

    /// Paints when there is something new (`shouldPaint` + `draw` without the
    /// drawing). Returns whether it painted.
    fn update(
        &mut self,
        now_ms: f64,
        version: u64,
        bands: Option<&[f32]>,
        peak_hold: bool,
        reduced_motion: bool,
    ) -> bool {
        let stale = self.dirty || self.painted_version != Some(version);
        if !stale {
            return false;
        }
        if reduced_motion
            && self
                .last_paint_ms
                .is_some_and(|last| now_ms - last < REDUCED_MOTION_INTERVAL_MS)
        {
            return false;
        }
        let elapsed_ms = self.last_paint_ms.map_or(0.0, |last| now_ms - last);
        self.last_paint_ms = Some(now_ms);
        self.dirty = false;
        self.painted_version = Some(version);
        self.bands = bands.filter(|bands| !bands.is_empty()).map(<[f32]>::to_vec);
        if let (Some(bands), true) = (&self.bands, peak_hold) {
            if self.peaks.len() != bands.len() {
                self.peaks = vec![0.0; bands.len()];
            }
            // Decay by elapsed time, not per paint, so peaks fall at the same
            // speed at 60 Hz, at 120 Hz and with reduced motion.
            let release = PEAK_RELEASE_PER_FRAME.powf(elapsed_ms / FRAME_MS) as f32;
            self.dirty = hold_peaks(bands, &mut self.peaks, release);
        }
        true
    }
}

/// Updates held peaks and reports whether any are still falling (`drawPeaks`).
fn hold_peaks(bands: &[f32], peaks: &mut [f32], release: f32) -> bool {
    let mut falling = false;
    for (band, peak) in bands.iter().zip(peaks.iter_mut()) {
        let level = clamp(*band, 0.0, 1.0);
        let held = level.max(*peak * release);
        *peak = held;
        if held > level + 0.001 {
            falling = true;
        }
    }
    falling
}

fn paint_rect(window: &mut Window, origin: gpui::Point<Pixels>, rect: [f32; 4], color: Hsla) {
    let [x, y, w, h] = rect;
    window.paint_quad(fill(
        Bounds::new(
            point(origin.x + px(x), origin.y + px(y)),
            size(px(w), px(h)),
        ),
        color,
    ));
}

/// `drawGrid`: three level lines and the default frequency ticks, 1px wide.
fn paint_grid(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    settings: &SpectrumSettings,
    color: Hsla,
) {
    let (width, height) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
    let span = settings.max_db - settings.min_db;
    let mut rows: Vec<f32> = [
        settings.max_db,
        settings.max_db - span / 3.0,
        settings.max_db - span * 2.0 / 3.0,
    ]
    .iter()
    .map(|db| ((1.0 - (db - settings.min_db) / span) * height).round())
    .collect();
    for &y in &rows {
        paint_rect(window, bounds.origin, [0.0, y, width, 1.0], color);
    }
    // One stroked path in canvas: skip the crossings so a translucent grid
    // colour does not double up where lines meet.
    rows.sort_by(f32::total_cmp);
    for hz in DEFAULT_FREQUENCY_TICKS {
        if hz > settings.min_hz && hz < settings.max_hz {
            let x = (settings.frequency_taper.to_position(hz) * width).round();
            let mut top = 0.0;
            for &y in &rows {
                if y > top {
                    paint_rect(window, bounds.origin, [x, top, 1.0, y - top], color);
                }
                top = top.max(y + 1.0);
            }
            if height > top {
                paint_rect(window, bounds.origin, [x, top, 1.0, height - top], color);
            }
        }
    }
}

/// `drawBars`.
fn paint_bars(window: &mut Window, bounds: Bounds<Pixels>, bands: &[f32], color: Hsla) {
    let (width, height) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
    let slot = width / bands.len() as f32;
    let gap = (slot * 0.25).min(2.0);
    for (index, band) in bands.iter().enumerate() {
        let bar_height = clamp(*band, 0.0, 1.0) * height;
        paint_rect(
            window,
            bounds.origin,
            [
                index as f32 * slot + gap / 2.0,
                height - bar_height,
                (slot - gap).max(1.0),
                bar_height,
            ],
            color,
        );
    }
}

/// `drawCurve`: a 1.5px line through the band tops, filled at 30% for `area`.
fn paint_curve(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    bands: &[f32],
    filled: bool,
    color: Hsla,
) {
    let (width, height) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
    let slot = width / bands.len() as f32;
    let at = |x: f32, y: f32| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    let points: Vec<_> = bands
        .iter()
        .enumerate()
        .map(|(index, band)| {
            at(
                (index as f32 + 0.5) * slot,
                height - clamp(*band, 0.0, 1.0) * height,
            )
        })
        .collect();
    if filled {
        let mut area = PathBuilder::fill();
        area.move_to(points[0]);
        for &p in &points[1..] {
            area.line_to(p);
        }
        area.line_to(at((bands.len() as f32 - 0.5) * slot, height));
        area.line_to(at(0.5 * slot, height));
        area.close();
        if let Ok(path) = area.build() {
            window.paint_path(path, color.opacity(0.3));
        }
    }
    let mut line = PathBuilder::stroke(px(1.5));
    line.move_to(points[0]);
    for &p in &points[1..] {
        line.line_to(p);
    }
    if filled {
        // The canvas strokes the closed outline too.
        line.line_to(at((bands.len() as f32 - 0.5) * slot, height));
        line.line_to(at(0.5 * slot, height));
        line.close();
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }
}

/// `drawPeaks`: a 2px marker at each held peak.
fn paint_peaks(window: &mut Window, bounds: Bounds<Pixels>, peaks: &[f32], color: Hsla) {
    let (width, height) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
    let slot = width / peaks.len() as f32;
    for (index, held) in peaks.iter().enumerate() {
        paint_rect(
            window,
            bounds.origin,
            [
                index as f32 * slot,
                height - held * height - 1.0,
                (slot - 1.0).max(1.0),
                2.0,
            ],
            color,
        );
    }
}

/// The plot: grid, bands and held peaks, painted from the spectrum's source.
/// Each canvas keeps its own state, so two in one [`Spectrum`] both paint.
///
/// ```ignore
/// Spectrum::new("spectrum").source(visual).child(SpectrumCanvas::new())
/// ```
#[derive(Default)]
pub struct SpectrumCanvas {
    colors: SpectrumColors,
    style: StyleRefinement,
}

impl SpectrumCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    /// `--spectrum` for this canvas: the bars or line. Default: the spectrum's.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.line = Some(color.into());
        self
    }

    /// `--spectrum-peak` for this canvas. Default: the spectrum's.
    pub fn peak_color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.peak = Some(color.into());
        self
    }

    /// `--spectrum-grid` for this canvas. Default: the spectrum's.
    pub fn grid_color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.grid = Some(color.into());
        self
    }
}

impl Styled for SpectrumCanvas {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The frequency labels under the plot.
///
/// ```ignore
/// SpectrumFrequencyAxis::new().ticks([50.0, 500.0, 5000.0])
/// ```
pub struct SpectrumFrequencyAxis {
    ticks: Vec<f32>,
    format: SpectrumFormat,
    style: StyleRefinement,
}

impl Default for SpectrumFrequencyAxis {
    fn default() -> Self {
        Self {
            ticks: DEFAULT_FREQUENCY_TICKS.to_vec(),
            format: Rc::new(format_hz),
            style: StyleRefinement::default(),
        }
    }
}

impl SpectrumFrequencyAxis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tick frequencies in Hz. Default 100 Hz, 1 kHz and 10 kHz.
    pub fn ticks(mut self, ticks: impl Into<Vec<f32>>) -> Self {
        self.ticks = ticks.into();
        self
    }

    /// Label format. Default [`format_hz`].
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
}

impl Styled for SpectrumFrequencyAxis {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The dB labels left of the plot.
///
/// ```ignore
/// SpectrumLevelAxis::new().ticks([-30.0, -60.0, -90.0])
/// ```
pub struct SpectrumLevelAxis {
    ticks: Option<Vec<f32>>,
    format: SpectrumFormat,
    style: StyleRefinement,
}

impl Default for SpectrumLevelAxis {
    fn default() -> Self {
        Self {
            ticks: None,
            format: Rc::new(format_level),
            style: StyleRefinement::default(),
        }
    }
}

impl SpectrumLevelAxis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tick levels in dB. Default: every 12 dB inside the range.
    pub fn ticks(mut self, ticks: impl Into<Vec<f32>>) -> Self {
        self.ticks = Some(ticks.into());
        self
    }

    /// Label format. Default [`format_level`].
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
}

impl Styled for SpectrumLevelAxis {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A child of [`Spectrum`].
pub enum SpectrumPart {
    LevelAxis(SpectrumLevelAxis),
    Canvas(SpectrumCanvas),
    FrequencyAxis(SpectrumFrequencyAxis),
}

impl From<SpectrumLevelAxis> for SpectrumPart {
    fn from(part: SpectrumLevelAxis) -> Self {
        Self::LevelAxis(part)
    }
}

impl From<SpectrumCanvas> for SpectrumPart {
    fn from(part: SpectrumCanvas) -> Self {
        Self::Canvas(part)
    }
}

impl From<SpectrumFrequencyAxis> for SpectrumPart {
    fn from(part: SpectrumFrequencyAxis) -> Self {
        Self::FrequencyAxis(part)
    }
}

/// A frequency spectrum analyser with axes, a grid and peak hold.
///
/// Without children it renders the level axis, the canvas and the frequency
/// axis. Set `min_hz`, `max_hz`, `min_db` and `max_db` to match the analyser
/// that produced the bands.
///
/// ```ignore
/// let signal = use_demo_signal("music", DemoSignalOptions::kind(DemoSignalKind::Music).with_bands(48), window, cx);
/// Spectrum::new("spectrum")
///     .source(signal.visual)
///     .min_db(-60.0)
///     .max_db(0.0)
///     .peak_hold(true)
///     .max_w(px(512.0))
/// ```
#[derive(IntoElement)]
pub struct Spectrum {
    id: ElementId,
    source: Option<FrameSource<VisualFrame>>,
    variant: SpectrumVariant,
    min_db: f32,
    max_db: f32,
    min_hz: f32,
    max_hz: f32,
    scale: SpectrumScale,
    peak_hold: bool,
    grid: bool,
    colors: SpectrumColors,
    parts: Vec<SpectrumPart>,
    aria_label: SharedString,
    style: StyleRefinement,
}

impl Spectrum {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            source: None,
            variant: SpectrumVariant::Bars,
            min_db: -100.0,
            max_db: -30.0,
            min_hz: 40.0,
            max_hz: 16_000.0,
            scale: SpectrumScale::Log,
            peak_hold: false,
            grid: true,
            colors: SpectrumColors::default(),
            parts: Vec::new(),
            aria_label: "Frequency spectrum".into(),
            style: StyleRefinement::default(),
        }
    }

    /// A visual source; the spectrum draws its frequency bands.
    pub fn source(mut self, source: FrameSource<VisualFrame>) -> Self {
        self.source = Some(source);
        self
    }

    /// Like [`source`](Self::source), `None` detaches.
    pub fn source_opt(mut self, source: Option<FrameSource<VisualFrame>>) -> Self {
        self.source = source;
        self
    }

    /// Default `Bars`.
    pub fn variant(mut self, variant: SpectrumVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Bottom of the level range the bands cover. Default −100.
    pub fn min_db(mut self, min_db: f32) -> Self {
        self.min_db = min_db;
        self
    }

    /// Top of the level range the bands cover. Default −30.
    pub fn max_db(mut self, max_db: f32) -> Self {
        self.max_db = max_db;
        self
    }

    /// Bottom of the frequency range the bands cover. Default 40.
    pub fn min_hz(mut self, min_hz: f32) -> Self {
        self.min_hz = min_hz;
        self
    }

    /// Top of the frequency range the bands cover. Default 16000.
    pub fn max_hz(mut self, max_hz: f32) -> Self {
        self.max_hz = max_hz;
        self
    }

    /// Frequency axis law. Default `Log`.
    pub fn scale(mut self, scale: SpectrumScale) -> Self {
        self.scale = scale;
        self
    }

    /// Keep falling peak markers. Default false.
    pub fn peak_hold(mut self, peak_hold: bool) -> Self {
        self.peak_hold = peak_hold;
        self
    }

    /// Draw grid lines. Default true.
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// `--spectrum`: the bars or line. Default `primary`.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.line = Some(color.into());
        self
    }

    /// `--spectrum-peak`: the peak markers. Default `foreground`.
    pub fn peak_color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.peak = Some(color.into());
        self
    }

    /// `--spectrum-grid`: the grid lines. Default `border`.
    pub fn grid_color(mut self, color: impl Into<Hsla>) -> Self {
        self.colors.grid = Some(color.into());
        self
    }

    /// Adds a part. Without parts the spectrum renders all three.
    pub fn child(mut self, part: impl Into<SpectrumPart>) -> Self {
        self.parts.push(part.into());
        self
    }

    /// Adds parts.
    pub fn children(mut self, parts: impl IntoIterator<Item = impl Into<SpectrumPart>>) -> Self {
        self.parts.extend(parts.into_iter().map(Into::into));
        self
    }

    /// `aria-label`. Default "Frequency spectrum".
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = label.into();
        self
    }
}

impl Styled for Spectrum {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn tabular_nums() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// The axis text style: `text-muted-foreground text-[0.625rem] tabular-nums`.
fn axis_base(cx: &App) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .text_color(cx.theme().muted_foreground)
        .text_size(px(AXIS_TEXT_SIZE))
        .line_height(relative(AXIS_LINE_HEIGHT))
        .font_features(tabular_nums())
}

fn level_axis_labels(
    axis: &SpectrumLevelAxis,
    settings: &SpectrumSettings,
) -> impl Iterator<Item = AnyElement> {
    let (min_db, max_db) = (settings.min_db, settings.max_db);
    let values = axis
        .ticks
        .clone()
        .unwrap_or_else(|| default_level_ticks(min_db, max_db));
    let format = axis.format.clone();
    values.into_iter().map(move |db| {
        // `absolute right-0 bottom-(--tick-position) translate-y-1/2`: a
        // zero-height row at the tick, the label centred on it.
        div()
            .absolute()
            .right_0()
            .bottom(relative((db - min_db) / (max_db - min_db)))
            .h(px(0.0))
            .flex()
            .items_center()
            .child(div().flex_none().whitespace_nowrap().child(format(db)))
            .into_any_element()
    })
}

fn frequency_axis_labels(
    axis: &SpectrumFrequencyAxis,
    settings: &SpectrumSettings,
) -> impl Iterator<Item = AnyElement> {
    let taper = settings.frequency_taper.clone();
    let (min_hz, max_hz) = (settings.min_hz, settings.max_hz);
    let format = axis.format.clone();
    axis.ticks
        .clone()
        .into_iter()
        .filter(move |hz| *hz >= min_hz && *hz <= max_hz)
        .map(move |hz| {
            // `absolute top-0 left-(--tick-position) -translate-x-1/2`.
            div()
                .absolute()
                .top_0()
                .left(relative(taper.to_position(hz)))
                .w(px(0.0))
                .flex()
                .justify_center()
                .child(div().flex_none().whitespace_nowrap().child(format(hz)))
                .into_any_element()
        })
}

impl RenderOnce for Spectrum {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let theme_colors = SpectrumColors {
            line: Some(theme.primary),
            peak: Some(theme.foreground),
            grid: Some(theme.border),
        };
        let settings = SpectrumSettings {
            min_db: self.min_db,
            max_db: self.max_db,
            min_hz: self.min_hz,
            max_hz: self.max_hz,
            frequency_taper: match self.scale {
                SpectrumScale::Log => AnyTaper::Log(log_taper(self.min_hz, self.max_hz)),
                SpectrumScale::Linear => AnyTaper::Linear(linear_taper(self.min_hz, self.max_hz)),
            },
            variant: self.variant,
            peak_hold: self.peak_hold,
            grid: self.grid,
        };
        let reduced_motion = prefers_reduced_motion(cx);
        let version = self.source.as_ref().map_or(0, FrameSource::version);
        let bands = self
            .source
            .as_ref()
            .and_then(|source| source.with_latest(|frame| frame.map(|f| f.bands.clone())));
        let now = now_ms();

        let parts = if self.parts.is_empty() {
            vec![
                SpectrumPart::LevelAxis(SpectrumLevelAxis::new()),
                SpectrumPart::Canvas(SpectrumCanvas::new()),
                SpectrumPart::FrequencyAxis(SpectrumFrequencyAxis::new()),
            ]
        } else {
            self.parts
        };

        let mut level_cell: Option<gpui::Div> = None;
        let mut level_width = StyleRefinement::default();
        let mut frequency_cell: Option<gpui::Div> = None;
        let mut canvases = Vec::new();
        let mut needs_frame = false;
        for part in parts {
            match part {
                SpectrumPart::LevelAxis(axis) => {
                    let cell = level_cell.take().unwrap_or_else(|| {
                        let mut cell = axis_base(cx).h_full().w(px(LEVEL_AXIS_WIDTH));
                        cell.style().refine(&axis.style);
                        level_width.size.width = cell.style().size.width;
                        level_width.min_size.width = cell.style().min_size.width;
                        level_width.max_size.width = cell.style().max_size.width;
                        cell
                    });
                    level_cell = Some(cell.children(level_axis_labels(&axis, &settings)));
                }
                SpectrumPart::FrequencyAxis(axis) => {
                    let cell = frequency_cell.take().unwrap_or_else(|| {
                        let mut cell = axis_base(cx).w_full().h(px(FREQUENCY_AXIS_HEIGHT));
                        cell.style().refine(&axis.style);
                        cell
                    });
                    frequency_cell = Some(cell.children(frequency_axis_labels(&axis, &settings)));
                }
                SpectrumPart::Canvas(part) => {
                    let index = canvases.len();
                    let key = ElementId::NamedChild(
                        Arc::new(self.id.clone()),
                        format!("canvas-{index}").into(),
                    );
                    let state = window.use_keyed_state(key, cx, |_, _| {
                        (settings.clone(), SpectrumPainter::new())
                    });
                    let painter = state.update(cx, |(painted_settings, painter), _| {
                        // New settings re-run the React effect: fresh state.
                        if *painted_settings != settings {
                            *painted_settings = settings.clone();
                            *painter = SpectrumPainter::new();
                        }
                        painter.update(
                            now,
                            version,
                            bands.as_deref(),
                            settings.peak_hold,
                            reduced_motion,
                        );
                        painter.clone()
                    });
                    needs_frame |= painter.dirty;
                    let colors = part.colors.or(self.colors).or(theme_colors);
                    let settings = settings.clone();
                    let mut element = canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            if bounds.size.width <= px(0.0) {
                                return;
                            }
                            if settings.grid
                                && let Some(color) = colors.grid
                            {
                                paint_grid(window, bounds, &settings, color);
                            }
                            let Some(bands) = &painter.bands else {
                                return;
                            };
                            let line = colors.line.unwrap_or_default();
                            match settings.variant {
                                SpectrumVariant::Bars => paint_bars(window, bounds, bands, line),
                                SpectrumVariant::Line => {
                                    paint_curve(window, bounds, bands, false, line)
                                }
                                SpectrumVariant::Area => {
                                    paint_curve(window, bounds, bands, true, line)
                                }
                            }
                            if settings.peak_hold && painter.peaks.len() == bands.len() {
                                let peak = colors.peak.unwrap_or_default();
                                paint_peaks(window, bounds, &painter.peaks, peak);
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full();
                    element.style().refine(&part.style);
                    canvases.push(element);
                }
            }
        }
        // A source can deliver a frame at any time, so keep looking for one.
        if needs_frame || (self.source.is_some() && !canvases.is_empty()) {
            window.request_animation_frame();
        }

        let mut spacer = div().flex_none();
        spacer.style().refine(&level_width);
        let mut root = div()
            .id(self.id)
            .role(gpui::Role::Image)
            .aria_label(self.aria_label)
            .flex()
            .flex_col()
            .gap(px(GRID_GAP))
            .h(px(DEFAULT_HEIGHT))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(GRID_GAP))
                    .flex_1()
                    .min_h_0()
                    .child(level_cell.unwrap_or_else(|| div().flex_none()))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .children(canvases),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .gap(px(GRID_GAP))
                    .child(spacer)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .when_some(frequency_cell, |el, cell| el.child(cell)),
                    ),
            );
        root.style().refine(&self.style);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canvas_repaints_on_a_new_frame() {
        let mut first = SpectrumPainter::new();
        let mut second = SpectrumPainter::new();
        assert!(first.update(16.0, 0, None, false, false));
        assert!(second.update(16.0, 0, None, false, false));
        assert!(!first.update(32.0, 0, None, false, false));
        let bands = [0.5, 0.8, 0.3];
        assert!(first.update(48.0, 1, Some(&bands), false, false));
        assert!(second.update(48.0, 1, Some(&bands), false, false));
        assert_eq!(second.bands.as_deref(), Some(&bands[..]));
    }

    #[test]
    fn peaks_hold_then_fall_by_elapsed_time() {
        let mut painter = SpectrumPainter::new();
        painter.update(0.0, 1, Some(&[1.0]), true, false);
        assert_eq!(painter.peaks, vec![1.0]);
        assert!(!painter.dirty);
        painter.update(FRAME_MS * 10.0, 2, Some(&[0.0]), true, false);
        assert!((painter.peaks[0] - 0.985f32.powi(10)).abs() < 1e-4);
        assert!(painter.dirty);
        // Still falling: repaints without a new frame.
        assert!(painter.update(FRAME_MS * 11.0, 2, Some(&[0.0]), true, false));
        let mut now = FRAME_MS * 11.0;
        while painter.dirty {
            now += FRAME_MS;
            painter.update(now, 2, Some(&[0.0]), true, false);
        }
        assert!(painter.peaks[0] <= 0.001);
    }

    #[test]
    fn reduced_motion_paints_four_times_a_second() {
        let mut painter = SpectrumPainter::new();
        assert!(painter.update(1000.0, 1, Some(&[0.5]), false, true));
        assert!(!painter.update(1100.0, 2, Some(&[0.6]), false, true));
        assert_eq!(painter.bands.as_deref(), Some(&[0.5][..]));
        assert!(painter.update(1250.0, 2, Some(&[0.6]), false, true));
    }

    #[test]
    fn labels_format_like_the_react_component() {
        assert_eq!(format_hz(100.0), "100");
        assert_eq!(format_hz(1000.0), "1k");
        assert_eq!(format_hz(10_000.0), "10k");
        assert_eq!(format_hz(2500.0), "2.5k");
        assert_eq!(format_level(-30.0), "-30");
        assert_eq!(format_level(-0.4), "0");
        assert_eq!(format_level(-12.5), "-12");
    }

    #[test]
    fn level_ticks_every_12_db() {
        assert_eq!(
            default_level_ticks(-100.0, -30.0),
            vec![-30.0, -42.0, -54.0, -66.0, -78.0, -90.0]
        );
        assert_eq!(
            default_level_ticks(-60.0, 0.0),
            vec![0.0, -12.0, -24.0, -36.0, -48.0, -60.0]
        );
    }
}
