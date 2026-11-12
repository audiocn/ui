//! Port of `components/ui/channel-strip.tsx`: one channel of a mixer, as a
//! row or a console strip, composed from the parts you need.
//!
//! # Passing config down (the container pattern)
//!
//! React's `ChannelStrip` wraps its children in an `AudioConfigProvider`
//! (`{ dimmed: muted || dimmed, disabled, orientation, size }`, merged over
//! the mixer's), so a `<LevelMeter>` inside a muted strip dims itself and a
//! fader inside a console strip turns vertical. GPUI has no context, so the
//! config travels explicitly, and every audiocn component that reads
//! `useAudioConfig()` in React takes `.config(AudioConfig)`. A strip gives you
//! that config in two ways:
//!
//! 1. **`child_with`** (preferred). The closure runs when the strip renders,
//!    with a [`ChannelStripContext`] whose `config` is final: the strip's own
//!    settings merged over its mixer's (zones, ballistics, dB range, size).
//!
//!    ```ignore
//!    ChannelStrip::new("mic")
//!        .muted(muted)
//!        .child(ChannelStripHeader::new().child(ChannelStripTitle::new("Mic")))
//!        .child_with(|strip, _, _| {
//!            ChannelStripMeter::new()
//!                .child(LevelMeter::new("mic-level").config(strip.config.clone()).source(src))
//!        })
//!    ```
//!
//! 2. **`child_config()`** on the builder, for code that builds children up
//!    front. It knows only what was set on the builder (including
//!    [`ChannelStrip::config`], the parent's config), so set `muted`, `dimmed`,
//!    `orientation`... first, and pass the mixer's
//!    [`Mixer::child_config`](crate::Mixer::child_config) to `.config()`.
//!
//! Shortcuts that do (1) for you: [`ChannelStrip::meter`],
//! [`ChannelStrip::fader`], [`ChannelStrip::controls`].
//!
//! Containers further down (blocks, custom strips) should follow the same
//! shape: a `.config(parent)` setter, a `child_config()` getter and a
//! `child_with` closure. Inside the strip, the [`AudioConfig`] and the
//! [`ChannelStripContext`] are also published to descendants that render
//! during layout (the crate's `ui::styles::use_context`), which is how the
//! parts below find their orientation, and how a component can pick up config
//! it was not handed: [`use_channel_strip`], [`use_inherited_audio_config`].
//!
//! # Layout
//!
//! React places the parts on a CSS grid by `grid-area`, whatever order they
//! come in. The parts here do the same: each one tells the strip which area it
//! fills, the strip sorts them into the grid of its orientation (and, for a
//! row, of its width: the header moves beside the meter from a 36rem strip
//! on), and the fader row exists only when a [`ChannelStripFader`] does. Any
//! other child goes below the grid.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Entity, FocusHandle, FontWeight, GlobalElementId,
    Hsla, InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels,
    Refineable, SharedString, StyleRefinement, Styled, Window, canvas, div, font,
    prelude::FluentBuilder, px,
};

use crate::audio::types::Orientation;
use crate::config::{AudioConfig, AudioSize};
use crate::theme::ActiveTheme;
use crate::ui::icons::Icon;
use crate::ui::styles::{
    TextSize, provide, radius_lg, radius_xl, ring_overlay, ring_overlay_bordered, shadow_xs,
    use_context,
};
use crate::ui::{Badge, BadgeVariant};

use super::mixer::StripRegistry;

/// `@xl/channel-strip`: a row strip at least this wide (content box) puts its
/// header beside the meter.
const WIDE_ROW_MIN: f32 = 576.0;
/// `--channel-strip-header-width` default: 12rem.
const HEADER_WIDTH: f32 = 192.0;
/// `--channel-strip-width` default: 6.5rem.
const CONSOLE_WIDTH: f32 = 104.0;
/// `min-h-72`.
const CONSOLE_MIN_HEIGHT: f32 = 288.0;
const GAP_X: f32 = 12.0;
const GAP_Y: f32 = 6.0;

// ---------------------------------------------------------------------------
// Grid areas

/// The grid area a strip or mixer part fills (`[grid-area:*]`).
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutArea {
    StripHeader,
    StripMeter,
    StripFader,
    StripValue,
    StripControls,
    StripNotice,
    MixerHeader,
    MixerChannels,
    MixerSeparator,
    MixerMaster,
    MixerEmpty,
}

type DeferredRender = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>;

/// The element every area part renders as: it carries the area so the parent
/// can place it (read with `AnyElement::downcast_mut`), and renders the part
/// lazily during layout, inside the parent's context.
#[doc(hidden)]
pub struct AreaElement {
    pub(crate) area: LayoutArea,
    /// `MixerChannels` only: it has no children (`:empty`).
    pub(crate) empty: bool,
    render: Option<DeferredRender>,
}

impl AreaElement {
    pub(crate) fn new(
        area: LayoutArea,
        render: impl FnOnce(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            area,
            empty: false,
            render: Some(Box::new(render)),
        }
    }

    /// The area of `element` when it is an area part.
    pub(crate) fn area_of(element: &mut AnyElement) -> Option<(LayoutArea, bool)> {
        element
            .downcast_mut::<AreaElement>()
            .map(|part| (part.area, part.empty))
    }
}

impl IntoElement for AreaElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for AreaElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let render = self.render.take().expect("AreaElement laid out twice");
        let mut child = render(window, cx);
        (child.request_layout(window, cx), child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

/// Implements `IntoElement` for an area part with an inherent
/// `fn render(self, &mut Window, &mut App) -> impl IntoElement`, and
/// `ParentElement` + `Styled` over its `children` / `style` fields.
macro_rules! area_part {
    ($ty:ty, $area:expr) => {
        impl gpui::IntoElement for $ty {
            type Element = AreaElement;
            fn into_element(self) -> AreaElement {
                AreaElement::new($area, move |window, cx| {
                    self.render(window, cx).into_any_element()
                })
            }
        }
        parent_styled!($ty);
    };
}

macro_rules! parent_styled {
    ($ty:ty) => {
        impl gpui::ParentElement for $ty {
            fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
                self.children.extend(elements);
            }
        }
        impl gpui::Styled for $ty {
            fn style(&mut self) -> &mut gpui::StyleRefinement {
                &mut self.style
            }
        }
    };
}
pub(crate) use area_part;
pub(crate) use parent_styled;

/// The `grid-template-areas` a strip lays its parts out on
/// (`channelStripLayoutVariants`), one string per row. A row strip has a
/// fader row only when it has a fader, a notice row only with a notice, and
/// puts its header beside the meter when `wide` (`@xl/channel-strip`).
pub fn channel_strip_template_areas(
    orientation: Orientation,
    wide: bool,
    has_fader: bool,
    has_notice: bool,
) -> Vec<&'static str> {
    let mut rows = Vec::new();
    match (orientation, wide) {
        (Orientation::Vertical, _) => {
            return vec![
                "header header header header",
                ". meter fader .",
                "value value value value",
                "controls controls controls controls",
                "notice notice notice notice",
            ];
        }
        (Orientation::Horizontal, false) => {
            rows.push("header header header");
            rows.push("meter value controls");
            if has_fader {
                rows.push("fader value controls");
            }
            if has_notice {
                rows.push("notice notice notice");
            }
        }
        (Orientation::Horizontal, true) => {
            rows.push("header meter value controls");
            if has_fader {
                rows.push("header fader value controls");
            }
            if has_notice {
                rows.push("notice notice notice notice");
            }
        }
    }
    rows
}

// ---------------------------------------------------------------------------
// ChannelStrip

/// `variant` of [`ChannelStrip`] (`channelStripVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelStripVariant {
    /// `bg-muted/40 rounded-xl p-3`.
    #[default]
    Default,
    /// `bg-card rounded-xl border p-3 shadow-xs`.
    Card,
    /// `p-2`, no surface.
    Ghost,
    /// `bg-muted/60 rounded-xl border p-3 ring-1 ring-primary/10`, for the master bus.
    Master,
}

/// The state of the surrounding channel strip, for custom parts
/// (`useChannelStrip`), plus the config the strip passes to its children.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelStripContext {
    pub orientation: Orientation,
    pub muted: bool,
    pub solo: bool,
    /// Silenced by another channel's solo (the `dimmed` prop).
    pub dimmed: bool,
    /// What the strip's `AudioConfigProvider` gives its children: dimmed when
    /// muted or dimmed, the strip's orientation, size and disabled state,
    /// over the mixer's config. Pass it to `.config()` of every component inside.
    pub config: AudioConfig,
}

/// The state of the surrounding channel strip (`useChannelStrip`). Call it
/// while rendering a part inside a [`ChannelStrip`]; `None` outside one
/// (React throws there).
pub fn use_channel_strip() -> Option<ChannelStripContext> {
    use_context::<ChannelStripContext>()
}

/// The [`AudioConfig`] published by the nearest [`ChannelStrip`] or
/// [`Mixer`](crate::Mixer) around the element that is rendering, or the
/// default config outside one (`useAudioConfig`). Only valid during render.
pub fn use_inherited_audio_config() -> AudioConfig {
    use_context::<AudioConfig>().unwrap_or_default()
}

type ChildBuilder = Box<dyn FnOnce(&ChannelStripContext, &mut Window, &mut App) -> AnyElement>;

enum StripChild {
    Element(AnyElement),
    Deferred(ChildBuilder),
}

/// One channel as a row (`horizontal`, the default) or a console strip
/// (`vertical`), built from [`ChannelStripHeader`], [`ChannelStripMeter`],
/// [`ChannelStripFader`], [`ChannelStripValue`], [`ChannelStripControls`] and
/// [`ChannelStripNotice`]. See the module docs for passing its config to the
/// components inside.
///
/// ```ignore
/// ChannelStrip::new("mic")
///     .muted(muted)
///     .solo(solo)
///     .child(ChannelStripHeader::new().child(ChannelStripTitle::new("Microphone")))
///     .meter(LevelMeter::new("mic-level").size(AudioSize::Sm).source(signal.meter))
///     .fader(Fader::new("mic-gain").size(AudioSize::Sm).value(gain_db))
///     .child(ChannelStripValue::new().child(format_db(gain_db)))
/// ```
#[derive(IntoElement)]
pub struct ChannelStrip {
    id: ElementId,
    orientation: Option<Orientation>,
    variant: ChannelStripVariant,
    size: Option<AudioSize>,
    muted: bool,
    solo: bool,
    dimmed: bool,
    selected: bool,
    disabled: Option<bool>,
    accent: Option<Hsla>,
    header_width: Pixels,
    config: Option<AudioConfig>,
    children: Vec<StripChild>,
    style: StyleRefinement,
}

impl ChannelStrip {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: None,
            variant: ChannelStripVariant::Default,
            size: None,
            muted: false,
            solo: false,
            dimmed: false,
            selected: false,
            disabled: None,
            accent: None,
            header_width: px(HEADER_WIDTH),
            config: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Horizontal is a row; vertical is a console strip. Unset, it follows the
    /// config (a mixer's), else horizontal.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Default [`ChannelStripVariant::Default`].
    pub fn variant(mut self, variant: ChannelStripVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Size passed to the components inside. Unset, it follows the config.
    pub fn size(mut self, size: AudioSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    pub fn solo(mut self, solo: bool) -> Self {
        self.solo = solo;
        self
    }

    /// Silenced by another channel's solo.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// `ring-2 ring-ring/40`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Disables every control inside (and fades the strip to 60%). Unset, it
    /// follows the config.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// The channel's colour tag: a 2px bar on the leading edge (left of a
    /// row, top of a console strip).
    pub fn accent(mut self, accent: impl Into<Hsla>) -> Self {
        self.accent = Some(accent.into());
        self
    }

    /// `--channel-strip-header-width`: the header column of a wide row.
    /// Default 12rem.
    pub fn header_width(mut self, width: Pixels) -> Self {
        self.header_width = width;
        self
    }

    /// The parent's config (a mixer's [`Mixer::child_config`](crate::Mixer::child_config)).
    /// Inside a [`Mixer`](crate::Mixer) it is inherited anyway; this one is
    /// merged over the inherited one.
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// The orientation this strip renders with, as far as the builder knows.
    pub fn resolved_orientation(&self) -> Orientation {
        resolve_orientation(self.orientation, self.config.as_ref())
    }

    /// What the strip passes to the components inside, as far as the builder
    /// knows (its own settings over [`ChannelStrip::config`]). Prefer
    /// [`ChannelStrip::child_with`], which also sees an inherited mixer config.
    pub fn child_config(&self) -> AudioConfig {
        self.context(self.config.clone().unwrap_or_default()).config
    }

    /// The [`ChannelStripContext`] parts would see, as far as the builder knows.
    pub fn strip_context(&self) -> ChannelStripContext {
        self.context(self.config.clone().unwrap_or_default())
    }

    /// Adds a child built when the strip renders, from the strip's final
    /// [`ChannelStripContext`] (use `strip.config` for `.config()`).
    pub fn child_with<E: IntoElement>(
        mut self,
        build: impl FnOnce(&ChannelStripContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.children
            .push(StripChild::Deferred(Box::new(move |strip, window, cx| {
                build(strip, window, cx).into_any_element()
            })));
        self
    }

    /// `<ChannelStripMeter>{meter}</ChannelStripMeter>`, with the strip's
    /// config applied to the meter.
    pub fn meter(self, meter: crate::LevelMeter) -> Self {
        self.child_with(move |strip, _, _| {
            ChannelStripMeter::new().child(meter.config(strip.config.clone()))
        })
    }

    /// `<ChannelStripFader>{fader}</ChannelStripFader>`, with the strip's
    /// config applied to the fader.
    pub fn fader(self, fader: crate::Fader) -> Self {
        self.child_with(move |strip, _, _| {
            ChannelStripFader::new().child(fader.config(strip.config.clone()))
        })
    }

    /// `<ChannelStripControls>` around channel toggles, with the strip's config
    /// applied to each.
    pub fn controls(self, toggles: impl IntoIterator<Item = crate::ChannelToggle>) -> Self {
        let toggles: Vec<_> = toggles.into_iter().collect();
        self.child_with(move |strip, _, _| {
            ChannelStripControls::new().children(
                toggles
                    .into_iter()
                    .map(|toggle| toggle.config(strip.config.clone())),
            )
        })
    }

    fn context(&self, parent: AudioConfig) -> ChannelStripContext {
        let orientation = resolve_orientation(self.orientation, Some(&parent));
        let size = self.size.or(parent.size).unwrap_or_default();
        let disabled = self.disabled.unwrap_or(parent.disabled);
        let own = AudioConfig {
            dimmed: self.muted || self.dimmed,
            disabled,
            orientation: Some(orientation),
            size: Some(size),
            ..Default::default()
        };
        ChannelStripContext {
            orientation,
            muted: self.muted,
            solo: self.solo,
            dimmed: self.dimmed,
            config: own.merged_over(&parent),
        }
    }
}

fn resolve_orientation(prop: Option<Orientation>, config: Option<&AudioConfig>) -> Orientation {
    prop.or(config.and_then(|config| config.orientation))
        .unwrap_or_default()
}

impl ParentElement for ChannelStrip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children
            .extend(elements.into_iter().map(StripChild::Element));
    }
}

impl Styled for ChannelStrip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct ChannelStripState {
    /// Tracks focus inside the strip, for the mixer's Ctrl+arrow navigation.
    /// Not a tab stop.
    focus: FocusHandle,
    /// Last measured width of the padding box (the `@container` query).
    width: Option<Pixels>,
}

/// The parts sorted by grid area.
#[derive(Default)]
struct StripAreas {
    header: Vec<AnyElement>,
    meter: Vec<AnyElement>,
    fader: Vec<AnyElement>,
    value: Vec<AnyElement>,
    controls: Vec<AnyElement>,
    notice: Vec<AnyElement>,
    other: Vec<AnyElement>,
}

impl StripAreas {
    fn sort(children: Vec<AnyElement>) -> Self {
        let mut areas = Self::default();
        for mut child in children {
            let bucket = match AreaElement::area_of(&mut child).map(|(area, _)| area) {
                Some(LayoutArea::StripHeader) => &mut areas.header,
                Some(LayoutArea::StripMeter) => &mut areas.meter,
                Some(LayoutArea::StripFader) => &mut areas.fader,
                Some(LayoutArea::StripValue) => &mut areas.value,
                Some(LayoutArea::StripControls) => &mut areas.controls,
                Some(LayoutArea::StripNotice) => &mut areas.notice,
                _ => &mut areas.other,
            };
            bucket.push(child);
        }
        areas
    }
}

/// One grid cell: the parts placed on an area.
fn cell(parts: Vec<AnyElement>) -> gpui::Div {
    div().flex().flex_col().children(parts)
}

impl gpui::RenderOnce for ChannelStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let inherited = use_context::<AudioConfig>().unwrap_or_default();
        let parent = match &self.config {
            Some(config) => config.merged_over(&inherited),
            None => inherited,
        };
        let context = self.context(parent);
        let orientation = context.orientation;
        let disabled = context.config.disabled;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| ChannelStripState {
            focus: cx.focus_handle(),
            width: None,
        });
        let (focus, width) = {
            let state = state.read(cx);
            (state.focus.clone(), state.width)
        };
        if let Some(registry) = use_context::<StripRegistry>() {
            registry.register(focus.clone());
        }

        let children: Vec<AnyElement> = self
            .children
            .into_iter()
            .map(|child| match child {
                StripChild::Element(element) => element,
                StripChild::Deferred(build) => build(&context, window, cx),
            })
            .collect();
        let areas = StripAreas::sort(children);

        let t = cx.theme();
        let (padding, border) = match self.variant {
            ChannelStripVariant::Ghost => (8.0, 0.0),
            ChannelStripVariant::Default => (12.0, 0.0),
            ChannelStripVariant::Card | ChannelStripVariant::Master => (12.0, 1.0),
        };
        let wide = orientation == Orientation::Horizontal
            && width.is_some_and(|width| is_wide(width, padding));
        let layout = match orientation {
            Orientation::Horizontal if wide => wide_row_layout(areas, self.header_width),
            Orientation::Horizontal => row_layout(areas),
            Orientation::Vertical => console_layout(areas),
        };

        let radius = radius_xl(t);
        let mut root = div()
            .id(self.id)
            .track_focus(&focus)
            .relative()
            .min_w_0()
            .p(px(padding))
            .map(|el| match orientation {
                Orientation::Horizontal => el.w_full(),
                Orientation::Vertical => el
                    .flex()
                    .flex_col()
                    .h_full()
                    .min_h(px(CONSOLE_MIN_HEIGHT))
                    .w(px(CONSOLE_WIDTH))
                    .flex_shrink_0(),
            })
            .map(|el| match self.variant {
                ChannelStripVariant::Default => el.bg(t.muted.opacity(0.4)).rounded(radius),
                ChannelStripVariant::Card => el
                    .bg(t.card)
                    .text_color(t.card_foreground)
                    .rounded(radius)
                    .border(px(border))
                    .border_color(t.border)
                    .shadow(shadow_xs()),
                ChannelStripVariant::Ghost => el,
                ChannelStripVariant::Master => el
                    .bg(t.muted.opacity(0.6))
                    .rounded(radius)
                    .border(px(border))
                    .border_color(t.border)
                    // `ring-1 ring-primary/10` sits outside the border. A plain
                    // overlay is placed from the padding box, so it covers the
                    // 1px border and the master reads as a single line.
                    .child(ring_overlay_bordered(
                        radius,
                        1.0,
                        border,
                        t.primary.opacity(0.1),
                    )),
            })
            .when(self.selected, |el| {
                let ring = t.ring.opacity(0.4);
                if border > 0.0 {
                    el.child(ring_overlay_bordered(radius, 2.0, border, ring))
                } else {
                    el.child(ring_overlay(radius, 2.0, ring))
                }
            })
            .when(disabled, |el| el.opacity(0.6))
            .when_some(self.accent, |el, accent| {
                let bar = div().absolute().rounded_full().bg(accent);
                el.child(match orientation {
                    Orientation::Horizontal => {
                        bar.top(px(12.0)).bottom(px(12.0)).left_0().w(px(2.0))
                    }
                    Orientation::Vertical => bar.left(px(12.0)).right(px(12.0)).top_0().h(px(2.0)),
                })
            })
            .child(provide(
                context.clone(),
                provide(context.config.clone(), layout),
            ))
            .when(orientation == Orientation::Horizontal, |el| {
                el.child(measure(state.clone(), padding))
            });
        root.style().refine(&self.style);
        root
    }
}

/// Whether a row strip of this padding-box width is past `@xl/channel-strip`.
fn is_wide(width: Pixels, padding: f32) -> bool {
    f32::from(width) - padding * 2.0 >= WIDE_ROW_MIN
}

/// Records the strip's width after layout and re-renders when it crosses the
/// wide-row breakpoint (the `@container` query).
fn measure(state: Entity<ChannelStripState>, padding: f32) -> impl IntoElement {
    canvas(
        move |bounds, _, cx| {
            let width = bounds.size.width;
            state.update(cx, |state, cx| {
                // Unmeasured strips render narrow.
                let was_wide = state
                    .width
                    .replace(width)
                    .is_some_and(|old| is_wide(old, padding));
                if was_wide != is_wide(width, padding) {
                    cx.notify();
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A narrow row: header on top; meter (and fader) beside value and controls;
/// notice below.
fn row_layout(areas: StripAreas) -> AnyElement {
    let has_notice = !areas.notice.is_empty();
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap_y(px(GAP_Y))
        .child(cell(areas.header).w_full())
        .child(meter_value_row(
            None,
            areas.meter,
            areas.fader,
            areas.value,
            areas.controls,
        ))
        .when(has_notice, |el| el.child(cell(areas.notice).w_full()))
        .children(areas.other)
        .into_any_element()
}

/// A wide row: header, meter (and fader), value and controls side by side.
fn wide_row_layout(areas: StripAreas, header_width: Pixels) -> AnyElement {
    let has_notice = !areas.notice.is_empty();
    let header = cell(areas.header)
        .w(header_width)
        .min_w_0()
        .flex_shrink(1.0);
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap_y(px(GAP_Y))
        .child(meter_value_row(
            Some(header),
            areas.meter,
            areas.fader,
            areas.value,
            areas.controls,
        ))
        .when(has_notice, |el| el.child(cell(areas.notice).w_full()))
        .children(areas.other)
        .into_any_element()
}

/// The meter / fader column beside the value and controls.
///
/// With a fader, CSS grid puts the meter and the fader on their own rows and
/// lets the header, value and controls span both (`items-center`). The rows
/// grow to that spanning height, so the meter and the fader are centred in
/// their halves instead of stacking at the track gap. Without a fader the
/// row is a single centred line.
fn meter_value_row(
    header: Option<gpui::Div>,
    meter: Vec<AnyElement>,
    fader: Vec<AnyElement>,
    value: Vec<AnyElement>,
    controls: Vec<AnyElement>,
) -> gpui::Div {
    let has_fader = !fader.is_empty();
    let mut row = div().flex().w_full().gap_x(px(GAP_X));
    row = if has_fader {
        row.items_stretch()
    } else {
        row.items_center()
    };
    if let Some(header) = header {
        row = row.child(header.when(has_fader, |el| el.self_center()));
    }
    row.child(meter_column(meter, fader, has_fader))
        .child(cell(value).when(has_fader, |el| el.self_center()))
        .child(cell(controls).when(has_fader, |el| el.self_center()))
}

/// The `minmax(0,1fr)` column holding the meter row and the fader row.
fn meter_column(meter: Vec<AnyElement>, fader: Vec<AnyElement>, has_fader: bool) -> gpui::Div {
    if !has_fader {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(cell(meter).w_full());
    }
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_y(px(GAP_Y))
        .child(cell(meter).w_full().flex_1().justify_center())
        .child(cell(fader).w_full().flex_1().justify_center())
}

/// A console strip: header, then meter and fader side by side filling the
/// height, then value, controls and notice, each centred.
fn console_layout(areas: StripAreas) -> AnyElement {
    let row = |parts: Vec<AnyElement>| div().w_full().flex().justify_center().children(parts);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .w_full()
        .gap_y(px(GAP_Y))
        .child(cell(areas.header).w_full().items_center())
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .justify_center()
                .gap_x(px(GAP_X))
                .child(cell(areas.meter).h_full())
                .child(cell(areas.fader).h_full()),
        )
        .child(row(areas.value))
        .child(row(areas.controls))
        .child(cell(areas.notice).w_full())
        .children(areas.other)
        .into_any_element()
}

fn strip_orientation() -> Orientation {
    use_channel_strip().map_or(Orientation::Horizontal, |strip| strip.orientation)
}

// ---------------------------------------------------------------------------
// Parts

/// The top of a strip: icon, text, status and actions (`[grid-area:header]`).
/// Wraps in a row; stacks and centres in a console strip.
#[derive(Default)]
pub struct ChannelStripHeader {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripHeader {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let vertical = strip_orientation() == Orientation::Vertical;
        let mut el = div()
            .flex()
            .min_w_0()
            .items_center()
            .gap(px(8.0))
            .map(|el| {
                if vertical {
                    el.w_full().flex_col().text_center()
                } else {
                    el.flex_wrap()
                }
            })
            // `*:max-w-full` on the console header. The wrapper is a full-width
            // row so `ml-auto` on the actions (and `w_full` + `justify_end`
            // below) reaches the header's right edge. A shrink-wrapped row
            // has no free space, and the switch stays centred.
            .children(self.children.into_iter().map(|child| {
                if vertical {
                    div()
                        .w_full()
                        .max_w_full()
                        .flex()
                        .justify_center()
                        .child(child)
                        .into_any_element()
                } else {
                    child
                }
            }));
        el.style().refine(&self.style);
        el
    }
}
area_part!(ChannelStripHeader, LayoutArea::StripHeader);

/// A small tile for the channel's icon (`size-8`, `bg-background`, `shadow-xs`).
///
/// ```ignore
/// ChannelStripIcon::new().child(Icon::new(IconName::Microphone))
/// ```
#[derive(IntoElement, Default)]
pub struct ChannelStripIcon {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripIcon {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(ChannelStripIcon);

impl gpui::RenderOnce for ChannelStripIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = div()
            .flex()
            .size(px(32.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(radius_lg(t))
            .bg(t.background)
            .text_color(t.muted_foreground)
            .shadow(shadow_xs())
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The channel name (`text-sm font-medium`, truncated).
#[derive(IntoElement)]
pub struct ChannelStripTitle {
    text: SharedString,
    style: StyleRefinement,
}

impl ChannelStripTitle {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for ChannelStripTitle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl gpui::RenderOnce for ChannelStripTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .min_w_0()
            .truncate()
            .text_size(px(14.0))
            .line_height(px(17.5))
            .font_weight(FontWeight::MEDIUM)
            .child(self.text);
        el.style().refine(&self.style);
        el
    }
}

/// A secondary line under the title (`text-xs text-muted-foreground`, truncated).
#[derive(IntoElement)]
pub struct ChannelStripDescription {
    text: SharedString,
    style: StyleRefinement,
}

impl ChannelStripDescription {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for ChannelStripDescription {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl gpui::RenderOnce for ChannelStripDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .min_w_0()
            .truncate()
            .text_color(cx.theme().muted_foreground)
            .text_size(px(12.0))
            .line_height(px(15.0))
            .child(self.text);
        el.style().refine(&self.style);
        el
    }
}

/// Groups a title and description so they stack next to an icon.
#[derive(IntoElement, Default)]
pub struct ChannelStripText {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripText {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(ChannelStripText);

impl gpui::RenderOnce for ChannelStripText {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let horizontal = strip_orientation() == Orientation::Horizontal;
        let mut el = div()
            .flex()
            .min_w_0()
            .flex_col()
            .gap(px(2.0))
            .when(horizontal, |el| {
                // `flex-1 min-w-20` inside a wrapping header. A zero basis
                // shrinks the title (and truncates it) before the status
                // wraps; CSS clamps the flex base to `min-w-20` first, so the
                // badge wraps once icon + 5rem + badge no longer fit.
                el.min_w(px(80.0))
                    .flex_grow(1.0)
                    .flex_shrink(1.0)
                    .flex_basis(px(80.0))
            })
            .when(!horizontal, |el| el.flex_1())
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `tone` of [`ChannelStripStatus`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelStripStatusTone {
    #[default]
    Default,
    Live,
    Muted,
    Warning,
    Error,
}

/// A status badge in the header ("Live", "Muted", "Blocked").
///
/// ```ignore
/// ChannelStripStatus::new().tone(ChannelStripStatusTone::Live).child("Live")
/// ```
#[derive(IntoElement, Default)]
pub struct ChannelStripStatus {
    tone: ChannelStripStatusTone,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripStatus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default [`ChannelStripStatusTone::Default`].
    pub fn tone(mut self, tone: ChannelStripStatusTone) -> Self {
        self.tone = tone;
        self
    }
}
parent_styled!(ChannelStripStatus);

impl gpui::RenderOnce for ChannelStripStatus {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let variant = match self.tone {
            ChannelStripStatusTone::Error => BadgeVariant::Destructive,
            ChannelStripStatusTone::Default => BadgeVariant::Secondary,
            _ => BadgeVariant::Outline,
        };
        let tone = match self.tone {
            ChannelStripStatusTone::Live => Some((t.meter_ok.opacity(0.15), t.meter_ok_foreground)),
            ChannelStripStatusTone::Muted => {
                Some((t.channel_mute.opacity(0.15), t.channel_mute_foreground))
            }
            ChannelStripStatusTone::Warning => Some((t.meter_warn.opacity(0.2), t.foreground)),
            _ => None,
        };
        let mut badge = Badge::new()
            .variant(variant)
            .flex_shrink_0()
            // The bundled face is a little narrower than the site's, so a
            // 192px header keeps "Blocked" (66px on the site) on the title's
            // line. Two extra pixels of padding restores that width: the
            // badge then wraps, while the narrower "Paused" still fits.
            .pl(px(10.0))
            .pr(px(10.0))
            .when_some(tone, |badge, (bg, fg)| badge.bg(bg).text_color(fg))
            .children(self.children);
        badge.style().refine(&self.style);
        badge
    }
}

/// Buttons at the end of the header (`ml-auto`).
#[derive(IntoElement, Default)]
pub struct ChannelStripActions {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripActions {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(ChannelStripActions);

impl gpui::RenderOnce for ChannelStripActions {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // A row header is `ml-auto` at the end of the flex line. A console
        // header is a column that centres each child in a full-width row, so
        // the actions span that row and pack to its end (`ml-auto` on the
        // cross axis of the React column).
        let vertical = strip_orientation() == Orientation::Vertical;
        let mut el = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(4.0))
            .map(|el| {
                if vertical {
                    el.w_full().justify_end()
                } else {
                    el.ml_auto()
                }
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// The meter slot (`[grid-area:meter]`): full width in a row, full height in
/// a console strip.
#[derive(Default)]
pub struct ChannelStripMeter {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripMeter {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = slot_div(strip_orientation()).children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(ChannelStripMeter, LayoutArea::StripMeter);

fn slot_div(orientation: Orientation) -> gpui::Div {
    let el = div().flex().min_h_0().min_w_0();
    match orientation {
        Orientation::Horizontal => el.w_full().items_center(),
        Orientation::Vertical => el.h_full().justify_center(),
    }
}

/// The fader slot (`[grid-area:fader]`). Its row exists only when a strip has
/// one.
#[derive(Default)]
pub struct ChannelStripFader {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripFader {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let orientation = strip_orientation();
        let only_child = self.children.len() == 1;
        let mut el = slot_div(orientation).children(self.children);
        el.style().refine(&self.style);
        provide(ChannelStripFaderSlot { only_child }, el)
    }
}
area_part!(ChannelStripFader, LayoutArea::StripFader);

/// Published to a fader rendering inside a [`ChannelStripFader`]. React's
/// slot reaches into the fader with `[&_[data-slot=fader-control]]:p-0
/// [&_[data-slot=fader-scale]]:p-0`, and when the control is the slot's only
/// child, a margin of `(track - thumb) / 2` across the strip axis, so the
/// fader row is only as tall as its track. A fader reads this with
/// [`use_channel_strip_fader_slot`] and drops that padding itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelStripFaderSlot {
    /// The fader is the slot's only child (`:only-child`).
    pub only_child: bool,
}

/// `Some` while rendering inside a [`ChannelStripFader`].
pub fn use_channel_strip_fader_slot() -> Option<ChannelStripFaderSlot> {
    use_context::<ChannelStripFaderSlot>()
}

/// The readout slot (`[grid-area:value]`): mono, muted, at least `8ch` wide
/// so dragging a fader never resizes the row.
#[derive(Default)]
pub struct ChannelStripValue {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripValue {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mono = t.font_mono.clone();
        let size = px(12.0);
        let font_id = window.text_system().resolve_font(&font(mono.clone()));
        let ch = window
            .text_system()
            .ch_width(font_id, size)
            .unwrap_or(px(7.2));
        let vertical = strip_orientation() == Orientation::Vertical;
        let mut el = div()
            .flex()
            .min_w(ch * 8.0)
            .items_center()
            .map(|el| {
                if vertical {
                    el.justify_center()
                } else {
                    el.justify_end()
                }
            })
            .text_color(t.muted_foreground)
            .font_family(mono)
            .text_size(size)
            .line_height(px(16.0))
            .whitespace_nowrap()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(ChannelStripValue, LayoutArea::StripValue);

/// The toggles slot (`[grid-area:controls]`): mute, solo, monitor.
#[derive(Default)]
pub struct ChannelStripControls {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripControls {
    pub fn new() -> Self {
        Self::default()
    }

    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
area_part!(ChannelStripControls, LayoutArea::StripControls);

/// `variant` of [`ChannelStripNotice`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelStripNoticeVariant {
    #[default]
    Default,
    Destructive,
    Warning,
}

/// A full-width message under the strip (`[grid-area:notice]`): blocked
/// access, a paused source.
///
/// ```ignore
/// ChannelStripNotice::new()
///     .variant(ChannelStripNoticeVariant::Warning)
///     .icon(IconName::Warning)
///     .child(div().flex_1().child("Paused to prevent an echo."))
/// ```
#[derive(Default)]
pub struct ChannelStripNotice {
    variant: ChannelStripNoticeVariant,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl ChannelStripNotice {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default [`ChannelStripNoticeVariant::Default`].
    pub fn variant(mut self, variant: ChannelStripNoticeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Adds an icon child at the notice's `size-3.5` (14px).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.children
            .push(icon.into().size(px(14.0)).into_any_element());
        self
    }

    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let (bg, fg) = match self.variant {
            ChannelStripNoticeVariant::Default => (t.muted, t.muted_foreground),
            ChannelStripNoticeVariant::Destructive => (t.destructive.opacity(0.1), t.destructive),
            ChannelStripNoticeVariant::Warning => (t.meter_warn.opacity(0.15), t.foreground),
        };
        let el = div()
            .flex()
            .w_full()
            .min_w_0()
            .items_center()
            .gap(px(8.0))
            .rounded(radius_lg(t))
            .px(px(10.0))
            .py(px(6.0))
            .text_xs_()
            .bg(bg)
            .text_color(fg);
        // CSS lets a flex item's text shrink and wrap (`min-w-0`); GPUI
        // measures text's min-content as the whole line. In a narrow console
        // strip that would overflow into the neighbouring strips, so there each
        // child gets a shrinkable wrapper and its text wraps inside it. Rows
        // keep the children as they are (so an `ml_auto` action still works).
        let el = if strip_orientation() == Orientation::Vertical {
            el.children(
                self.children
                    .into_iter()
                    .map(|child| div().min_w_0().child(child)),
            )
        } else {
            el.children(self.children)
        };
        let mut el = el;
        el.style().refine(&self.style);
        el
    }
}
area_part!(ChannelStripNotice, LayoutArea::StripNotice);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::ballistics::BallisticsInput;

    #[test]
    fn passes_its_orientation_and_dimming_down() {
        let strip = ChannelStrip::new("mic")
            .muted(true)
            .orientation(Orientation::Vertical);
        let config = strip.child_config();
        assert_eq!(config.orientation, Some(Orientation::Vertical));
        assert!(config.dimmed);
        assert_eq!(strip.resolved_orientation(), Orientation::Vertical);
        assert!(strip.strip_context().muted);
    }

    #[test]
    fn dims_children_when_silenced_by_another_solo() {
        let strip = ChannelStrip::new("music").dimmed(true);
        assert!(strip.child_config().dimmed);
        assert!(!ChannelStrip::new("music").child_config().dimmed);
    }

    #[test]
    fn exposes_its_state_to_custom_parts() {
        let context = ChannelStrip::new("mic").solo(true).strip_context();
        assert!(context.solo);
        assert!(!context.muted);
        assert_eq!(context.orientation, Orientation::Horizontal);
    }

    #[test]
    fn inherits_orientation_size_and_meter_settings_from_a_mixer() {
        let mixer = AudioConfig {
            orientation: Some(Orientation::Vertical),
            size: Some(AudioSize::Sm),
            min_db: Some(-48.0),
            ballistics: Some(BallisticsInput::default()),
            disabled: true,
            ..Default::default()
        };
        let strip = ChannelStrip::new("mic").config(mixer.clone());
        assert_eq!(strip.resolved_orientation(), Orientation::Vertical);
        let config = strip.child_config();
        assert_eq!(config.size, Some(AudioSize::Sm));
        assert_eq!(config.min_db, Some(-48.0));
        assert_eq!(config.ballistics, mixer.ballistics);
        assert!(config.disabled);
        // Explicit props win over the mixer's.
        let row = ChannelStrip::new("mic")
            .config(mixer)
            .orientation(Orientation::Horizontal)
            .size(AudioSize::Lg);
        assert_eq!(
            row.child_config().orientation,
            Some(Orientation::Horizontal)
        );
        assert_eq!(row.child_config().size, Some(AudioSize::Lg));
    }

    #[test]
    fn disables_its_children() {
        assert!(
            ChannelStrip::new("a")
                .disabled(true)
                .child_config()
                .disabled
        );
        assert!(!ChannelStrip::new("a").child_config().disabled);
    }

    #[test]
    fn reserves_a_fader_row_only_when_the_strip_has_a_fader() {
        // Without a fader, the stacked and the wide layout are one meter row.
        assert_eq!(
            channel_strip_template_areas(Orientation::Horizontal, false, false, false),
            ["header header header", "meter value controls"]
        );
        assert_eq!(
            channel_strip_template_areas(Orientation::Horizontal, true, false, false),
            ["header meter value controls"]
        );
        assert_eq!(
            channel_strip_template_areas(Orientation::Horizontal, false, true, false),
            [
                "header header header",
                "meter value controls",
                "fader value controls"
            ]
        );
        assert_eq!(
            channel_strip_template_areas(Orientation::Horizontal, true, true, true),
            [
                "header meter value controls",
                "header fader value controls",
                "notice notice notice notice"
            ]
        );
        assert_eq!(
            channel_strip_template_areas(Orientation::Horizontal, false, false, true),
            [
                "header header header",
                "meter value controls",
                "notice notice notice"
            ]
        );
        // A console strip always has its five rows.
        assert_eq!(
            channel_strip_template_areas(Orientation::Vertical, false, false, false).len(),
            5
        );
    }
}
