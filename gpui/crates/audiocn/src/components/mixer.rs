//! Port of `components/ui/mixer.tsx`: the container for channel strips, with
//! shared meter settings and keyboard navigation between strips.
//!
//! [`Mixer`] is to its strips what [`ChannelStrip`](crate::ChannelStrip) is
//! to its components (see the `channel_strip` module docs): it publishes an
//! [`AudioConfig`] (orientation, size, disabled, dB range, zones, ballistics)
//! that every strip inside inherits automatically, and that you hand to other
//! components with [`Mixer::child_config`] or [`Mixer::child_with`]. A strip
//! merges its own settings over it and passes the result on through
//! [`ChannelStrip::child_with`](crate::ChannelStrip::child_with).
//!
//! ```ignore
//! Mixer::new("mixer")
//!     .min_db(-48.0)
//!     .child(MixerHeader::new().child(MixerTitle::new("Audio mixer")))
//!     .child(MixerChannels::new().children(strips))
//!     .child(MixerSeparator::new())
//!     .child(MixerMaster::new().child(master_strip))
//! ```
//!
//! Parts are placed by grid area like the React grid, whatever their order.
//! Inside [`MixerChannels`], Ctrl+arrow (Cmd+arrow on macOS) moves focus to
//! the same control on the previous / next strip: up/down for rows,
//! left/right for console strips.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Refineable, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, px,
};

use crate::audio::ballistics::BallisticsInput;
use crate::audio::types::{MeterZone, Orientation};
use crate::config::{AudioConfig, AudioSize};
use crate::theme::ActiveTheme;
use crate::ui::styles::{TextSize, provide, radius_xl, use_context};

use super::channel_strip::{AreaElement, LayoutArea, area_part, parent_styled};

/// `--mixer-gap`: the space between strips.
const MIXER_GAP: f32 = 8.0;
/// `gap-3` between the mixer's areas.
const AREA_GAP: f32 = 12.0;
/// Upper bound on tab stops walked when looking for a neighbour's control.
const MAX_TAB_STOPS: usize = 4096;

/// The surrounding mixer's layout, for custom parts (`useMixerContext`), plus
/// the config it passes to its strips.
#[derive(Clone, Debug, PartialEq)]
pub struct MixerContext {
    pub orientation: Orientation,
    /// What the mixer's `AudioConfigProvider` gives its children.
    pub config: AudioConfig,
}

/// The surrounding mixer's layout (`useMixerContext`). Call it while
/// rendering a part inside a [`Mixer`]; `None` outside one (React throws).
pub fn use_mixer_context() -> Option<MixerContext> {
    use_context::<MixerContext>()
}

/// The strips rendered inside one [`MixerChannels`], in order: each
/// [`ChannelStrip`](crate::ChannelStrip) registers its focus handle here
/// while it renders.
#[derive(Clone, Default)]
pub(crate) struct StripRegistry(Rc<RefCell<Vec<FocusHandle>>>);

impl StripRegistry {
    pub(crate) fn register(&self, focus: FocusHandle) {
        self.0.borrow_mut().push(focus);
    }

    fn strips(&self) -> Vec<FocusHandle> {
        self.0.borrow().clone()
    }
}

type ChildBuilder = Box<dyn FnOnce(&MixerContext, &mut Window, &mut App) -> AnyElement>;

enum MixerChild {
    Element(AnyElement),
    Deferred(ChildBuilder),
}

/// A mixer: [`MixerHeader`], [`MixerChannels`] full of
/// [`ChannelStrip`](crate::ChannelStrip)s, an optional [`MixerSeparator`] and
/// [`MixerMaster`], and a [`MixerEmpty`] state. `horizontal` (default) stacks
/// row strips; `vertical` lays console strips side by side.
///
/// ```ignore
/// Mixer::new("console")
///     .orientation(Orientation::Vertical)
///     .child(MixerHeader::new().child(MixerTitle::new("Console")))
///     .child(MixerChannels::new().children(strips))
/// ```
#[derive(IntoElement)]
pub struct Mixer {
    id: gpui::ElementId,
    orientation: Orientation,
    size: Option<AudioSize>,
    min_db: Option<f32>,
    max_db: Option<f32>,
    zones: Option<Vec<MeterZone>>,
    ballistics: Option<BallisticsInput>,
    disabled: bool,
    children: Vec<MixerChild>,
    style: StyleRefinement,
}

impl Mixer {
    pub fn new(id: impl Into<gpui::ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Horizontal,
            size: None,
            min_db: None,
            max_db: None,
            zones: None,
            ballistics: None,
            disabled: false,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// `horizontal` stacks row strips; `vertical` lays console strips side by
    /// side. Default `horizontal`.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Shared meter range: the bottom of the scale.
    pub fn min_db(mut self, min_db: f32) -> Self {
        self.min_db = Some(min_db);
        self
    }

    /// Shared meter range: the top of the scale.
    pub fn max_db(mut self, max_db: f32) -> Self {
        self.max_db = Some(max_db);
        self
    }

    /// Shared meter zones.
    pub fn zones(mut self, zones: Vec<MeterZone>) -> Self {
        self.zones = Some(zones);
        self
    }

    /// Shared meter movement.
    pub fn ballistics(mut self, ballistics: impl Into<BallisticsInput>) -> Self {
        self.ballistics = Some(ballistics.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The config the mixer passes to its strips (and to anything else you
    /// build inside it). Strips inside a mixer inherit it on their own.
    pub fn child_config(&self) -> AudioConfig {
        AudioConfig {
            ballistics: self.ballistics.clone(),
            disabled: self.disabled,
            max_db: self.max_db,
            min_db: self.min_db,
            orientation: Some(self.orientation),
            size: self.size,
            zones: self.zones.clone(),
            dimmed: false,
        }
    }

    /// Adds a child built when the mixer renders, from its final
    /// [`MixerContext`] (its config merged over any inherited one).
    pub fn child_with<E: IntoElement>(
        mut self,
        build: impl FnOnce(&MixerContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.children
            .push(MixerChild::Deferred(Box::new(move |mixer, window, cx| {
                build(mixer, window, cx).into_any_element()
            })));
        self
    }
}

impl ParentElement for Mixer {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children
            .extend(elements.into_iter().map(MixerChild::Element));
    }
}

impl Styled for Mixer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The mixer's parts sorted by grid area.
#[derive(Default)]
struct MixerAreas {
    header: Vec<AnyElement>,
    channels: Vec<AnyElement>,
    separator: Vec<AnyElement>,
    master: Vec<AnyElement>,
    empty: Vec<AnyElement>,
    other: Vec<AnyElement>,
    /// A `MixerChannels` renders nothing (`:empty`).
    channels_empty: bool,
}

impl MixerAreas {
    fn sort(children: Vec<AnyElement>) -> Self {
        let mut areas = Self::default();
        for mut child in children {
            let area = AreaElement::area_of(&mut child);
            let bucket = match area {
                Some((LayoutArea::MixerHeader, _)) => &mut areas.header,
                Some((LayoutArea::MixerChannels, empty)) => {
                    areas.channels_empty |= empty;
                    &mut areas.channels
                }
                Some((LayoutArea::MixerSeparator, _)) => &mut areas.separator,
                Some((LayoutArea::MixerMaster, _)) => &mut areas.master,
                Some((LayoutArea::MixerEmpty, _)) => &mut areas.empty,
                _ => &mut areas.other,
            };
            bucket.push(child);
        }
        areas
    }
}

/// Whether [`MixerEmpty`] shows: only when a [`MixerChannels`] renders nothing.
pub fn mixer_empty_visible(channels: &[bool]) -> bool {
    channels.iter().any(|&empty| empty)
}

fn cell(parts: Vec<AnyElement>) -> gpui::Div {
    div().flex().flex_col().min_w_0().min_h_0().children(parts)
}

impl gpui::RenderOnce for Mixer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let inherited = use_context::<AudioConfig>().unwrap_or_default();
        let config = self.child_config().merged_over(&inherited);
        let orientation = self.orientation;
        let context = MixerContext {
            orientation,
            config: config.clone(),
        };
        let children: Vec<AnyElement> = self
            .children
            .into_iter()
            .map(|child| match child {
                MixerChild::Element(element) => element,
                MixerChild::Deferred(build) => build(&context, window, cx),
            })
            .collect();
        let mut areas = MixerAreas::sort(children);
        // `MixerEmpty` takes the channels' place while they are empty.
        let mut channels = std::mem::take(&mut areas.channels);
        if areas.channels_empty {
            channels.append(&mut areas.empty);
        }

        let layout = match orientation {
            Orientation::Horizontal => div()
                .flex()
                .flex_col()
                .gap(px(AREA_GAP))
                .child(cell(areas.header))
                .child(cell(channels))
                .child(cell(areas.separator))
                .child(cell(areas.master)),
            Orientation::Vertical => div()
                .flex()
                .flex_col()
                .gap(px(AREA_GAP))
                .child(cell(areas.header).w_full())
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .gap(px(AREA_GAP))
                        .child(cell(channels).flex_1())
                        .child(cell(areas.separator))
                        .child(cell(areas.master)),
                ),
        };
        let mut root = layout.id(self.id).min_w_0().children(areas.other);
        root.style().refine(&self.style);
        provide(context, provide(config, root))
    }
}

/// The top row: title and actions.
#[derive(Default)]
pub struct MixerHeader {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MixerHeader {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .min_w_0()
            .items_center()
            .gap(px(8.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(MixerHeader, LayoutArea::MixerHeader);

/// The mixer's name (`text-base font-medium`, pushes actions to the end).
#[derive(IntoElement)]
pub struct MixerTitle {
    text: SharedString,
    style: StyleRefinement,
}

impl MixerTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for MixerTitle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl gpui::RenderOnce for MixerTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .mr_auto()
            .text_base_()
            .font_weight(FontWeight::MEDIUM)
            .child(self.text);
        el.style().refine(&self.style);
        el
    }
}

/// Buttons in the header.
#[derive(IntoElement, Default)]
pub struct MixerActions {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MixerActions {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(MixerActions);

impl gpui::RenderOnce for MixerActions {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// Which way Ctrl/Cmd + `key` moves between strips: -1, +1, or not at all.
/// Rows move with up/down, console strips with left/right.
pub fn mixer_navigation_direction(
    orientation: Orientation,
    key: &str,
    ctrl_or_meta: bool,
) -> Option<isize> {
    if !ctrl_or_meta {
        return None;
    }
    let (previous, next) = match orientation {
        Orientation::Horizontal => ("up", "down"),
        Orientation::Vertical => ("left", "right"),
    };
    if key == previous {
        Some(-1)
    } else if key == next {
        Some(1)
    } else {
        None
    }
}

/// The control to focus on the neighbouring strip: the one at the same
/// position among the strip's focusable controls, else its first one.
/// `current` / `neighbour` are each strip's controls in tab order.
pub fn mixer_neighbour_target<'a, T: PartialEq>(
    current: &[T],
    neighbour: &'a [T],
    focused: &T,
) -> Option<&'a T> {
    current
        .iter()
        .position(|control| control == focused)
        .and_then(|index| neighbour.get(index))
        .or_else(|| neighbour.first())
}

/// Every tab stop of the window in order, found by walking `focus_next` from
/// the focused element (focus is restored afterwards; nothing is painted in
/// between, so no focus events fire).
fn tab_order(window: &mut Window, cx: &mut App) -> Vec<FocusHandle> {
    let start = window.focused(cx);
    let mut order: Vec<FocusHandle> = Vec::new();
    for _ in 0..MAX_TAB_STOPS {
        window.focus_next(cx);
        match window.focused(cx) {
            Some(handle) if !order.contains(&handle) => order.push(handle),
            _ => break,
        }
    }
    if let Some(start) = start {
        window.focus(&start, cx);
    }
    order
}

/// `focusNeighbour`: moves focus from the focused strip to the same control on
/// the strip `direction` away. Returns whether it moved.
fn focus_neighbour(
    registry: &StripRegistry,
    direction: isize,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let strips = registry.strips();
    let Some(focused) = window.focused(cx) else {
        return false;
    };
    let Some(index) = strips
        .iter()
        .position(|strip| strip.contains(&focused, window))
    else {
        return false;
    };
    let Some(neighbour) = index
        .checked_add_signed(direction)
        .and_then(|index| strips.get(index))
    else {
        return false;
    };
    let order = tab_order(window, cx);
    let controls = |strip: &FocusHandle| -> Vec<FocusHandle> {
        order
            .iter()
            .filter(|handle| strip.contains(handle, window))
            .cloned()
            .collect()
    };
    let current = controls(&strips[index]);
    let next = controls(neighbour);
    let Some(target) = mixer_neighbour_target(&current, &next, &focused) else {
        return false;
    };
    target.focus(window, cx);
    true
}

/// The strips, in a column (rows) or a row (console strips) that scrolls along
/// the strip axis when they overflow. Ctrl/Cmd+arrow moves between strips.
#[derive(Default)]
pub struct MixerChannels {
    scrollable: Option<bool>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MixerChannels {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll along the strip axis when strips overflow. Default true.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = Some(scrollable);
        self
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let orientation = use_mixer_context().map_or(Orientation::Horizontal, |m| m.orientation);
        let horizontal = orientation == Orientation::Horizontal;
        let scrollable = self.scrollable.unwrap_or(true);
        let empty = self.children.is_empty();
        let registry = StripRegistry::default();
        let keys = registry.clone();
        let mut el = div()
            .id("mixer-channels")
            .flex()
            .min_h_0()
            .min_w_0()
            .gap(px(MIXER_GAP))
            .map(|el| {
                if horizontal {
                    el.flex_col()
                } else {
                    el.flex_row()
                }
            })
            .when(scrollable, |el| {
                if horizontal {
                    el.overflow_y_scroll()
                } else {
                    el.overflow_x_scroll()
                }
            })
            .when(empty, |el| el.hidden())
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                let Some(direction) = mixer_navigation_direction(
                    orientation,
                    &event.keystroke.key,
                    modifiers.control || modifiers.platform,
                ) else {
                    return;
                };
                if focus_neighbour(&keys, direction, window, cx) {
                    cx.stop_propagation();
                }
            })
            .children(self.children);
        el.style().refine(&self.style);
        provide(registry, el)
    }
}

impl IntoElement for MixerChannels {
    type Element = AreaElement;
    fn into_element(self) -> AreaElement {
        let empty = self.children.is_empty();
        let mut element = AreaElement::new(LayoutArea::MixerChannels, move |window, cx| {
            self.render(window, cx).into_any_element()
        });
        element.empty = empty;
        element
    }
}
parent_styled!(MixerChannels);

/// A line between the channels and the master: horizontal under rows,
/// vertical beside console strips.
#[derive(Default)]
pub struct MixerSeparator {
    style: StyleRefinement,
}

impl MixerSeparator {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let horizontal =
            use_mixer_context().is_none_or(|mixer| mixer.orientation == Orientation::Horizontal);
        let mut el = div().flex_shrink_0().bg(cx.theme().border).map(|el| {
            if horizontal {
                el.h(px(1.0)).w_full()
            } else {
                el.h_full().w(px(1.0))
            }
        });
        el.style().refine(&self.style);
        el
    }
}

impl IntoElement for MixerSeparator {
    type Element = AreaElement;
    fn into_element(self) -> AreaElement {
        AreaElement::new(LayoutArea::MixerSeparator, move |window, cx| {
            self.render(window, cx).into_any_element()
        })
    }
}

impl Styled for MixerSeparator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The master bus area, after the separator.
#[derive(Default)]
pub struct MixerMaster {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MixerMaster {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let horizontal =
            use_mixer_context().is_none_or(|mixer| mixer.orientation == Orientation::Horizontal);
        let mut el = div()
            .flex()
            .min_h_0()
            .min_w_0()
            .map(|el| {
                if horizontal {
                    el.flex_col()
                } else {
                    el.flex_row()
                }
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(MixerMaster, LayoutArea::MixerMaster);

/// Shown in the channels' place when [`MixerChannels`] renders nothing.
///
/// ```ignore
/// .child(MixerChannels::new())
/// .child(MixerEmpty::new().child("No audio sources yet."))
/// ```
#[derive(Default)]
pub struct MixerEmpty {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MixerEmpty {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = div()
            .flex()
            .min_h(px(96.0))
            .items_center()
            .justify_center()
            .rounded(radius_xl(t))
            .border_1()
            .border_dashed()
            .border_color(t.border)
            .p(px(24.0))
            .text_center()
            .text_sm_()
            .text_color(t.muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(MixerEmpty, LayoutArea::MixerEmpty);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_meter_settings_with_its_strips() {
        let mixer = Mixer::new("mixer")
            .orientation(Orientation::Vertical)
            .size(AudioSize::Sm)
            .min_db(-48.0)
            .max_db(6.0)
            .disabled(true);
        let config = mixer.child_config();
        assert_eq!(config.orientation, Some(Orientation::Vertical));
        assert_eq!(config.size, Some(AudioSize::Sm));
        assert_eq!(config.min_db, Some(-48.0));
        assert_eq!(config.max_db, Some(6.0));
        assert!(config.disabled);
        assert!(!config.dimmed);
        // A strip inside inherits the orientation and the meter range.
        let strip = crate::ChannelStrip::new("a").config(config).muted(true);
        let strip_config = strip.child_config();
        assert_eq!(strip.resolved_orientation(), Orientation::Vertical);
        assert_eq!(strip_config.min_db, Some(-48.0));
        assert!(strip_config.dimmed);
    }

    #[test]
    fn defaults_to_rows() {
        let config = Mixer::new("mixer").child_config();
        assert_eq!(config.orientation, Some(Orientation::Horizontal));
        assert_eq!(config.min_db, None);
        assert!(!config.disabled);
    }

    #[test]
    fn shows_the_empty_state_without_channels() {
        assert!(mixer_empty_visible(&[true]));
        assert!(!mixer_empty_visible(&[false]));
        // No `MixerChannels` at all: nothing to be empty.
        assert!(!mixer_empty_visible(&[]));
        assert!(MixerChannels::new().into_element().empty);
        assert!(!MixerChannels::new().child(div()).into_element().empty);
    }

    #[test]
    fn moves_with_ctrl_arrows_along_the_strip_axis() {
        use Orientation::{Horizontal, Vertical};
        assert_eq!(mixer_navigation_direction(Vertical, "right", true), Some(1));
        assert_eq!(mixer_navigation_direction(Vertical, "left", true), Some(-1));
        assert_eq!(mixer_navigation_direction(Vertical, "down", true), None);
        assert_eq!(
            mixer_navigation_direction(Horizontal, "down", true),
            Some(1)
        );
        assert_eq!(mixer_navigation_direction(Horizontal, "up", true), Some(-1));
        assert_eq!(mixer_navigation_direction(Horizontal, "right", true), None);
        // Plain arrows belong to the focused control.
        assert_eq!(mixer_navigation_direction(Vertical, "right", false), None);
    }

    #[test]
    fn moves_focus_to_the_same_control_on_the_next_strip() {
        let a = ["a-volume", "a-mute", "a-solo"];
        let b = ["b-volume", "b-mute", "b-solo"];
        assert_eq!(
            mixer_neighbour_target(&a, &b, &"a-volume"),
            Some(&"b-volume")
        );
        assert_eq!(mixer_neighbour_target(&a, &b, &"a-solo"), Some(&"b-solo"));
        // No matching control: the neighbour's first one.
        assert_eq!(
            mixer_neighbour_target(&a, &b[..1], &"a-solo"),
            Some(&"b-volume")
        );
        assert_eq!(
            mixer_neighbour_target(&a, &b, &"strip-a"),
            Some(&"b-volume")
        );
        // Nothing focusable on the neighbour.
        assert_eq!(mixer_neighbour_target::<&str>(&a, &[], &"a-mute"), None);
    }
}
