//! Port of `components/ui/track-list.tsx`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, FontWeight, ImageSource, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ObjectFit, ParentElement, Pixels, Refineable,
    RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled, StyledImage,
    Window, div, img, prelude::FluentBuilder, px,
};

use crate::motion::{Easing, Instant};
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::ActiveTheme;
use crate::ui::styles::{
    TextSize, provide, radius_lg, radius_md, radius_xl, ring_overlay, use_context,
};

/// `size` of [`TrackList`] (`trackListVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TrackListSize {
    #[default]
    Default,
    Sm,
    Lg,
}

impl TrackListSize {
    /// `--track-row-height`.
    pub fn row_height(self) -> Pixels {
        px(match self {
            Self::Default => 48.0,
            Self::Lg => 56.0,
            Self::Sm => 36.0,
        })
    }

    /// The gap between rows.
    pub fn gap(self) -> Pixels {
        px(match self {
            Self::Default => 2.0,
            Self::Lg => 4.0,
            Self::Sm => 0.0,
        })
    }
}

/// `variant` of [`TrackList`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TrackListVariant {
    #[default]
    Default,
    /// `rounded-xl border p-1`.
    Outline,
}

/// The keys [`TrackList`] moves focus with (`moveFocus`): the index of the
/// row to focus among `count` enabled rows, from the focused row `index`.
pub fn track_list_target(key: &str, index: usize, count: usize) -> Option<usize> {
    let target = match key {
        "down" => index.checked_add(1)?,
        "up" => index.checked_sub(1)?,
        "end" => count.checked_sub(1)?,
        "home" => 0,
        _ => return None,
    };
    (target < count).then_some(target)
}

#[derive(Clone)]
struct TrackListContext {
    size: TrackListSize,
    /// Enabled rows in render order (`ITEM_SELECTOR`).
    items: Rc<RefCell<Vec<FocusHandle>>>,
}

/// A list of tracks with active and playing states. Up/Down move focus between
/// rows, Home/End jump to the first or last.
///
/// ```ignore
/// TrackList::new("queue").variant(TrackListVariant::Outline).child(
///     TrackListItem::new("night-drive").active(true).on_select(|_, _| play())
///         .child(TrackListItemIndex::new().child("1"))
///         .child(TrackListItemContent::new().child(TrackListItemTitle::new().child("Night Drive"))),
/// )
/// ```
#[derive(IntoElement)]
pub struct TrackList {
    id: ElementId,
    size: TrackListSize,
    variant: TrackListVariant,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TrackList {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: TrackListSize::Default,
            variant: TrackListVariant::Default,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Default `Default` (3rem rows).
    pub fn size(mut self, size: TrackListSize) -> Self {
        self.size = size;
        self
    }

    pub fn variant(mut self, variant: TrackListVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl ParentElement for TrackList {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for TrackList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TrackList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let items = Rc::new(RefCell::new(Vec::new()));
        let context = TrackListContext {
            size: self.size,
            items: items.clone(),
        };
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_col()
            .w_full()
            .gap(self.size.gap())
            .when(self.variant == TrackListVariant::Outline, |el| {
                el.rounded(radius_xl(t))
                    .border_1()
                    .border_color(t.border)
                    .p(px(4.0))
            })
            .role(gpui::Role::List)
            .children(self.children)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if window.default_prevented() {
                    return;
                }
                let items = items.borrow();
                // Only when a row itself has focus, not a control inside it.
                let Some(index) = items.iter().position(|item| item.is_focused(window)) else {
                    return;
                };
                if let Some(next) = track_list_target(&event.keystroke.key, index, items.len()) {
                    window.prevent_default();
                    cx.stop_propagation();
                    window.focus(&items[next], cx);
                }
            });
        el.style().refine(&self.style);
        provide(context, el)
    }
}

#[derive(Clone, Copy, Default)]
struct TrackListItemContext {
    active: bool,
    playing: bool,
}

struct TrackListItemState {
    focus: FocusHandle,
}

type SelectHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// One row. `active` marks the current track, `playing` the current track
/// while it plays. Click, Enter or Space calls `on_select`.
///
/// ```ignore
/// TrackListItem::new("low-tide").active(true).playing(true).on_select(|_, _| toggle())
/// ```
#[derive(IntoElement)]
pub struct TrackListItem {
    id: ElementId,
    active: bool,
    playing: bool,
    disabled: bool,
    on_select: Option<SelectHandler>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl TrackListItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            active: false,
            playing: false,
            disabled: false,
            on_select: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// The current track.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The current track, playing.
    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Click, Enter or Space.
    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl ParentElement for TrackListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for TrackListItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TrackListItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let list = use_context::<TrackListContext>();
        let row_height = list
            .as_ref()
            .map_or(TrackListSize::Default, |l| l.size)
            .row_height();
        let focus = window
            .use_keyed_state(
                self.id.clone(),
                cx,
                |_, cx: &mut Context<TrackListItemState>| TrackListItemState {
                    focus: cx.focus_handle().tab_stop(true),
                },
            )
            .read(cx)
            .focus
            .clone();
        let disabled = self.disabled;
        if !disabled && let Some(list) = &list {
            list.items.borrow_mut().push(focus.clone());
        }

        let t = cx.theme();
        let radius = radius_lg(t);
        let focus_visible =
            !disabled && focus.is_focused(window) && window.last_input_was_keyboard();
        let hover_bg = t.muted.opacity(0.6);
        let active_bg = t.muted;
        let context = TrackListItemContext {
            active: self.active,
            playing: self.playing,
        };

        let mut el = div()
            .id(self.id)
            .relative()
            .flex()
            .min_h(row_height)
            .items_center()
            .gap(px(12.0))
            .rounded(radius)
            .px(px(8.0))
            .text_sm_()
            .role(gpui::Role::ListItem)
            .when(self.active, |el| el.bg(active_bg))
            .when(!self.active && !disabled, |el| {
                el.hover(move |s| s.bg(hover_bg))
            })
            .when(disabled, |el| el.opacity(0.5))
            .when(focus_visible, |el| {
                el.child(ring_overlay(radius, 3.0, t.ring.opacity(0.3)))
            })
            .children(self.children);
        el.style().refine(&self.style);
        if !disabled {
            let on_select = self.on_select;
            el = el.track_focus(&focus).on_click(move |_, window, cx| {
                if let Some(on_select) = &on_select {
                    on_select(window, cx);
                }
            });
        }
        provide(context, el)
    }
}

macro_rules! part {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(IntoElement, Default)]
        pub struct $name {
            children: Vec<AnyElement>,
            style: StyleRefinement,
        }

        impl $name {
            pub fn new() -> Self {
                Self::default()
            }
        }

        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }
    };
}

part!(
    /// The track number; three pulsing bars while the row is playing.
    ///
    /// ```ignore
    /// TrackListItemIndex::new().child("1")
    /// ```
    TrackListItemIndex
);

/// `animate-pulse` (2s `cubic-bezier(0.4, 0, 0.6, 1)`, opacity 1 → 0.5 → 1)
/// at `elapsed` seconds.
pub fn pulse_opacity(elapsed: f32) -> f32 {
    const PULSE: Easing = Easing::CubicBezier(0.4, 0.0, 0.6, 1.0);
    let phase = (elapsed / 2.0).rem_euclid(1.0);
    if phase < 0.5 {
        1.0 - 0.5 * PULSE.apply(phase * 2.0)
    } else {
        0.5 + 0.5 * PULSE.apply((phase - 0.5) * 2.0)
    }
}

fn pulse_epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

impl RenderOnce for TrackListItemIndex {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let item = use_context::<TrackListItemContext>().unwrap_or_default();
        let t = cx.theme();
        let color = if item.active {
            t.primary
        } else {
            t.muted_foreground
        };
        let mut el = div()
            .flex()
            .w(px(20.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .font_family(t.font_mono.clone())
            .text_xs_()
            .text_color(color);
        if item.playing {
            let animate = !prefers_reduced_motion(cx);
            let elapsed = Instant::now()
                .saturating_duration_since(pulse_epoch())
                .as_secs_f32();
            if animate {
                window.request_animation_frame();
            }
            let bar = |height: f32, delay: f32| {
                let opacity = if animate {
                    pulse_opacity(elapsed - delay)
                } else {
                    1.0
                };
                div()
                    .w(px(2.0))
                    .h(px(height))
                    .rounded_full()
                    .bg(color.opacity(color.a * opacity))
            };
            el = el.child(
                div()
                    .id("playing")
                    .flex()
                    .h(px(12.0))
                    .items_end()
                    .gap(px(1.0))
                    .aria_label("Playing")
                    .role(gpui::Role::Image)
                    .child(bar(12.0, 0.0))
                    .child(bar(8.0, 0.2))
                    .child(bar(10.0, 0.4)),
            );
        } else {
            el = el.children(self.children);
        }
        el.style().refine(&self.style);
        el
    }
}

/// The cover image of a row (`size-9 rounded-md`). Decorative unless `alt` is set.
///
/// ```ignore
/// TrackListItemArtwork::new("https://example.com/cover.png")
/// ```
#[derive(IntoElement)]
pub struct TrackListItemArtwork {
    src: ImageSource,
    alt: SharedString,
    style: StyleRefinement,
}

impl TrackListItemArtwork {
    pub fn new(src: impl Into<ImageSource>) -> Self {
        Self {
            src: src.into(),
            alt: SharedString::default(),
            style: StyleRefinement::default(),
        }
    }

    /// Alternative text. Default "" (decorative).
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = alt.into();
        self
    }
}

impl Styled for TrackListItemArtwork {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TrackListItemArtwork {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = img(self.src)
            .size(px(36.0))
            .flex_shrink_0()
            .rounded(radius_md(t))
            .bg(t.muted)
            .object_fit(ObjectFit::Cover);
        if !self.alt.is_empty() {
            el = el.aria_label(self.alt);
        }
        el.style().refine(&self.style);
        el
    }
}

part!(
    /// Title and description (`flex min-w-0 flex-1 flex-col`).
    ///
    /// ```ignore
    /// TrackListItemContent::new().child(TrackListItemTitle::new().child("Arcade"))
    /// ```
    TrackListItemContent
);

impl RenderOnce for TrackListItemContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .min_w_0()
            .flex_1()
            .flex_col()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

part!(
    /// The track title (`truncate font-medium`, primary when active).
    ///
    /// ```ignore
    /// TrackListItemTitle::new().child("Night Drive")
    /// ```
    TrackListItemTitle
);

impl RenderOnce for TrackListItemTitle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let item = use_context::<TrackListItemContext>().unwrap_or_default();
        let t = cx.theme();
        let mut el = div()
            .truncate()
            .font_weight(FontWeight::MEDIUM)
            .when(item.active, |el| el.text_color(t.primary))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

part!(
    /// The artist (`text-muted-foreground truncate text-xs`).
    ///
    /// ```ignore
    /// TrackListItemDescription::new().child("audiocn")
    /// ```
    TrackListItemDescription
);

impl RenderOnce for TrackListItemDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = div()
            .truncate()
            .text_xs_()
            .text_color(t.muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

part!(
    /// The track length (`font-mono text-xs`).
    ///
    /// ```ignore
    /// TrackListItemDuration::new().child(format_time(19.2, Default::default()))
    /// ```
    TrackListItemDuration
);

impl RenderOnce for TrackListItemDuration {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = div()
            .flex_shrink_0()
            .font_family(t.font_mono.clone())
            .text_xs_()
            .text_color(t.muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

part!(
    /// Row buttons (`flex shrink-0 items-center gap-1`). Clicking them doesn't
    /// select the row.
    ///
    /// ```ignore
    /// TrackListItemActions::new().child(Button::new("more").size(ButtonSize::IconXs).icon(IconName::DotsThree))
    /// ```
    TrackListItemActions
);

impl RenderOnce for TrackListItemActions {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div().flex().flex_shrink_0().items_center().gap(px(4.0));
        for (index, child) in self.children.into_iter().enumerate() {
            // A click on a control (`button, a, input`) is not a row click.
            el = el.child(
                div()
                    .id(("track-list-item-action", index))
                    .flex()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(child),
            );
        }
        el.style().refine(&self.style);
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_with_the_arrow_keys() {
        assert_eq!(track_list_target("down", 0, 2), Some(1));
        assert_eq!(track_list_target("down", 1, 2), None);
        assert_eq!(track_list_target("up", 1, 2), Some(0));
        assert_eq!(track_list_target("up", 0, 2), None);
        assert_eq!(track_list_target("home", 2, 3), Some(0));
        assert_eq!(track_list_target("end", 0, 3), Some(2));
        assert_eq!(track_list_target("enter", 0, 3), None);
    }

    #[test]
    fn sizes_rows() {
        assert_eq!(TrackListSize::Default.row_height(), px(48.0));
        assert_eq!(TrackListSize::Lg.row_height(), px(56.0));
        assert_eq!(TrackListSize::Sm.row_height(), px(36.0));
    }

    #[test]
    fn pulses_between_full_and_half_opacity() {
        assert!((pulse_opacity(0.0) - 1.0).abs() < 1e-4);
        assert!((pulse_opacity(1.0) - 0.5).abs() < 1e-4);
        assert!((pulse_opacity(2.0) - 1.0).abs() < 1e-4);
    }
}
