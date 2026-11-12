//! Port of `components/ui/channel-toggle.tsx`: mute, solo and monitor toggle
//! buttons with their own pressed colours (`--channel-mute`, `--channel-solo`,
//! `--channel-monitor`), built on Base UI's `Toggle`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, Pixels, Refineable, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::config::AudioConfig;
use crate::theme::{ActiveTheme, Theme};
use crate::ui::icons::Icon;
use crate::ui::styles::{TextSize, fit_radius, radius_lg, ring_overlay, ring_overlay_bordered};

/// `tone` of a [`ChannelToggle`]: which colour it takes when pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelToggleTone {
    /// `bg-foreground text-background` when pressed.
    #[default]
    Neutral,
    /// `--channel-mute` when pressed.
    Mute,
    /// `--channel-solo` when pressed.
    Solo,
    /// `--channel-monitor` when pressed.
    Monitor,
}

impl ChannelToggleTone {
    /// The `data-tone` attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Mute => "mute",
            Self::Solo => "solo",
            Self::Monitor => "monitor",
        }
    }
}

/// `variant` of a [`ChannelToggle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelToggleVariant {
    /// `bg-muted text-muted-foreground hover:text-foreground`.
    #[default]
    Default,
    /// `text-muted-foreground hover:bg-muted hover:text-foreground`.
    Ghost,
    /// `border border-border bg-transparent`, `hover:bg-muted`.
    Outline,
}

/// `size` of a [`ChannelToggle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelToggleSize {
    /// `h-7 min-w-7 px-1.5`.
    #[default]
    Default,
    /// `size-7`.
    Icon,
    /// `h-8 min-w-8 px-2`.
    Lg,
    /// `h-6 min-w-6 px-1`.
    Sm,
}

impl ChannelToggleSize {
    /// Height (and minimum width).
    pub fn height(self) -> Pixels {
        px(match self {
            Self::Sm => 24.0,
            Self::Default | Self::Icon => 28.0,
            Self::Lg => 32.0,
        })
    }

    /// Horizontal padding.
    pub fn padding_x(self) -> Pixels {
        px(match self {
            Self::Sm => 4.0,
            Self::Default => 6.0,
            Self::Lg => 8.0,
            Self::Icon => 0.0,
        })
    }
}

/// The resolved look of a [`ChannelToggle`] (`channelToggleVariants`): the
/// classes the cva produces, as colours for the current theme. Colours that
/// are `None` are not set (transparent / inherited).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelToggleStyle {
    pub height: Pixels,
    pub min_width: Pixels,
    /// `Some` only for [`ChannelToggleSize::Icon`] (`size-7`).
    pub width: Option<Pixels>,
    pub padding_x: Pixels,
    pub radius: Pixels,
    pub bg: Option<Hsla>,
    pub fg: Hsla,
    /// Border colour; only drawn (1px) for [`ChannelToggleVariant::Outline`].
    pub border: Option<Hsla>,
    pub hover_bg: Option<Hsla>,
    pub hover_fg: Option<Hsla>,
}

/// Port of the exported `channelToggleVariants` cva: the style of a toggle
/// with the given size, tone and variant, pressed or not.
pub fn channel_toggle_variants(
    theme: &Theme,
    size: ChannelToggleSize,
    tone: ChannelToggleTone,
    variant: ChannelToggleVariant,
    pressed: bool,
) -> ChannelToggleStyle {
    let t = theme;
    let height = size.height();
    let mut style = ChannelToggleStyle {
        height,
        min_width: height,
        width: (size == ChannelToggleSize::Icon).then_some(height),
        padding_x: size.padding_x(),
        radius: fit_radius(radius_lg(t), height),
        bg: None,
        fg: t.muted_foreground,
        border: None,
        hover_bg: None,
        hover_fg: Some(t.foreground),
    };
    match variant {
        ChannelToggleVariant::Default => style.bg = Some(t.muted),
        ChannelToggleVariant::Ghost => style.hover_bg = Some(t.muted),
        ChannelToggleVariant::Outline => {
            style.border = Some(t.border);
            style.hover_bg = Some(t.muted);
        }
    }
    if pressed {
        // `data-pressed:*` comes after `hover:*` in Tailwind's variant order,
        // so the pressed colours win over hover ones.
        let (border, bg, fg) = match tone {
            ChannelToggleTone::Neutral => (None, t.foreground, t.background),
            ChannelToggleTone::Mute => (
                Some(t.channel_mute.opacity(0.4)),
                t.channel_mute.opacity(0.15),
                t.channel_mute_foreground,
            ),
            ChannelToggleTone::Solo => (
                Some(t.channel_solo.opacity(0.5)),
                t.channel_solo.opacity(0.2),
                t.channel_solo_foreground,
            ),
            ChannelToggleTone::Monitor => (
                Some(t.channel_monitor.opacity(0.4)),
                t.channel_monitor.opacity(0.15),
                t.channel_monitor_foreground,
            ),
        };
        if style.border.is_some() {
            style.border = border.or(style.border);
        }
        style.bg = Some(bg);
        style.fg = fg;
        style.hover_bg = None;
        style.hover_fg = None;
    }
    style
}

type PressedHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

struct ChannelToggleState {
    pressed: bool,
    focus: FocusHandle,
}

/// A compact two-state channel button (`<ChannelToggle>`): neutral, mute,
/// solo or monitor tone; default, outline or ghost variant; four sizes.
/// Controlled with [`ChannelToggle::pressed`], uncontrolled with
/// [`ChannelToggle::default_pressed`]; Space / Enter toggle it when focused.
/// `disabled` falls back to the [`AudioConfig`]'s.
///
/// ```ignore
/// ChannelToggle::new("arm")
///     .aria_label("Record arm")
///     .default_pressed(true)
///     .variant(ChannelToggleVariant::Outline)
///     .child("R")
/// ```
#[derive(IntoElement)]
pub struct ChannelToggle {
    id: ElementId,
    tone: ChannelToggleTone,
    variant: ChannelToggleVariant,
    size: ChannelToggleSize,
    pressed: Option<bool>,
    default_pressed: bool,
    disabled: Option<bool>,
    config: AudioConfig,
    aria_label: Option<SharedString>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_pressed_change: Option<PressedHandler>,
}

impl ChannelToggle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tone: ChannelToggleTone::Neutral,
            variant: ChannelToggleVariant::Default,
            size: ChannelToggleSize::Default,
            pressed: None,
            default_pressed: false,
            disabled: None,
            config: AudioConfig::default(),
            aria_label: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
            on_pressed_change: None,
        }
    }

    /// Pressed colour. Default [`ChannelToggleTone::Neutral`].
    pub fn tone(mut self, tone: ChannelToggleTone) -> Self {
        self.tone = tone;
        self
    }

    /// Default [`ChannelToggleVariant::Default`].
    pub fn variant(mut self, variant: ChannelToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Default [`ChannelToggleSize::Default`].
    pub fn size(mut self, size: ChannelToggleSize) -> Self {
        self.size = size;
        self
    }

    /// Controlled pressed state.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Initial pressed state when uncontrolled. Default `false`.
    pub fn default_pressed(mut self, pressed: bool) -> Self {
        self.default_pressed = pressed;
        self
    }

    /// Disables the toggle. Unset, it follows [`AudioConfig::disabled`].
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Settings from a container (`useAudioConfig`).
    pub fn config(mut self, config: AudioConfig) -> Self {
        self.config = config;
        self
    }

    /// Accessible name (`aria-label`).
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Adds an icon child at the default `size-3.5` (14px). Use
    /// `.child(icon)` to keep the icon's own size.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.children
            .push(icon.into().size(px(14.0)).into_any_element());
        self
    }

    /// Called with the next pressed state (`onPressedChange`).
    pub fn on_pressed_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_pressed_change = Some(Rc::new(f));
        self
    }
}

impl ParentElement for ChannelToggle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for ChannelToggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ChannelToggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self.pressed.unwrap_or(self.default_pressed);
        let state = window.use_keyed_state(
            self.id.clone(),
            cx,
            |_, cx: &mut Context<ChannelToggleState>| ChannelToggleState {
                pressed: initial,
                focus: cx.focus_handle().tab_stop(true),
            },
        );
        if let Some(p) = self.pressed {
            state.update(cx, |s, _| s.pressed = p);
        }
        let (pressed, focus) = {
            let s = state.read(cx);
            (s.pressed, s.focus.clone())
        };
        let disabled = self.disabled.unwrap_or(self.config.disabled);
        let focus_visible =
            !disabled && focus.is_focused(window) && window.last_input_was_keyboard();

        let t = cx.theme();
        let s = channel_toggle_variants(t, self.size, self.tone, self.variant, pressed);
        let ring = focus_visible.then(|| t.ring.opacity(0.3));

        let mut el = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(s.height)
            .min_w(s.min_width)
            .when_some(s.width, |el, w| el.w(w))
            .px(s.padding_x)
            .rounded(s.radius)
            .when_some(s.border, |el, c| el.border_1().border_color(c))
            .when_some(s.bg, |el, c| el.bg(c))
            .text_color(s.fg)
            .text_xs_()
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .when(!disabled, |el| {
                el.hover(move |st| {
                    let st = match s.hover_bg {
                        Some(c) => st.bg(c),
                        None => st,
                    };
                    match s.hover_fg {
                        Some(c) => st.text_color(c),
                        None => st,
                    }
                })
            })
            .when(disabled, |el| el.opacity(0.5))
            .role(gpui::Role::Button)
            .aria_toggled(if pressed {
                gpui::Toggled::True
            } else {
                gpui::Toggled::False
            })
            .when_some(self.aria_label, |el, l| el.aria_label(l))
            .when_some(ring, |el, ring| {
                el.child(if s.border.is_some() {
                    ring_overlay_bordered(s.radius, 3.0, 1.0, ring)
                } else {
                    ring_overlay(s.radius, 3.0, ring)
                })
            })
            .children(self.children);
        el.style().refine(&self.style);
        if disabled {
            return el;
        }
        let controlled = self.pressed.is_some();
        let handler = self.on_pressed_change;
        el.track_focus(&focus).on_click(move |_, window, cx| {
            let next = !state.read(cx).pressed;
            if !controlled {
                state.update(cx, |s, cx| {
                    s.pressed = next;
                    cx.notify();
                });
            }
            if let Some(h) = handler.as_ref() {
                h(next, window, cx);
            }
        })
    }
}

/// `<MuteToggle>`: a [`ChannelToggle`] with the mute tone and
/// `aria-label="Mute"`.
///
/// ```ignore
/// MuteToggle::new("mute").pressed(muted).on_pressed_change(|m, _, _| set_muted(m)).child("M")
/// ```
pub struct MuteToggle;

impl MuteToggle {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<ElementId>) -> ChannelToggle {
        ChannelToggle::new(id)
            .aria_label("Mute")
            .tone(ChannelToggleTone::Mute)
    }
}

/// `<SoloToggle>`: a [`ChannelToggle`] with the solo tone and
/// `aria-label="Solo"`.
///
/// ```ignore
/// SoloToggle::new("solo").child("S")
/// ```
pub struct SoloToggle;

impl SoloToggle {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<ElementId>) -> ChannelToggle {
        ChannelToggle::new(id)
            .aria_label("Solo")
            .tone(ChannelToggleTone::Solo)
    }
}

/// `<MonitorToggle>`: a [`ChannelToggle`] with the monitor tone and
/// `aria-label="Monitor"`.
///
/// ```ignore
/// MonitorToggle::new("cue").size(ChannelToggleSize::Icon).icon(IconName::Headphones)
/// ```
pub struct MonitorToggle;

impl MonitorToggle {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<ElementId>) -> ChannelToggle {
        ChannelToggle::new(id)
            .aria_label("Monitor")
            .tone(ChannelToggleTone::Monitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_toggle_presets_set_tone_and_label() {
        let m = MuteToggle::new("m");
        assert_eq!(m.tone, ChannelToggleTone::Mute);
        assert_eq!(m.aria_label.as_deref(), Some("Mute"));
        let s = SoloToggle::new("s");
        assert_eq!(s.tone, ChannelToggleTone::Solo);
        assert_eq!(s.aria_label.as_deref(), Some("Solo"));
        let c = MonitorToggle::new("c").aria_label("Cue");
        assert_eq!(c.tone, ChannelToggleTone::Monitor);
        assert_eq!(c.aria_label.as_deref(), Some("Cue"));
    }

    #[test]
    fn channel_toggle_defaults() {
        let t = ChannelToggle::new("x");
        assert_eq!(t.tone, ChannelToggleTone::Neutral);
        assert_eq!(t.variant, ChannelToggleVariant::Default);
        assert_eq!(t.size, ChannelToggleSize::Default);
        assert_eq!(t.tone.as_str(), "neutral");
    }

    #[test]
    fn channel_toggle_pressed_tones_use_channel_tokens() {
        let theme = Theme::light();
        let style = |tone, variant, pressed| {
            channel_toggle_variants(&theme, ChannelToggleSize::Default, tone, variant, pressed)
        };
        let off = style(
            ChannelToggleTone::Mute,
            ChannelToggleVariant::Default,
            false,
        );
        assert_eq!(off.bg, Some(theme.muted));
        assert_eq!(off.fg, theme.muted_foreground);
        assert_eq!(off.hover_fg, Some(theme.foreground));

        let mute = style(ChannelToggleTone::Mute, ChannelToggleVariant::Default, true);
        assert_eq!(mute.bg, Some(theme.channel_mute.opacity(0.15)));
        assert_eq!(mute.fg, theme.channel_mute_foreground);
        assert_eq!(mute.border, None);

        let solo = style(ChannelToggleTone::Solo, ChannelToggleVariant::Outline, true);
        assert_eq!(solo.bg, Some(theme.channel_solo.opacity(0.2)));
        assert_eq!(solo.border, Some(theme.channel_solo.opacity(0.5)));

        let monitor = style(
            ChannelToggleTone::Monitor,
            ChannelToggleVariant::Ghost,
            true,
        );
        assert_eq!(monitor.bg, Some(theme.channel_monitor.opacity(0.15)));
        assert_eq!(monitor.fg, theme.channel_monitor_foreground);

        let neutral = style(
            ChannelToggleTone::Neutral,
            ChannelToggleVariant::Outline,
            true,
        );
        assert_eq!(neutral.bg, Some(theme.foreground));
        assert_eq!(neutral.fg, theme.background);
        assert_eq!(neutral.border, Some(theme.border));
    }

    #[test]
    fn channel_toggle_sizes() {
        let theme = Theme::dark();
        let s = |size| {
            channel_toggle_variants(
                &theme,
                size,
                ChannelToggleTone::Neutral,
                ChannelToggleVariant::Default,
                false,
            )
        };
        assert_eq!(s(ChannelToggleSize::Default).height, px(28.0));
        assert_eq!(s(ChannelToggleSize::Default).padding_x, px(6.0));
        assert_eq!(s(ChannelToggleSize::Icon).width, Some(px(28.0)));
        assert_eq!(s(ChannelToggleSize::Sm).min_width, px(24.0));
        assert_eq!(s(ChannelToggleSize::Lg).padding_x, px(8.0));
    }
}
