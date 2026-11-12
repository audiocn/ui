//! Playground page for `Waveform`: peaks, a play clock, seeking and every part.

use std::sync::{Arc, OnceLock};

use audiocn::audio::frame_loop::now_ms;
use audiocn::components::{
    Waveform, WaveformCanvas, WaveformCursor, WaveformHover, WaveformMarker, WaveformRegion,
    WaveformVariant,
};
use audiocn::{FrameEmitter, create_frame_emitter};
use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec};

pub const SPEC: Spec = Spec {
    slug: "waveform",
    component: "Waveform",
    source: SourceUse::None,
    controls,
    render,
};

const PEAKS: &[(&str, &str)] = &[("short", ""), ("long", ""), ("silence", ""), ("none", "")];

const VARIANT: &[(&str, &str)] = &[
    ("bars", "WaveformVariant::Bars"),
    ("line", "WaveformVariant::Line"),
    ("mirror", "WaveformVariant::Mirror"),
];

const HOVER_FORMAT: &[(&str, &str)] = &[("clock", ""), ("seconds", ""), ("milliseconds", "")];

fn controls() -> Vec<Control> {
    vec![
        Control::bool("controlled", "Controlled", true)
            .group("Playhead")
            .no_code()
            .remount()
            .help("Off uses default_current_time and remounts."),
        Control::num("current_time", "Current time", 4.0, 0.0, 600.0, 0.1)
            .group("Playhead")
            .help("Seconds. The play clock writes this while Playing is on."),
        Control::num("default_current_time", "Default time", 0.0, 0.0, 600.0, 0.1)
            .group("Playhead")
            .remount(),
        Control::bool("playing", "Playing", false)
            .group("Playhead")
            .no_code()
            .help("On while Controlled or Smooth time source: advances the playhead and stops at the end. Seek on the wave, or move Current time by more than 0.3 s."),
        Control::bool("smooth_time", "Smooth time source", false)
            .group("Playhead")
            .no_code()
            .help("Feeds .time(FrameSource) from the clock instead of current_time."),
        Control::choice("peaks", "Peaks", 0, PEAKS)
            .group("Data")
            .no_code()
            .help("Synthetic peaks. none leaves peaks unset; turn Loading on for the skeleton."),
        Control::num("duration", "Duration", 48.0, 0.0, 600.0, 1.0).group("Data"),
        Control::num("step", "Step", 5.0, 0.1, 60.0, 0.1).group("Keyboard"),
        Control::num("large_step", "Large step", 15.0, 0.5, 120.0, 0.5).group("Keyboard"),
        Control::choice("variant", "Variant", 0, VARIANT).group("Look"),
        Control::num("bar_width", "Bar width", 2.0, 1.0, 20.0, 1.0).group("Look"),
        Control::num("bar_gap", "Bar gap", 1.0, 0.0, 12.0, 1.0).group("Look"),
        Control::num("bar_radius", "Bar radius", 1.0, 0.0, 16.0, 0.5).group("Look"),
        Control::color("color", "Colour").group("Look"),
        Control::color("progress_color", "Progress colour").group("Look"),
        Control::color("cursor_color", "Cursor colour").group("Look"),
        Control::bool("interactive", "Interactive", true).group("State"),
        Control::bool("loading", "Loading", false).group("State"),
        Control::bool("disabled", "Disabled", false).group("State"),
        Control::text("aria_label", "ARIA label", "").group("State"),
        Control::bool("show_canvas", "Canvas", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_cursor", "Cursor", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_hover", "Hover", true)
            .group("Hover")
            .no_code(),
        Control::choice("hover_format", "Hover format", 0, HOVER_FORMAT)
            .group("Hover")
            .no_code(),
        Control::bool("show_region", "Region", true)
            .group("Region")
            .no_code(),
        Control::num("region_start", "Start", 8.0, 0.0, 600.0, 0.1)
            .group("Region")
            .no_code(),
        Control::num("region_end", "End", 22.0, 0.0, 600.0, 0.1)
            .group("Region")
            .no_code(),
        Control::bool("draggable", "Draggable", true)
            .group("Region")
            .no_code(),
        Control::bool("resizable", "Resizable", true)
            .group("Region")
            .no_code(),
        Control::num("min_length", "Min length", 0.1, 0.05, 30.0, 0.05)
            .group("Region")
            .no_code(),
        Control::bool("show_marker", "Marker", true)
            .group("Marker")
            .no_code(),
        Control::num("marker_time", "Time", 30.0, 0.0, 600.0, 0.1)
            .group("Marker")
            .no_code(),
        Control::text("marker_label", "Label", "Intro ends")
            .group("Marker")
            .no_code(),
    ]
}

struct PlayClock {
    latched: bool,
    ended: bool,
    anchor: f64,
    anchor_ms: f64,
    written: f64,
    last_emit: Option<f64>,
    emitter: FrameEmitter<f64>,
}

#[derive(Clone, Copy)]
struct Tick {
    shown: f64,
    ended: bool,
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let clock = window.use_keyed_state(ctx.id("clock"), cx, |_, _| PlayClock {
        latched: false,
        ended: false,
        anchor: 0.0,
        anchor_ms: 0.0,
        written: 0.0,
        last_emit: None,
        emitter: create_frame_emitter(),
    });

    let controlled = p.bool("controlled");
    let smooth = p.bool("smooth_time");
    let playing = p.bool("playing") && (controlled || smooth);
    let duration = f64::from(p.num("duration")).max(0.0);
    let slider = f64::from(p.num("current_time"));
    let tick = clock.update(cx, |c, _| c.advance(playing, slider, duration, smooth));
    publish_playhead(ctx, &clock, playing, slider, tick, cx);
    if playing && !smooth {
        window.request_animation_frame();
    }

    let shown = tick.shown;
    let mut wave = Waveform::new(ctx.id("wave"))
        .duration(duration)
        .step(f64::from(p.num("step")))
        .large_step(f64::from(p.num("large_step")))
        .variant(variant(p.choice("variant")))
        .bar_width(p.num("bar_width"))
        .bar_gap(p.num("bar_gap"))
        .bar_radius(p.num("bar_radius"))
        .interactive(p.bool("interactive"))
        .loading(p.bool("loading"))
        .disabled(p.bool("disabled"))
        .w_full()
        .h(px(96.0));
    if let Some(peaks) = peaks(p.choice("peaks")) {
        wave = wave.peaks(peaks);
    }
    if smooth {
        wave = wave.time(clock.read(cx).emitter.source());
    } else if controlled {
        wave = wave.current_time(shown);
    } else {
        wave = wave.default_current_time(f64::from(p.num("default_current_time")));
    }
    if let Some(color) = p.color("color", cx) {
        wave = wave.color(color);
    }
    if let Some(color) = p.color("progress_color", cx) {
        wave = wave.progress_color(color);
    }
    if let Some(color) = p.color("cursor_color", cx) {
        wave = wave.cursor_color(color);
    }
    let label = p.text("aria_label");
    if !label.is_empty() {
        wave = wave.aria_label(label);
    }

    let seek_clock = clock.clone();
    let seek_ctx = ctx.clone();
    let follow = controlled || smooth;
    wave = wave.on_seek(move |t, _, cx| {
        retarget(&seek_clock, t, cx);
        if follow {
            seek_ctx.set_num("current_time", t as f32, cx);
        }
        seek_ctx.log("on_seek", format!("{t:.2}s"), cx);
    });
    let commit_ctx = ctx.clone();
    wave = wave.on_seek_committed(move |t, _, cx| {
        commit_ctx.log("on_seek_committed", format!("{t:.2}s"), cx);
    });

    wave = attach_parts(wave, p, ctx, duration);
    div().w_full().pt(px(28.0)).child(wave).into_any_element()
}

fn publish_playhead(
    ctx: &Ctx,
    clock: &Entity<PlayClock>,
    playing: bool,
    slider: f64,
    tick: Tick,
    cx: &mut App,
) {
    if !(tick.ended || (playing && (tick.shown - slider).abs() > 0.05)) {
        return;
    }
    let ctx = ctx.clone();
    let clock = clock.clone();
    let shown = tick.shown as f32;
    let ended = tick.ended;
    cx.defer(move |cx| {
        ctx.set_num("current_time", shown, cx);
        clock.update(cx, |c, _| c.written = f64::from(shown));
        if ended {
            ctx.set_bool("playing", false, cx);
        }
    });
}

fn retarget(clock: &Entity<PlayClock>, time: f64, cx: &mut App) {
    clock.update(cx, |c, _| {
        c.anchor = time;
        c.anchor_ms = now_ms();
        c.written = time;
        c.ended = false;
        c.emitter.emit(time);
        c.last_emit = Some(time);
    });
}

impl PlayClock {
    fn advance(&mut self, playing: bool, slider: f64, duration: f64, smooth: bool) -> Tick {
        let now = now_ms();
        if !playing {
            self.latched = false;
            self.ended = false;
            self.anchor = slider.clamp(0.0, duration.max(slider));
            self.anchor_ms = now;
            self.written = slider;
            self.emit(self.anchor, smooth);
            return Tick {
                shown: self.anchor,
                ended: false,
            };
        }
        let predicted = if self.latched && !self.ended {
            let at = self.anchor + (now - self.anchor_ms) / 1000.0;
            if duration > 0.0 {
                at.clamp(0.0, duration)
            } else {
                self.anchor
            }
        } else {
            self.anchor
        };
        if (slider - predicted).abs() > 0.3 && (slider - self.written).abs() > 0.3 {
            self.anchor = slider.clamp(0.0, duration.max(slider));
            self.anchor_ms = now;
            self.written = slider;
            self.ended = false;
        }
        if !self.latched {
            self.latched = true;
            self.ended = false;
            if duration > 0.0 && self.anchor >= duration - 0.05 {
                self.anchor = 0.0;
            }
            self.anchor_ms = now;
            self.written = self.anchor;
        }
        if self.ended {
            self.emit(self.anchor, smooth);
            return Tick {
                shown: self.anchor,
                ended: false,
            };
        }
        let at = if duration > 0.0 {
            (self.anchor + (now - self.anchor_ms) / 1000.0).clamp(0.0, duration)
        } else {
            0.0
        };
        if duration > 0.0 && at >= duration - 1e-3 && now - self.anchor_ms > 20.0 {
            self.ended = true;
            self.anchor = duration;
            self.emit(duration, smooth);
            return Tick {
                shown: duration,
                ended: true,
            };
        }
        self.emit(at, smooth);
        Tick {
            shown: at,
            ended: false,
        }
    }

    fn emit(&mut self, shown: f64, smooth: bool) {
        if smooth && self.last_emit.is_none_or(|v| (v - shown).abs() > 5e-4) {
            self.emitter.emit(shown);
            self.last_emit = Some(shown);
        }
    }
}

fn attach_parts(mut wave: Waveform, p: &Props, ctx: &Ctx, duration: f64) -> Waveform {
    let show_canvas = p.bool("show_canvas");
    let show_cursor = p.bool("show_cursor");
    let show_hover = p.bool("show_hover");
    let show_region = p.bool("show_region");
    let show_marker = p.bool("show_marker");
    if show_canvas {
        wave = wave.child(WaveformCanvas::new());
    }
    if show_region {
        let start = f64::from(p.num("region_start")).clamp(0.0, duration);
        let end = f64::from(p.num("region_end")).clamp(start, duration.max(start));
        let region_ctx = ctx.clone();
        wave = wave.child(
            WaveformRegion::new(ctx.id("region"), start, end)
                .draggable(p.bool("draggable"))
                .resizable(p.bool("resizable"))
                .min_length(f64::from(p.num("min_length")))
                .on_value_change(move |value, _, cx| {
                    region_ctx.set_num("region_start", value.start as f32, cx);
                    region_ctx.set_num("region_end", value.end as f32, cx);
                    region_ctx.log(
                        "on_value_change",
                        format!("{:.2}s–{:.2}s", value.start, value.end),
                        cx,
                    );
                }),
        );
    }
    if show_marker {
        let mut marker = WaveformMarker::new(f64::from(p.num("marker_time")));
        let label = p.text("marker_label");
        if !label.is_empty() {
            marker = marker.child(label);
        }
        wave = wave.child(marker);
    }
    if show_cursor {
        wave = wave.child(WaveformCursor::new());
    }
    if show_hover {
        let mut hover = WaveformHover::new();
        hover = match p.choice("hover_format") {
            "seconds" => hover.format(|t| format!("{t:.2}s")),
            "milliseconds" => hover.format(|t| format!("{} ms", (t * 1000.0).round() as i64)),
            _ => hover,
        };
        wave = wave.child(hover);
    }
    if !(show_canvas || show_cursor || show_hover || show_region || show_marker) {
        // Empty children restore the default canvas and cursor.
        wave = wave.child(div());
    }
    wave
}

fn variant(choice: &str) -> WaveformVariant {
    match choice {
        "line" => WaveformVariant::Line,
        "mirror" => WaveformVariant::Mirror,
        _ => WaveformVariant::Bars,
    }
}

fn peaks(kind: &str) -> Option<Arc<[f32]>> {
    match kind {
        "short" => Some(cached(&SHORT, || shape(48, 1.7))),
        "long" => Some(cached(&LONG, || shape(960, 0.37))),
        "silence" => Some(cached(&SILENCE, || vec![0.0; 160].into())),
        _ => None,
    }
}

static SHORT: OnceLock<Arc<[f32]>> = OnceLock::new();
static LONG: OnceLock<Arc<[f32]>> = OnceLock::new();
static SILENCE: OnceLock<Arc<[f32]>> = OnceLock::new();

fn cached(slot: &'static OnceLock<Arc<[f32]>>, make: fn() -> Arc<[f32]>) -> Arc<[f32]> {
    slot.get_or_init(make).clone()
}

fn shape(count: usize, freq: f64) -> Arc<[f32]> {
    let hit = (count / 8).max(1);
    (0..count)
        .map(|i| {
            let body = (i as f64 * freq).sin().abs() as f32 * 0.75;
            let kick = if i % hit < 2 { 0.25 } else { 0.0 };
            (body + kick).clamp(0.0, 1.0)
        })
        .collect()
}
