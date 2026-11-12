//! Port of shadcn `components/ui/button.tsx` (Base UI `Button`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, Refineable, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::icons::Icon;
use super::styles::{TextSize, by_mode, fit_radius, focus_ring_color, radius_2xl, ring_overlay};
use crate::theme::{ActiveTheme, mix};

/// `variant` of [`Button`] (`buttonVariants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Default,
    Outline,
    Secondary,
    Ghost,
    Destructive,
    Link,
}

/// `size` of [`Button`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    #[default]
    Default,
    Xs,
    Sm,
    Lg,
    Icon,
    IconXs,
    IconSm,
    IconLg,
}

impl ButtonSize {
    /// Height (and width for the icon sizes).
    pub fn height(self) -> Pixels {
        px(match self {
            Self::Xs | Self::IconXs => 24.0,
            Self::Sm | Self::IconSm => 28.0,
            Self::Default | Self::Icon => 32.0,
            Self::Lg | Self::IconLg => 36.0,
        })
    }

    pub fn is_icon(self) -> bool {
        matches!(
            self,
            Self::Icon | Self::IconXs | Self::IconSm | Self::IconLg
        )
    }

    /// `[&_svg:not([class*='size-'])]:size-*`.
    pub fn icon_size(self) -> Pixels {
        match self {
            Self::Xs | Self::IconXs => px(12.0),
            _ => px(16.0),
        }
    }

    /// (padding-x, padding next to an inline icon, gap)
    fn padding(self) -> (f32, f32, f32) {
        match self {
            Self::Default => (12.0, 10.0, 6.0),
            Self::Xs => (10.0, 8.0, 4.0),
            Self::Sm => (12.0, 8.0, 4.0),
            Self::Lg => (16.0, 12.0, 6.0),
            _ => (0.0, 0.0, 6.0),
        }
    }
}

pub(crate) type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Colours of a [`ButtonVariant`] for the current theme.
pub(crate) struct VariantColors {
    pub bg: Option<Hsla>,
    pub fg: Option<Hsla>,
    pub hover_bg: Option<Hsla>,
    pub hover_fg: Option<Hsla>,
    pub border: Option<Hsla>,
    pub expanded_bg: Option<Hsla>,
    pub expanded_fg: Option<Hsla>,
    pub focus_border: Option<Hsla>,
    pub focus_ring: Option<Hsla>,
}

pub(crate) fn variant_colors(variant: ButtonVariant, cx: &App) -> VariantColors {
    let t = cx.theme();
    let mut c = VariantColors {
        bg: None,
        fg: None,
        hover_bg: None,
        hover_fg: None,
        border: None,
        expanded_bg: None,
        expanded_fg: None,
        focus_border: None,
        focus_ring: None,
    };
    match variant {
        ButtonVariant::Default => {
            c.bg = Some(t.primary);
            c.fg = Some(t.primary_foreground);
            c.hover_bg = Some(t.primary.opacity(0.8));
        }
        ButtonVariant::Destructive => {
            c.bg = Some(by_mode(
                t,
                t.destructive.opacity(0.1),
                t.destructive.opacity(0.2),
            ));
            c.fg = Some(t.destructive);
            c.hover_bg = Some(by_mode(
                t,
                t.destructive.opacity(0.2),
                t.destructive.opacity(0.3),
            ));
            c.focus_border = Some(t.destructive.opacity(0.4));
            c.focus_ring = Some(by_mode(
                t,
                t.destructive.opacity(0.2),
                t.destructive.opacity(0.4),
            ));
        }
        ButtonVariant::Ghost => {
            c.hover_bg = Some(by_mode(t, t.muted, t.muted.opacity(0.5)));
            c.hover_fg = Some(t.foreground);
            c.expanded_bg = Some(t.muted);
            c.expanded_fg = Some(t.foreground);
        }
        ButtonVariant::Link => {
            c.fg = Some(t.primary);
        }
        ButtonVariant::Outline => {
            c.border = Some(t.border);
            c.bg = Some(by_mode(t, t.background, gpui::transparent_black()));
            c.hover_bg = Some(by_mode(t, t.muted, t.input.opacity(0.3)));
            c.hover_fg = Some(t.foreground);
            c.expanded_bg = Some(t.muted);
            c.expanded_fg = Some(t.foreground);
        }
        ButtonVariant::Secondary => {
            c.bg = Some(t.secondary);
            c.fg = Some(t.secondary_foreground);
            c.hover_bg = Some(mix(t.secondary, t.foreground, 0.05));
            c.expanded_bg = Some(t.secondary);
            c.expanded_fg = Some(t.secondary_foreground);
        }
    }
    c
}

struct ButtonState {
    focus: FocusHandle,
}

/// A button, with shadcn's variants and sizes.
///
/// Children are the label (any element); icons go through
/// [`Button::icon_start`] / [`Button::icon_end`] (`data-icon="inline-start"`)
/// so they get the size's icon size and padding, or `.child(Icon::new(..))`.
///
/// ```ignore
/// Button::new("save").child("Save")
/// Button::new("play").variant(ButtonVariant::Ghost).size(ButtonSize::Icon)
///     .icon(IconName::Play).on_click(|_, _, _| play())
/// ```
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    invalid: bool,
    expanded: bool,
    has_popup: bool,
    icon_start: Option<Icon>,
    icon_end: Option<Icon>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_click: Option<ClickHandler>,
    focus_handle: Option<FocusHandle>,
    tab_index: Option<isize>,
    aria_label: Option<gpui::SharedString>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            variant: ButtonVariant::Default,
            size: ButtonSize::Default,
            disabled: false,
            invalid: false,
            expanded: false,
            has_popup: false,
            icon_start: None,
            icon_end: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
            on_click: None,
            focus_handle: None,
            tab_index: None,
            aria_label: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `aria-invalid`: destructive border and ring.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// `aria-expanded` (set by popover / menu triggers).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// `aria-haspopup`: disables the `active:translate-y-px` press nudge.
    pub fn has_popup(mut self, has_popup: bool) -> Self {
        self.has_popup = has_popup;
        self
    }

    /// Leading icon (`data-icon="inline-start"`), sized for the button.
    pub fn icon_start(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_start = Some(icon.into());
        self
    }

    /// Trailing icon (`data-icon="inline-end"`), sized for the button.
    pub fn icon_end(mut self, icon: impl Into<Icon>) -> Self {
        self.icon_end = Some(icon.into());
        self
    }

    /// The icon of an icon-size button (same as [`Button::icon_start`]).
    pub fn icon(self, icon: impl Into<Icon>) -> Self {
        self.icon_start(icon)
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Use an external focus handle (e.g. to return focus to a trigger).
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus_handle = Some(focus.clone());
        self
    }

    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = Some(index);
        self
    }

    /// `aria-label` (accessibility name for icon buttons).
    pub fn aria_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let trigger = super::styles::use_context::<super::overlay::OverlayTrigger>();
        let focus_handle = self
            .focus_handle
            .clone()
            .or_else(|| trigger.as_ref().map(|t| t.focus.clone()));
        let expanded = self.expanded || trigger.as_ref().is_some_and(|t| t.expanded);
        let has_popup = self.has_popup || trigger.is_some();
        let focus = match focus_handle {
            Some(f) => f,
            None => window
                .use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<ButtonState>| {
                    ButtonState {
                        focus: cx.focus_handle().tab_stop(true),
                    }
                })
                .read(cx)
                .focus
                .clone(),
        };
        let focus = match self.tab_index {
            Some(i) => focus.tab_index(i),
            None => focus,
        };
        let focus_visible =
            !self.disabled && focus.is_focused(window) && window.last_input_was_keyboard();

        let colors = variant_colors(self.variant, cx);
        let t = cx.theme();
        let size = self.size;
        let height = size.height();
        let radius = fit_radius(radius_2xl(t), height);
        let (pad, pad_icon, gap) = size.padding();

        let mut border = colors.border.unwrap_or(gpui::transparent_black());
        if focus_visible {
            border = colors.focus_border.unwrap_or(t.ring);
        }
        if self.invalid {
            border = by_mode(t, t.destructive, t.destructive.opacity(0.5));
        }
        let ring = if self.invalid {
            focus_ring_color(t, false, true, 0.3)
        } else if focus_visible {
            colors.focus_ring.or(Some(t.ring.opacity(0.3)))
        } else {
            None
        };

        let mut bg = colors.bg;
        let mut fg = colors.fg;
        if expanded {
            bg = colors.expanded_bg.or(bg);
            fg = colors.expanded_fg.or(fg);
        }
        let hover_bg = colors.hover_bg;
        let hover_fg = colors.hover_fg;
        let is_link = self.variant == ButtonVariant::Link;
        let icon_size = size.icon_size();
        let has_start = self.icon_start.is_some();
        let has_end = self.icon_end.is_some();
        let disabled = self.disabled;
        let nudge = !has_popup && !disabled;

        let mut el = div()
            .id(self.id)
            .group("audiocn-button")
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .h(height)
            .gap(px(gap))
            .rounded(radius)
            .border_1()
            .border_color(border)
            .text_sm_()
            .when(size == ButtonSize::Xs, |el| el.text_xs_())
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .when_some(fg, |el, fg| el.text_color(fg))
            .when_some(hover_fg.filter(|_| !disabled), |el, c| {
                el.hover(move |s| s.text_color(c))
            })
            .when(is_link && !disabled, |el| el.hover(|s| s.underline()))
            .map(|el| {
                if size.is_icon() {
                    el.w(height)
                } else {
                    el.pl(px(if has_start { pad_icon } else { pad }))
                        .pr(px(if has_end { pad_icon } else { pad }))
                }
            })
            .when(nudge, |el| el.active(|s| s.top(px(1.0))))
            .when(disabled, |el| el.opacity(0.5))
            .when_some(self.aria_label, |el, l| el.aria_label(l))
            .role(gpui::Role::Button)
            // `bg-clip-padding`: the background sits inside the 1px border.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .rounded(radius - px(1.0))
                    .when_some(bg, |el, bg| el.bg(bg))
                    .when_some(hover_bg.filter(|_| !disabled), |el, c| {
                        el.group_hover("audiocn-button", move |s| s.bg(c))
                    }),
            )
            .when_some(ring, |el, ring| el.child(ring_overlay(radius, 3.0, ring)))
            .when_some(self.icon_start, |el, icon| el.child(icon.size(icon_size)))
            .children(self.children)
            .when_some(self.icon_end, |el, icon| el.child(icon.size(icon_size)));

        el.style().refine(&self.style);

        if disabled {
            return el;
        }
        let el = el.track_focus(&focus);
        match self.on_click {
            Some(handler) => el.on_click(move |e, w, cx| handler(e, w, cx)),
            None => el,
        }
    }
}
