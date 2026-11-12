//! Port of `components/ui/clip-indicator.tsx`: a clip light that holds after
//! the signal clips, with an optional count and click to reset.
//!
//! The latch logic is [`ClipHold`] (`useClipHold`); its release timer is a
//! GPUI task in keyed state. A source is read on every render and polled while
//! attached (a [`FrameSource`] is a shared slot, not an event stream).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext, Context, ElementId, Entity, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, ParentElement, Refineable, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Task, WeakEntity, Window, div,
    prelude::FluentBuilder, px,
};

use crate::audio::decibels::SILENCE_DB;
use crate::audio::frame_loop::now_ms;
use crate::audio::frame_source::FrameSource;
use crate::audio::types::MeterFrame;
use crate::audio::zones::{CLIP_HOLD_MS, CLIP_THRESHOLD_DB};
use crate::components::db_scale::tabular_nums;
use crate::signal::clip_hold::{ClipHold, ClipHoldOptions};
use crate::theme::ActiveTheme;
use crate::ui::styles::ring_overlay;

/// How often an attached source is checked for new frames while nothing else
/// re-renders the indicator.
const SOURCE_POLL_MS: u64 = 33;

type ClippingHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The loudest peak of a frame (`loudestPeak`).
pub fn loudest_peak(frame: &MeterFrame) -> f32 {
    frame
        .channels
        .iter()
        .map(|level| level.peak_db)
        .fold(SILENCE_DB, f32::max)
}

/// Imperative handle (React `actionsRef`): feed readings and reset from
/// outside. Create one, keep it (e.g. in keyed state), pass a clone to
/// [`ClipIndicator::actions`].
///
/// ```ignore
/// let clip = ClipIndicatorActions::new();
/// ClipIndicator::new("clip").actions(clip.clone());
/// clip.report(0.0, window, cx);
/// ```
#[derive(Clone, Default)]
pub struct ClipIndicatorActions {
    state: Rc<RefCell<Option<WeakEntity<ClipIndicatorState>>>>,
}

impl ClipIndicatorActions {
    pub fn new() -> Self {
        Self::default()
    }

    fn entity(&self) -> Option<Entity<ClipIndicatorState>> {
        self.state.borrow().as_ref().and_then(WeakEntity::upgrade)
    }

    /// Feed a level reading.
    pub fn report(&self, db: f32, window: &mut Window, cx: &mut App) {
        if let Some(state) = self.entity() {
            let handle = window.window_handle();
            let changed = state.update(cx, |state, cx| {
                let changed = state.hold.report(db, now_ms());
                state.after(changed, handle, cx)
            });
            fire(changed, window, cx);
        }
    }

    /// Turn the light off and clear the count.
    pub fn reset(&self, window: &mut Window, cx: &mut App) {
        if let Some(state) = self.entity() {
            fire(reset(&state, cx), window, cx);
        }
    }

    /// Whether the light is on (detected state).
    pub fn clipping(&self, cx: &App) -> bool {
        self.entity()
            .is_some_and(|state| state.read(cx).hold.clipping())
    }

    /// Clips counted since the last reset.
    pub fn count(&self, cx: &App) -> u32 {
        self.entity().map_or(0, |state| state.read(cx).hold.count())
    }
}

type Change = Option<(bool, ClippingHandler)>;

fn fire(change: Change, window: &mut Window, cx: &mut App) {
    if let Some((clipping, handler)) = change {
        handler(clipping, window, cx);
    }
}

fn reset(state: &Entity<ClipIndicatorState>, cx: &mut App) -> Change {
    state.update(cx, |state, cx| {
        let changed = state.hold.reset();
        state.release = None;
        cx.notify();
        changed.zip(state.on_change.clone())
    })
}

/// Keyed state of a [`ClipIndicator`].
pub struct ClipIndicatorState {
    hold: ClipHold,
    focus: FocusHandle,
    source: Option<FrameSource<MeterFrame>>,
    version: Option<u64>,
    poll: Option<Task<()>>,
    release: Option<(f64, Task<()>)>,
    on_change: Option<ClippingHandler>,
}

impl ClipIndicatorState {
    /// Reads the source's latest frame if it is new. Returns the change.
    fn consume(&mut self) -> Option<bool> {
        let source = self.source.as_ref()?;
        let version = source.version();
        if self.version == Some(version) {
            return None;
        }
        self.version = Some(version);
        let db = source.with_latest(|frame| frame.map(loudest_peak))?;
        self.hold.report(db, now_ms())
    }

    /// Notifies on a change, (re)arms the release timer, and returns the
    /// callback to fire outside the update.
    fn after(
        &mut self,
        changed: Option<bool>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Change {
        if changed.is_some() {
            cx.notify();
        }
        let release_at = self.hold.release_at_ms();
        let scheduled = self.release.as_ref().map(|(at, _)| *at);
        if release_at != scheduled {
            self.release = release_at.map(|at| {
                let delay = Duration::from_secs_f64((at - now_ms()).max(0.0) / 1000.0);
                let task = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(delay).await;
                    let Ok(change) = this.update(cx, |state, cx| {
                        state.release = None;
                        let changed = state.hold.update(now_ms());
                        state.after(changed, window, cx)
                    }) else {
                        return;
                    };
                    if let Some((clipping, handler)) = change {
                        cx.update_window(window, |_, window, cx| handler(clipping, window, cx))
                            .ok();
                    }
                });
                (at, task)
            });
        }
        changed.zip(self.on_change.clone())
    }

    fn poll(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let interval = Duration::from_millis(SOURCE_POLL_MS);
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let Ok(change) = this.update(cx, |state, cx| {
                    let changed = state.consume();
                    state.after(changed, window, cx)
                }) else {
                    break;
                };
                if let Some((clipping, handler)) = change {
                    cx.update_window(window, |_, window, cx| handler(clipping, window, cx))
                        .ok();
                }
            }
        }));
    }
}

/// A clip light that holds after the signal clips, with an optional count and
/// click (or Enter / Space) to reset.
///
/// ```ignore
/// ClipIndicator::new("clip").source(signal.meter.clone())
/// ClipIndicator::new("count").show_count(true).source(signal.meter.clone())
/// ClipIndicator::new("text").source(signal.meter.clone()).child("Clip")
/// ```
#[derive(IntoElement)]
pub struct ClipIndicator {
    id: ElementId,
    clipping: Option<bool>,
    source: Option<FrameSource<MeterFrame>>,
    threshold_db: f32,
    hold_ms: f64,
    on_clipping_change: Option<ClippingHandler>,
    show_count: bool,
    actions: Option<ClipIndicatorActions>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ClipIndicator {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            clipping: None,
            source: None,
            threshold_db: CLIP_THRESHOLD_DB,
            hold_ms: CLIP_HOLD_MS,
            on_clipping_change: None,
            show_count: false,
            actions: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Replaces the element id (used by parts that wrap the indicator).
    pub(crate) fn with_id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Controlled clip state: the light shows this instead of what it detected.
    pub fn clipping(mut self, clipping: bool) -> Self {
        self.clipping = Some(clipping);
        self
    }

    /// A meter source; the indicator detects clipping itself.
    pub fn source(mut self, source: impl Into<Option<FrameSource<MeterFrame>>>) -> Self {
        self.source = source.into();
        self
    }

    /// Levels at or above this count as a clip. Default −1 dBFS.
    pub fn threshold_db(mut self, threshold_db: f32) -> Self {
        self.threshold_db = threshold_db;
        self
    }

    /// How long the light stays on. `f64::INFINITY` latches until reset. Default 1500 ms.
    pub fn hold_ms(mut self, hold_ms: f64) -> Self {
        self.hold_ms = hold_ms;
        self
    }

    pub fn on_clipping_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clipping_change = Some(Rc::new(handler));
        self
    }

    /// Show the number of clips next to the light. Default false.
    pub fn show_count(mut self, show_count: bool) -> Self {
        self.show_count = show_count;
        self
    }

    /// Imperative report / reset (React `actionsRef`).
    pub fn actions(mut self, actions: ClipIndicatorActions) -> Self {
        self.actions = Some(actions);
        self
    }
}

impl ParentElement for ClipIndicator {
    /// Children replace the light.
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for ClipIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ClipIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let options = ClipHoldOptions {
            threshold_db: self.threshold_db,
            hold_ms: self.hold_ms,
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| ClipIndicatorState {
            hold: ClipHold::new(options),
            focus: cx.focus_handle().tab_stop(true),
            source: None,
            version: None,
            poll: None,
            release: None,
            on_change: None,
        });
        if let Some(actions) = &self.actions {
            *actions.state.borrow_mut() = Some(state.downgrade());
        }
        let handle = window.window_handle();
        let source = self.source.clone();
        let on_change = self.on_clipping_change.clone();
        let change = state.update(cx, |state, cx| {
            state.on_change = on_change;
            let now = now_ms();
            if *state.hold.options() != options {
                state.hold.set_options(options, now);
            }
            if state.source != source {
                state.source = source;
                state.version = None;
                state.poll = None;
                if state.source.is_some() {
                    state.poll(handle, cx);
                }
            }
            let mut changed = state.hold.update(now);
            if let Some(consumed) = state.consume() {
                changed = Some(consumed);
            }
            state.after(changed, handle, cx)
        });
        fire(change, window, cx);

        let s = state.read(cx);
        let detected = s.hold.clipping();
        let count = s.hold.count();
        let focus = s.focus.clone();
        let clipping = self.clipping.unwrap_or(detected);
        let focus_visible = focus.is_focused(window) && window.last_input_was_keyboard();

        let theme = cx.theme();
        let radius = px(10.0);
        let light = if clipping {
            theme.meter_clip
        } else {
            theme.muted_foreground.opacity(0.3)
        };
        let hover_bg = theme.muted;
        let ring = theme.ring.opacity(0.3);

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .h(px(20.0))
            .gap(px(4.0))
            .px(px(4.0))
            .rounded(radius)
            .text_size(px(12.0))
            .line_height(px(16.0))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .text_color(if clipping {
                theme.meter_clip_foreground
            } else {
                theme.muted_foreground
            })
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .role(gpui::Role::Button)
            .aria_label(if clipping {
                "Clipping. Reset clip indicator"
            } else {
                "Clip indicator"
            })
            .track_focus(&focus)
            .when(focus_visible, |el| {
                el.child(ring_overlay(radius, 3.0, ring))
            })
            .map(|el| {
                if self.children.is_empty() {
                    el.child(div().flex_shrink_0().size(px(8.0)).rounded_full().bg(light))
                } else {
                    el.children(self.children)
                }
            })
            .when(self.show_count, |el| {
                el.child(div().font_features(tabular_nums()).child(count.to_string()))
            });
        el.style().refine(&self.style);

        let click = state.clone();
        el.on_click(move |_, window, cx| {
            fire(reset(&click, cx), window, cx);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::ChannelLevel;

    #[test]
    fn reads_the_loudest_peak() {
        let frame = MeterFrame {
            channels: vec![ChannelLevel::new(-12.0), ChannelLevel::new(-0.5)],
        };
        assert_eq!(loudest_peak(&frame), -0.5);
        assert_eq!(loudest_peak(&MeterFrame::default()), SILENCE_DB);
    }

    #[test]
    fn lights_up_on_a_clip_counts_it_and_resets() {
        let mut hold = ClipHold::default();
        assert!(!hold.clipping());
        assert_eq!(hold.report(0.0, 0.0), Some(true));
        assert!(hold.clipping());
        assert_eq!(hold.count(), 1);
        assert_eq!(hold.reset(), Some(false));
        assert_eq!(hold.count(), 0);
    }

    #[test]
    fn turns_off_after_the_hold_time() {
        let mut hold = ClipHold::new(ClipHoldOptions {
            hold_ms: 500.0,
            ..Default::default()
        });
        hold.report(-0.5, 0.0);
        assert!(hold.clipping());
        hold.update(10.0);
        assert!(hold.clipping());
        hold.update(610.0);
        assert!(!hold.clipping());
    }
}
