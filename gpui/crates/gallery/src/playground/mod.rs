//! The playground: every component with a live props panel, a signal source
//! picker, shared `AudioConfig` controls, theme presets and an event log.
//!
//! Each component has a [`Spec`] in its own file (`playground/<slug>.rs`): a
//! list of [`Control`]s (one per prop) and a `render` function that builds the
//! component from the current [`Props`]. Callbacks report to the event log with
//! [`Ctx::log`]; controlled components write their value back with
//! [`Ctx::set`], so the panel follows the component.
//!
//! ```ignore
//! pub const SPEC: Spec = Spec {
//!     slug: "knob",
//!     component: "Knob",
//!     source: SourceUse::None,
//!     controls: || vec![
//!         Control::num("value", "Value", 0.0, -24.0, 24.0, 0.1),
//!         Control::bool("disabled", "Disabled", false),
//!         Control::choice("size", "Size", 1, &[("sm", "AudioSize::Sm"), ("default", "AudioSize::Default"), ("lg", "AudioSize::Lg")]),
//!     ],
//!     render: |p, ctx, _window, _cx| {
//!         let c = ctx.clone();
//!         Knob::new(ctx.id("knob"))
//!             .value(p.num("value"))
//!             .disabled(p.bool("disabled"))
//!             .config(ctx.config())
//!             .on_value_change(move |v, _d, _w, cx| { c.set_num("value", v, cx); c.log("on_value_change", format!("{v:.1}"), cx); })
//!             .into_any_element()
//!     },
//! };
//! ```

mod shell;
pub mod source;

pub use shell::Playground;

use std::collections::HashMap;
use std::rc::Rc;

use audiocn::theme::ActiveTheme;
use audiocn::{AudioConfig, FrameSource, MeterFrame, VisualFrame};
use gpui::{AnyElement, App, Entity, Hsla, SharedString, Window};

// One module per component / block, in the docs sidebar order.
mod audio_device_select;
mod audio_player;
mod bar_visualizer;
mod channel_strip;
mod channel_toggle;
mod clip_indicator;
mod db_readout;
mod db_scale;
mod electric_bar_visualizer;
mod electric_waveform;
mod fader;
mod knob;
mod lcd_display;
mod lcd_meter;
mod level_meter;
mod live_waveform;
mod mic_setup;
mod mixer;
mod music_player;
mod pan_control;
mod parameter_slider;
mod quick_audio_popover;
mod smooth_waveform;
mod sound_pad;
mod soundboard;
mod spectrum;
mod system_audio_mixer;
mod system_audio_settings;
mod tempo_control;
mod track_list;
mod volume_control;
mod waveform;

/// Every playground page, grouped like the docs sidebar.
pub const SPECS: &[(&str, &[Spec])] = &[
    (
        "Meters and visualizers",
        &[
            level_meter::SPEC,
            lcd_meter::SPEC,
            db_scale::SPEC,
            db_readout::SPEC,
            clip_indicator::SPEC,
            bar_visualizer::SPEC,
            electric_bar_visualizer::SPEC,
            electric_waveform::SPEC,
            smooth_waveform::SPEC,
            live_waveform::SPEC,
            waveform::SPEC,
            spectrum::SPEC,
        ],
    ),
    (
        "Controls",
        &[
            fader::SPEC,
            parameter_slider::SPEC,
            tempo_control::SPEC,
            knob::SPEC,
            lcd_display::SPEC,
            pan_control::SPEC,
            channel_toggle::SPEC,
            volume_control::SPEC,
            audio_device_select::SPEC,
        ],
    ),
    ("Mixer", &[channel_strip::SPEC, mixer::SPEC]),
    (
        "Sounds and music",
        &[audio_player::SPEC, track_list::SPEC, sound_pad::SPEC],
    ),
    (
        "Blocks",
        &[
            system_audio_mixer::SPEC,
            mic_setup::SPEC,
            system_audio_settings::SPEC,
            quick_audio_popover::SPEC,
            soundboard::SPEC,
            music_player::SPEC,
        ],
    ),
];

/// All specs in sidebar order.
pub fn all_specs() -> impl Iterator<Item = &'static Spec> {
    SPECS.iter().flat_map(|(_, specs)| specs.iter())
}

pub fn find(slug: &str) -> Option<&'static Spec> {
    all_specs().find(|s| s.slug == slug)
}

/// What a component reads from the playground's signal source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceUse {
    /// Doesn't take frames (knobs, faders, ...). The source picker is hidden.
    None,
    /// Takes `FrameSource<MeterFrame>` (meters, readouts, clip lights).
    Meter,
    /// Takes `FrameSource<VisualFrame>` (visualizers, waveforms).
    Visual,
    /// Takes both.
    Both,
}

/// One playground page.
pub struct Spec {
    /// The docs slug, e.g. `knob`.
    pub slug: &'static str,
    /// The Rust type shown in the generated code, e.g. `Knob`.
    pub component: &'static str,
    /// Which frames it reads; shows the source picker when not `None`.
    pub source: SourceUse,
    /// The props panel, in display order. Controls with the same `group`
    /// are shown together under that heading.
    pub controls: fn() -> Vec<Control>,
    /// Builds the component from the current props.
    pub render: fn(&Props, &Ctx, &mut Window, &mut App) -> AnyElement,
}

impl Spec {
    pub fn title(&self) -> String {
        self.slug
            .split('-')
            .map(|w| match w {
                "db" => "dB".to_string(),
                "lcd" => "LCD".to_string(),
                _ => {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A prop value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f32),
    Bool(bool),
    /// Index into the control's options.
    Choice(usize),
    Text(SharedString),
    /// Index into [`COLOR_TOKENS`] (0 = "default": don't set the prop).
    Color(usize),
}

/// How a prop is edited.
#[derive(Clone, Debug)]
pub enum ControlKind {
    /// A slider with a numeric input.
    Num { min: f32, max: f32, step: f32 },
    /// A switch.
    Bool,
    /// A select. Each option is `(label, rust expression)`, the expression
    /// shown in the generated code (e.g. `("vertical", "Orientation::Vertical")`).
    Choice(&'static [(&'static str, &'static str)]),
    /// A text input.
    Text,
    /// A theme colour token (see [`COLOR_TOKENS`]).
    Color,
}

/// One row in the props panel.
#[derive(Clone, Debug)]
pub struct Control {
    /// Key in [`Props`]; also the builder method name in generated code
    /// unless [`Control::method`] says otherwise.
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ControlKind,
    pub default: Value,
    /// Heading in the panel ("Props" by default).
    pub group: &'static str,
    /// Builder method in generated code; `None` hides it from the code.
    pub method: Option<&'static str>,
    /// Changing it remounts the component (for `default_*` / uncontrolled
    /// props that only apply on mount).
    pub remount: bool,
    /// One-line help shown under the label.
    pub help: Option<&'static str>,
    /// Generated-code template, `{}` replaced by the value, e.g.
    /// `".zone_color(MeterZoneName::Ok, {})"`. Overrides `.method(value)`.
    pub code: Option<&'static str>,
    /// Number written as `px(v)` in generated code.
    pub pixels: bool,
}

impl Control {
    fn new(key: &'static str, label: &'static str, kind: ControlKind, default: Value) -> Self {
        Self {
            key,
            label,
            kind,
            default,
            group: "Props",
            method: Some(key),
            remount: false,
            help: None,
            code: None,
            pixels: false,
        }
    }

    /// A number in pixels (a slider; written as `px(v)` in generated code).
    pub fn px(key: &'static str, label: &'static str, default: f32, min: f32, max: f32, step: f32) -> Self {
        let mut c = Self::num(key, label, default, min, max, step);
        c.pixels = true;
        c
    }

    /// Generated code for this prop from a template, `{}` replaced by the value.
    pub fn code(mut self, template: &'static str) -> Self {
        self.code = Some(template);
        self
    }

    /// Writes a number as `px(v)` in generated code.
    pub fn pixels(mut self) -> Self {
        self.pixels = true;
        self
    }

    pub fn num(
        key: &'static str,
        label: &'static str,
        default: f32,
        min: f32,
        max: f32,
        step: f32,
    ) -> Self {
        Self::new(
            key,
            label,
            ControlKind::Num { min, max, step },
            Value::Num(default),
        )
    }

    pub fn bool(key: &'static str, label: &'static str, default: bool) -> Self {
        Self::new(key, label, ControlKind::Bool, Value::Bool(default))
    }

    pub fn choice(
        key: &'static str,
        label: &'static str,
        default: usize,
        options: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self::new(
            key,
            label,
            ControlKind::Choice(options),
            Value::Choice(default),
        )
    }

    pub fn text(key: &'static str, label: &'static str, default: &'static str) -> Self {
        Self::new(key, label, ControlKind::Text, Value::Text(default.into()))
    }

    pub fn color(key: &'static str, label: &'static str) -> Self {
        Self::new(key, label, ControlKind::Color, Value::Color(0))
    }

    pub fn group(mut self, group: &'static str) -> Self {
        self.group = group;
        self
    }

    /// The builder method in generated code (when it differs from `key`).
    pub fn method(mut self, method: &'static str) -> Self {
        self.method = Some(method);
        self
    }

    /// Leave this control out of the generated code.
    pub fn no_code(mut self) -> Self {
        self.method = None;
        self
    }

    pub fn remount(mut self) -> Self {
        self.remount = true;
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }
}

/// Theme colour tokens a [`ControlKind::Color`] control can pick.
/// Index 0 means "default" (the component's own colour).
pub const COLOR_TOKENS: &[&str] = &[
    "default",
    "foreground",
    "primary",
    "muted-foreground",
    "destructive",
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "meter-ok",
    "meter-warn",
    "meter-clip",
    "channel-mute",
    "channel-solo",
    "channel-monitor",
];

/// Resolves a [`COLOR_TOKENS`] index against the current theme.
pub fn token_color(index: usize, cx: &App) -> Option<Hsla> {
    let t = cx.theme();
    Some(match *COLOR_TOKENS.get(index)? {
        "foreground" => t.foreground,
        "primary" => t.primary,
        "muted-foreground" => t.muted_foreground,
        "destructive" => t.destructive,
        "chart-1" => t.chart_1,
        "chart-2" => t.chart_2,
        "chart-3" => t.chart_3,
        "chart-4" => t.chart_4,
        "chart-5" => t.chart_5,
        "meter-ok" => t.meter_ok,
        "meter-warn" => t.meter_warn,
        "meter-clip" => t.meter_clip,
        "channel-mute" => t.channel_mute,
        "channel-solo" => t.channel_solo,
        "channel-monitor" => t.channel_monitor,
        _ => return None,
    })
}

/// The current prop values of one page.
#[derive(Clone, Debug, Default)]
pub struct Props {
    values: HashMap<&'static str, Value>,
    /// Choice options per key, for [`Props::choice`].
    options: HashMap<&'static str, &'static [(&'static str, &'static str)]>,
}

impl Props {
    pub fn from_controls(controls: &[Control]) -> Self {
        let mut props = Self::default();
        for c in controls {
            props.values.insert(c.key, c.default.clone());
            if let ControlKind::Choice(options) = c.kind {
                props.options.insert(c.key, options);
            }
        }
        props
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: &'static str, value: Value) {
        self.values.insert(key, value);
    }

    /// A number prop (0 if missing or not a number).
    pub fn num(&self, key: &str) -> f32 {
        match self.values.get(key) {
            Some(Value::Num(v)) => *v,
            _ => 0.0,
        }
    }

    pub fn bool(&self, key: &str) -> bool {
        matches!(self.values.get(key), Some(Value::Bool(true)))
    }

    /// The selected option's index.
    pub fn index(&self, key: &str) -> usize {
        match self.values.get(key) {
            Some(Value::Choice(i)) => *i,
            _ => 0,
        }
    }

    /// The selected option's label (the first tuple element), e.g. `"vertical"`.
    pub fn choice(&self, key: &str) -> &'static str {
        let options = self.options.get(key).copied().unwrap_or(&[]);
        options.get(self.index(key)).map(|o| o.0).unwrap_or("")
    }

    pub fn text(&self, key: &str) -> SharedString {
        match self.values.get(key) {
            Some(Value::Text(t)) => t.clone(),
            _ => SharedString::default(),
        }
    }

    /// The picked colour token, or `None` for "default".
    pub fn color(&self, key: &str, cx: &App) -> Option<Hsla> {
        match self.values.get(key) {
            Some(Value::Color(i)) => token_color(*i, cx),
            _ => None,
        }
    }
}

/// One event-log entry.
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub at_ms: f64,
    pub event: SharedString,
    pub detail: SharedString,
}

/// Shared state the page's callbacks write to (props and the event log).
pub struct PageState {
    pub props: Props,
    pub log: Vec<LogEntry>,
    /// Bumped to remount the component (fresh keyed state).
    pub generation: u64,
    started: audiocn::motion::Instant,
}

impl PageState {
    pub fn new(props: Props) -> Self {
        Self {
            props,
            log: Vec::new(),
            generation: 0,
            started: audiocn::motion::Instant::now(),
        }
    }

    pub fn push_log(&mut self, event: SharedString, detail: SharedString) {
        let at_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        self.log.push(LogEntry {
            at_ms,
            event,
            detail,
        });
        if self.log.len() > 500 {
            self.log.drain(..self.log.len() - 500);
        }
    }
}

/// What a spec's `render` gets besides its props: ids, frames, config and the
/// way back to the panel and the event log. Cheap to clone into callbacks.
#[derive(Clone)]
pub struct Ctx {
    pub(crate) state: Entity<PageState>,
    pub(crate) slug: &'static str,
    pub(crate) generation: u64,
    pub(crate) meter: Option<FrameSource<MeterFrame>>,
    pub(crate) visual: Option<FrameSource<VisualFrame>>,
    pub(crate) config: AudioConfig,
    pub(crate) channels: usize,
}

impl Ctx {
    /// A stable element id for a part of this page; changes on remount.
    pub fn id(&self, part: &str) -> SharedString {
        format!("pg/{}/{}/{}", self.slug, self.generation, part).into()
    }

    /// The meter frames picked in the source panel (None = "no source").
    pub fn meter(&self) -> Option<FrameSource<MeterFrame>> {
        self.meter.clone()
    }

    /// The visual frames picked in the source panel (None = "no source").
    pub fn visual(&self) -> Option<FrameSource<VisualFrame>> {
        self.visual.clone()
    }

    /// The shared `AudioConfig` from the toolbar (size, orientation, disabled,
    /// dimmed). Pass it to `.config(..)` on components that read config.
    pub fn config(&self) -> AudioConfig {
        self.config.clone()
    }

    /// Channel count picked in the source panel (1 or 2).
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Writes a prop back (controlled components), so the panel follows.
    pub fn set(&self, key: &'static str, value: Value, cx: &mut App) {
        self.state.update(cx, |s, cx| {
            s.props.set(key, value);
            cx.notify();
        });
    }

    pub fn set_num(&self, key: &'static str, value: f32, cx: &mut App) {
        self.set(key, Value::Num(value), cx);
    }

    pub fn set_bool(&self, key: &'static str, value: bool, cx: &mut App) {
        self.set(key, Value::Bool(value), cx);
    }

    pub fn set_text(&self, key: &'static str, value: impl Into<SharedString>, cx: &mut App) {
        self.set(key, Value::Text(value.into()), cx);
    }

    /// Adds an entry to the event log, e.g. `ctx.log("on_value_change", "-6.0 dB", cx)`.
    pub fn log(
        &self,
        event: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        cx: &mut App,
    ) {
        let (event, detail) = (event.into(), detail.into());
        self.state.update(cx, |s, cx| {
            s.push_log(event, detail);
            cx.notify();
        });
    }

    /// A logging callback for `Fn(T, &mut Window, &mut App)` props.
    pub fn logger<T: std::fmt::Debug>(
        &self,
        event: &'static str,
    ) -> impl Fn(T, &mut Window, &mut App) + 'static {
        let ctx = self.clone();
        move |value, _, cx| ctx.log(event, format!("{value:?}"), cx)
    }
}

/// Generated Rust for the current props: `Component::new(..)` plus one builder
/// call per prop that differs from its default.
pub fn generate_code(spec: &Spec, controls: &[Control], props: &Props) -> String {
    let mut out = format!("{}::new(\"{}\")", spec.component, spec.slug);
    match spec.source {
        SourceUse::None => {}
        SourceUse::Meter => out.push_str("\n    .source(signal.meter.clone())"),
        SourceUse::Visual => out.push_str("\n    .source(signal.visual.clone())"),
        SourceUse::Both => out.push_str("\n    .source(signal.visual.clone())"),
    }
    for c in controls {
        if c.method.is_none() && c.code.is_none() {
            continue;
        }
        let Some(value) = props.get(c.key) else {
            continue;
        };
        if *value == c.default {
            continue;
        }
        let arg = match (&c.kind, value) {
            (_, Value::Num(v)) if c.pixels => format!("px({})", rust_f32(*v)),
            (_, Value::Num(v)) => rust_f32(*v),
            (_, Value::Bool(b)) => b.to_string(),
            (ControlKind::Choice(options), Value::Choice(i)) => {
                options.get(*i).map(|o| o.1.to_string()).unwrap_or_default()
            }
            (_, Value::Text(t)) => format!("{t:?}"),
            (_, Value::Color(i)) => match COLOR_TOKENS.get(*i) {
                Some(&"default") | None => continue,
                Some(token) => format!("cx.theme().{}", token.replace('-', "_")),
            },
            _ => continue,
        };
        match (c.code, c.method) {
            (Some(template), _) => {
                let call = template.replace("{}", &arg);
                let call = call.trim_start_matches('.');
                out.push_str(&format!("\n    .{call}"));
            }
            (None, Some(method)) => out.push_str(&format!("\n    .{method}({arg})")),
            (None, None) => {}
        }
    }
    out
}

fn rust_f32(v: f32) -> String {
    if v.is_infinite() {
        return if v < 0.0 {
            "f32::NEG_INFINITY".into()
        } else {
            "f32::INFINITY".into()
        };
    }
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{s}.0")
    }
}

/// A shared, cheap-to-clone render function used by the shell.
pub(crate) type RenderFn = Rc<dyn Fn(&Props, &Ctx, &mut Window, &mut App) -> AnyElement>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_f32_formats_like_source() {
        assert_eq!(rust_f32(24.0), "24.0");
        assert_eq!(rust_f32(-0.5), "-0.5");
        assert_eq!(rust_f32(f32::NEG_INFINITY), "f32::NEG_INFINITY");
    }

    #[test]
    fn code_templates_and_pixels() {
        let spec = Spec {
            slug: "x",
            component: "X",
            source: SourceUse::None,
            controls: Vec::new,
            render: |_, _, _, _| unreachable!(),
        };
        let controls = vec![
            Control::px("gap", "Gap", 2.0, 0.0, 8.0, 1.0),
            Control::color("ok", "Ok colour").code(".zone_color(MeterZoneName::Ok, {})"),
        ];
        let mut props = Props::from_controls(&controls);
        props.set("gap", Value::Num(3.0));
        props.set("ok", Value::Color(2));
        let code = generate_code(&spec, &controls, &props);
        assert!(code.contains(".gap(px(3.0))"), "{code}");
        assert!(code.contains(".zone_color(MeterZoneName::Ok, cx.theme().primary)"), "{code}");
    }

    #[test]
    fn every_spec_has_unique_slug() {
        let mut seen = std::collections::HashSet::new();
        for spec in all_specs() {
            assert!(seen.insert(spec.slug), "duplicate {}", spec.slug);
        }
    }

    #[test]
    fn props_read_defaults() {
        let controls = vec![
            Control::num("min", "Min", -60.0, -120.0, 0.0, 1.0),
            Control::bool("hold", "Hold", true),
            Control::choice(
                "o",
                "Orientation",
                1,
                &[("horizontal", "H"), ("vertical", "V")],
            ),
        ];
        let p = Props::from_controls(&controls);
        assert_eq!(p.num("min"), -60.0);
        assert!(p.bool("hold"));
        assert_eq!(p.choice("o"), "vertical");
    }
}
