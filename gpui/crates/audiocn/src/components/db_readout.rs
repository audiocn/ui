//! Port of `components/ui/db-readout.tsx`: a numeric level label that updates
//! at a readable rate and never shifts the layout.
//!
//! React writes the text from a `setInterval` ticker outside React. Here the
//! ticker is a GPUI task in keyed state that wakes every `interval_ms`, steps a
//! [`DbReadoutTicker`] and notifies only when the text changed. Frames are read
//! from the source on every render and every tick (a [`FrameSource`] is a
//! shared slot, not an event stream).

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Div, ElementId, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Task, Window, div,
};

use crate::audio::decibels::{FormatDbOptions, SILENCE_DB, format_db};
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::types::{MeterFrame, MeterZone, MeterZoneName};
use crate::audio::zones::{DEFAULT_ZONES, zone_for_db};
use crate::components::db_scale::tabular_nums;
use crate::theme::ActiveTheme;

const DEFAULT_INTERVAL_MS: f64 = 250.0;
const DEFAULT_FLOOR_DB: f32 = -60.0;
/// Two-digit levels, the widest a meter usually shows, on either side of 0.
const WIDEST_MAGNITUDE_DB: f32 = 88.8;

/// Which measurement a readout shows (`measure`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DbReadoutMeasure {
    #[default]
    Peak,
    Rms,
}

/// A channel index, or the loudest channel (`channel`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DbReadoutChannel {
    #[default]
    Max,
    Index(usize),
}

impl From<usize> for DbReadoutChannel {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

/// `readChannel`: the level a readout shows from one frame.
pub fn read_channel(
    frame: &MeterFrame,
    measure: DbReadoutMeasure,
    channel: DbReadoutChannel,
) -> f32 {
    let read = |index: usize| {
        let Some(level) = frame.channels.get(index) else {
            return SILENCE_DB;
        };
        match measure {
            DbReadoutMeasure::Rms => level.rms_db.unwrap_or(level.peak_db),
            DbReadoutMeasure::Peak => level.peak_db,
        }
    };
    match channel {
        DbReadoutChannel::Index(index) => read(index),
        DbReadoutChannel::Max => (0..frame.channels.len())
            .map(read)
            .fold(SILENCE_DB, f32::max),
    }
}

/// Replaces all formatting.
pub type DbReadoutFormat = Rc<dyn Fn(f32) -> SharedString>;

/// Restyles or wraps a readout by what it shows (see [`DbReadout::decorate`]).
pub type DbReadoutDecorate = Rc<dyn Fn(&DbReadoutDisplay, Div) -> AnyElement>;

/// What a readout shows right now, handed to [`DbReadout::decorate`]
/// (React's `data-zone` and `data-silent`).
#[derive(Clone, Debug, PartialEq)]
pub struct DbReadoutDisplay {
    pub text: SharedString,
    pub db: f32,
    pub zone: MeterZoneName,
    /// At or below the floor.
    pub silent: bool,
}

/// The ticker behind a source-driven readout, as plain state: feed it frames
/// with [`frame`](Self::frame), step it every interval with
/// [`tick`](Self::tick).
#[derive(Clone, Debug, PartialEq)]
pub struct DbReadoutTicker {
    peak: f32,
    peak_at: f64,
    shown: Option<DbReadoutDisplay>,
    fresh: bool,
}

impl Default for DbReadoutTicker {
    fn default() -> Self {
        Self {
            peak: SILENCE_DB,
            peak_at: 0.0,
            shown: None,
            fresh: false,
        }
    }
}

impl DbReadoutTicker {
    /// A level from one frame, at `now_ms`. Keeps the highest within `hold_ms`.
    pub fn frame(&mut self, db: f32, now_ms: f64, hold_ms: f64) {
        if db >= self.peak || now_ms - self.peak_at > hold_ms {
            self.peak = db;
            self.peak_at = now_ms;
        }
        self.fresh = true;
    }

    /// One tick: updates the shown text. Returns whether the ticker still has
    /// work (the text changed or frames arrived since the last tick); a quiet
    /// source whose readout stopped changing lets it stop.
    pub fn tick(
        &mut self,
        render: &dyn Fn(f32) -> SharedString,
        zones: &[MeterZone],
        floor_db: f32,
        hold_ms: f64,
    ) -> bool {
        let db = self.peak;
        let text = render(db);
        let changed = self.shown.as_ref().is_none_or(|shown| shown.text != text);
        if changed {
            self.shown = Some(DbReadoutDisplay {
                text,
                db,
                zone: zone_for_db(db, zones),
                silent: db <= floor_db,
            });
        }
        // Without a hold, a source that stops sending frames falls to −∞.
        if hold_ms == 0.0 {
            self.peak = SILENCE_DB;
        }
        let fresh = self.fresh;
        self.fresh = false;
        changed || fresh
    }

    /// What the last tick wrote, if any.
    pub fn shown(&self) -> Option<&DbReadoutDisplay> {
        self.shown.as_ref()
    }

    /// Forget everything: the next tick writes whatever it reads.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Props the ticker reads on each tick (React's `useEffectEvent`).
#[derive(Clone)]
struct TickProps {
    measure: DbReadoutMeasure,
    channel: DbReadoutChannel,
    hold_ms: f64,
    floor_db: f32,
    zones: Vec<MeterZone>,
    render: DbReadoutFormat,
}

struct DbReadoutState {
    ticker: DbReadoutTicker,
    source: Option<FrameSource<MeterFrame>>,
    version: Option<u64>,
    interval_ms: f64,
    task: Option<Task<()>>,
    props: Option<TickProps>,
}

impl DbReadoutState {
    /// Reads the source's latest frame if it is new.
    fn consume(&mut self) {
        let (Some(source), Some(props)) = (&self.source, &self.props) else {
            return;
        };
        let version = source.version();
        if self.version == Some(version) {
            return;
        }
        self.version = Some(version);
        let db = source.with_latest(|frame| {
            frame.map(|frame| read_channel(frame, props.measure, props.channel))
        });
        if let Some(db) = db {
            self.ticker.frame(db, now_ms(), props.hold_ms);
        }
    }

    /// One interval: returns whether the shown text changed.
    fn tick(&mut self) -> bool {
        self.consume();
        let Some(props) = self.props.clone() else {
            return false;
        };
        let before = self.ticker.shown().map(|s| s.text.clone());
        self.ticker
            .tick(&*props.render, &props.zones, props.floor_db, props.hold_ms);
        before != self.ticker.shown().map(|s| s.text.clone())
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let interval = Duration::from_secs_f64(self.interval_ms.max(1.0) / 1000.0);
        self.ticker.reset();
        // The first tick always writes, whatever the readout shows now.
        self.tick();
        self.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let alive = this.update(cx, |state, cx| {
                    if state.tick() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }
}

/// A numeric level label that updates at a readable rate and never shifts the
/// layout: it reserves the width of the widest text it can show.
///
/// ```ignore
/// DbReadout::new("peak").source(signal.meter.clone())
/// DbReadout::new("rms").measure(DbReadoutMeasure::Rms).source(signal.meter.clone())
/// DbReadout::new("fixed").value(-6.0)
/// ```
#[derive(IntoElement)]
pub struct DbReadout {
    id: ElementId,
    value: Option<f32>,
    source: Option<FrameSource<MeterFrame>>,
    measure: DbReadoutMeasure,
    channel: DbReadoutChannel,
    interval_ms: f64,
    hold_ms: f64,
    decimals: usize,
    unit: bool,
    floor_db: f32,
    zones: Vec<MeterZone>,
    format: Option<DbReadoutFormat>,
    decorate: Option<DbReadoutDecorate>,
    style: StyleRefinement,
}

impl DbReadout {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            source: None,
            measure: DbReadoutMeasure::Peak,
            channel: DbReadoutChannel::Max,
            interval_ms: DEFAULT_INTERVAL_MS,
            hold_ms: 0.0,
            decimals: 1,
            unit: true,
            floor_db: DEFAULT_FLOOR_DB,
            zones: DEFAULT_ZONES.to_vec(),
            format: None,
            decorate: None,
            style: StyleRefinement::default(),
        }
    }

    /// A level in dB, for declarative use.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// A level in dB, or none (React `value={undefined}`).
    pub fn value_opt(mut self, value: Option<f32>) -> Self {
        self.value = value;
        self
    }

    /// A meter source; the readout updates itself at its interval.
    pub fn source(mut self, source: impl Into<Option<FrameSource<MeterFrame>>>) -> Self {
        self.source = source.into();
        self
    }

    /// Which measurement to show. Default `Peak`.
    pub fn measure(mut self, measure: DbReadoutMeasure) -> Self {
        self.measure = measure;
        self
    }

    /// A channel index, or `Max` for the loudest channel. Default `Max`.
    pub fn channel(mut self, channel: impl Into<DbReadoutChannel>) -> Self {
        self.channel = channel.into();
        self
    }

    /// How often the text changes. Default 250 ms.
    pub fn interval_ms(mut self, interval_ms: f64) -> Self {
        self.interval_ms = interval_ms;
        self
    }

    /// Show the highest value seen within this window. Default 0.
    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.hold_ms = hold_ms;
        self
    }

    /// Digits after the decimal point. Default 1.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Append " dB". Default true.
    pub fn unit(mut self, unit: bool) -> Self {
        self.unit = unit;
        self
    }

    /// At or below this level, show "−∞". Default −60.
    pub fn floor_db(mut self, floor_db: f32) -> Self {
        self.floor_db = floor_db;
        self
    }

    /// Zones used for the display's `zone` (React `data-zone`).
    pub fn zones(mut self, zones: impl Into<Vec<MeterZone>>) -> Self {
        self.zones = zones.into();
        self
    }

    /// Replaces all formatting.
    pub fn format(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// Styles by what is shown, React's `data-[zone=clip]:…` / `data-silent:…`
    /// and a parent's `has-data-[zone=clip]:…`: gets the display and the
    /// styled text element, returns what to render (the element, restyled or
    /// wrapped).
    pub fn decorate(
        mut self,
        decorate: impl Fn(&DbReadoutDisplay, Div) -> AnyElement + 'static,
    ) -> Self {
        self.decorate = Some(Rc::new(decorate));
        self
    }
}

impl Styled for DbReadout {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DbReadout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (decimals, unit, floor_db) = (self.decimals, self.unit, self.floor_db);
        let render: DbReadoutFormat = self.format.clone().unwrap_or_else(|| {
            Rc::new(move |db| {
                format_db(
                    db,
                    FormatDbOptions {
                        decimals,
                        floor_db,
                        unit,
                        ..Default::default()
                    },
                )
                .into()
            })
        });

        // Size for the widest text the readout can show, so it never changes
        // width. Values at or below the floor read as −∞, so measure just above it too.
        let just_above_floor = if floor_db.is_finite() {
            floor_db + 10f32.powi(-(decimals as i32))
        } else {
            -WIDEST_MAGNITUDE_DB
        };
        let widest = [
            SILENCE_DB,
            just_above_floor,
            -WIDEST_MAGNITUDE_DB,
            WIDEST_MAGNITUDE_DB,
        ]
        .into_iter()
        .map(|db| render(db).chars().count())
        .max()
        .unwrap_or(0);

        let props = TickProps {
            measure: self.measure,
            channel: self.channel,
            hold_ms: self.hold_ms,
            floor_db,
            zones: self.zones.clone(),
            render: render.clone(),
        };
        let initial_db = self.value.unwrap_or(SILENCE_DB);
        let initial = DbReadoutDisplay {
            text: render(initial_db),
            db: initial_db,
            zone: zone_for_db(initial_db, &self.zones),
            silent: initial_db <= floor_db,
        };

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| DbReadoutState {
            ticker: DbReadoutTicker::default(),
            source: None,
            version: None,
            interval_ms: DEFAULT_INTERVAL_MS,
            task: None,
            props: None,
        });
        let source = self.source.clone();
        let interval_ms = self.interval_ms;
        let display = state.update(cx, |state, cx| {
            state.props = Some(props);
            let restart = state.source != source || state.interval_ms != interval_ms;
            if restart {
                state.source = source.clone();
                state.interval_ms = interval_ms;
                state.version = None;
                state.task = None;
                state.ticker.reset();
                if source.is_some() {
                    state.start(cx);
                }
            } else {
                state.consume();
            }
            // A value never shows the last live level, and a returning source
            // starts from a fresh first paint.
            match &state.source {
                Some(_) => state.ticker.shown().cloned().unwrap_or(initial.clone()),
                None => initial.clone(),
            }
        });

        // `font-mono tabular-nums`, `min-w-(widest)ch`, `text-end`.
        let theme = cx.theme();
        let mut text_style = window.text_style();
        text_style.font_family = theme.font_mono.clone();
        text_style.refine(&self.style.text);
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let font_id = window.text_system().resolve_font(&text_style.font());
        let ch = window
            .text_system()
            .ch_width(font_id, font_size)
            // Unrounded, like the text advances GPUI lays out: Chrome on Linux
            // rounds both (8ch of 14px Geist Mono is 64px there, 67.2px here).
            .unwrap_or(font_size * 0.6);

        let mut el = div()
            .flex()
            .flex_none()
            .justify_end()
            .min_w(ch * widest as f32)
            .font_family(theme.font_mono.clone())
            .font_features(tabular_nums())
            .whitespace_nowrap();
        el.style().refine(&self.style);
        let el = el.child(display.text.clone());
        match &self.decorate {
            Some(decorate) => decorate(&display, el),
            None => el.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::ChannelLevel;

    fn render(db: f32) -> SharedString {
        format_db(
            db,
            FormatDbOptions {
                decimals: 1,
                floor_db: DEFAULT_FLOOR_DB,
                unit: true,
                ..Default::default()
            },
        )
        .into()
    }

    fn tick(ticker: &mut DbReadoutTicker, hold_ms: f64) -> bool {
        ticker.tick(&render, &DEFAULT_ZONES, DEFAULT_FLOOR_DB, hold_ms)
    }

    #[test]
    fn formats_a_declarative_value() {
        assert_eq!(render(-6.0).as_ref(), "−6.0 dB");
    }

    #[test]
    fn reads_peak_rms_and_the_loudest_channel() {
        let frame = MeterFrame {
            channels: vec![
                ChannelLevel::with_rms(-12.0, -20.0),
                ChannelLevel::new(-6.0),
            ],
        };
        assert_eq!(
            read_channel(&frame, DbReadoutMeasure::Peak, DbReadoutChannel::Max),
            -6.0
        );
        assert_eq!(read_channel(&frame, DbReadoutMeasure::Rms, 0.into()), -20.0);
        // No RMS: falls back to the peak.
        assert_eq!(read_channel(&frame, DbReadoutMeasure::Rms, 1.into()), -6.0);
        assert_eq!(
            read_channel(&frame, DbReadoutMeasure::Peak, 5.into()),
            SILENCE_DB
        );
    }

    #[test]
    fn updates_from_a_source_at_its_interval() {
        let mut ticker = DbReadoutTicker::default();
        tick(&mut ticker, 0.0);
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−∞ dB");
        ticker.frame(-12.34, 10.0, 0.0);
        tick(&mut ticker, 0.0);
        let shown = ticker.shown().unwrap();
        assert_eq!(shown.text.as_ref(), "−12.3 dB");
        assert_eq!(shown.zone, MeterZoneName::Warn);
    }

    #[test]
    fn stops_when_the_source_goes_quiet_and_restarts_on_a_frame() {
        let mut ticker = DbReadoutTicker::default();
        ticker.frame(-12.0, 0.0, 0.0);
        assert!(tick(&mut ticker, 0.0));
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−12.0 dB");
        // No frames: falls to −∞, then has nothing left to do.
        assert!(tick(&mut ticker, 0.0));
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−∞ dB");
        assert!(ticker.shown().unwrap().silent);
        assert!(!tick(&mut ticker, 0.0));
        ticker.frame(-6.0, 3000.0, 0.0);
        assert!(tick(&mut ticker, 0.0));
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−6.0 dB");
    }

    #[test]
    fn holds_the_highest_level_within_the_hold() {
        let mut ticker = DbReadoutTicker::default();
        ticker.frame(-6.0, 0.0, 1000.0);
        ticker.frame(-20.0, 100.0, 1000.0);
        tick(&mut ticker, 1000.0);
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−6.0 dB");
        ticker.frame(-20.0, 1200.0, 1000.0);
        tick(&mut ticker, 1000.0);
        assert_eq!(ticker.shown().unwrap().text.as_ref(), "−20.0 dB");
    }
}
