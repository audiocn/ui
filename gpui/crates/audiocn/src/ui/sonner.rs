//! Port of shadcn `components/ui/sonner.tsx`: a minimal [sonner] toaster.
//! Push toasts from anywhere with [`toast`] (or [`Toast::show`]); a
//! [`Toaster`] element placed once at the root of a window renders them in
//! the bottom-right corner (sonner's default), each for 4 s.
//!
//! [sonner]: https://sonner.emilkowal.ski

use std::time::Duration;

use gpui::{
    Anchor, App, FontWeight, Global, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, anchored, deferred, div, point, prelude::FluentBuilder, px, relative,
};

use super::icons::{Icon, IconName};
use super::styles::css_shadow;
use crate::motion::{Easing, Instant};
use crate::theme::ActiveTheme;

/// Toast type (sonner's `toast.success(...)` etc.), which picks the icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastKind {
    #[default]
    Default,
    Success,
    Info,
    Warning,
    Error,
    /// Spinner; stays until dismissed or replaced.
    Loading,
}

/// A toast to show.
///
/// ```ignore
/// Toast::new("Preset saved").description("Vocal chain").kind(ToastKind::Success).show(cx);
/// toast("Copied", cx);
/// ```
#[derive(Clone, Debug)]
pub struct Toast {
    title: SharedString,
    description: Option<SharedString>,
    kind: ToastKind,
    duration: Option<Duration>,
}

impl Toast {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            kind: ToastKind::Default,
            duration: Some(Duration::from_millis(4000)),
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        if kind == ToastKind::Loading {
            self.duration = None;
        }
        self
    }

    /// How long it stays (default 4 s). `None` keeps it until dismissed.
    pub fn duration(mut self, duration: Option<Duration>) -> Self {
        self.duration = duration;
        self
    }

    /// Shows the toast; returns its id (for [`dismiss_toast`]).
    pub fn show(self, cx: &mut App) -> usize {
        let duration = self.duration;
        let id = {
            let toasts = cx.default_global::<Toasts>();
            toasts.next_id += 1;
            let id = toasts.next_id;
            toasts.items.push(ToastItem {
                id,
                toast: self,
                created: Instant::now(),
            });
            id
        };
        cx.refresh_windows();
        if let Some(duration) = duration {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(duration).await;
                cx.update(|cx| dismiss_toast(id, cx));
            })
            .detach();
        }
        id
    }
}

/// `toast("message")`.
pub fn toast(title: impl Into<SharedString>, cx: &mut App) -> usize {
    Toast::new(title).show(cx)
}

/// Removes a toast.
pub fn dismiss_toast(id: usize, cx: &mut App) {
    if let Some(toasts) = cx.try_global::<Toasts>()
        && toasts.items.iter().any(|t| t.id == id)
    {
        cx.global_mut::<Toasts>().items.retain(|t| t.id != id);
        cx.refresh_windows();
    }
}

#[derive(Default)]
struct Toasts {
    next_id: usize,
    items: Vec<ToastItem>,
}

impl Global for Toasts {}

struct ToastItem {
    id: usize,
    toast: Toast,
    created: Instant,
}

/// Renders the active toasts (bottom-right of the window, newest at the
/// bottom) in a top layer. Place one anywhere in each window's tree.
#[derive(IntoElement, Default)]
pub struct Toaster {
    max_visible: Option<usize>,
}

impl Toaster {
    pub fn new() -> Self {
        Self::default()
    }

    /// `visibleToasts` (default 3).
    pub fn visible_toasts(mut self, n: usize) -> Self {
        self.max_visible = Some(n);
        self
    }
}

impl RenderOnce for Toaster {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let now = Instant::now();
        let max = self.max_visible.unwrap_or(3);
        let Some(toasts) = cx.try_global::<Toasts>().filter(|t| !t.items.is_empty()) else {
            return div().into_any_element();
        };
        let start = toasts.items.len().saturating_sub(max);
        let mut column = div().w(px(356.0)).flex().flex_col().gap(px(14.0));
        for item in &toasts.items[start..] {
            // sonner: 400ms ease enter (translateY 100% → 0, opacity 0 → 1).
            let p = (now.duration_since(item.created).as_secs_f32() / 0.4).clamp(0.0, 1.0);
            if p < 1.0 {
                window.request_animation_frame();
            }
            let e = Easing::EASE.apply(p);
            let icon = match item.toast.kind {
                ToastKind::Default => None,
                ToastKind::Success => Some(Icon::new(IconName::CheckCircle)),
                ToastKind::Info => Some(Icon::new(IconName::Info)),
                ToastKind::Warning => Some(Icon::new(IconName::Warning)),
                ToastKind::Error => Some(Icon::new(IconName::XCircle)),
                ToastKind::Loading => Some(Icon::new(IconName::Spinner).spin(true)),
            };
            column = column.child(
                div()
                    .relative()
                    .top(px(40.0 * (1.0 - e)))
                    .opacity(e)
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .w_full()
                    .p(px(16.0))
                    .rounded(t.radius)
                    .border_1()
                    .border_color(t.border)
                    .bg(t.popover)
                    .text_color(t.popover_foreground)
                    .font_family(t.font_sans.clone())
                    .text_size(px(13.0))
                    .shadow(vec![css_shadow(
                        0.0,
                        4.0,
                        12.0,
                        0.0,
                        gpui::black().opacity(0.1),
                    )])
                    .when_some(icon, |el, icon| {
                        el.child(div().flex_shrink_0().child(icon.size(px(16.0))))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .line_height(relative(1.5))
                                    .child(item.toast.title.clone()),
                            )
                            .when_some(item.toast.description.clone(), |el, d| {
                                el.child(div().line_height(relative(1.4)).child(d))
                            }),
                    ),
            );
        }
        let viewport = window.viewport_size();
        deferred(
            anchored()
                .position(point(viewport.width - px(24.0), viewport.height - px(24.0)))
                .anchor(Anchor::BottomRight)
                .child(column),
        )
        .with_priority(3)
        .into_any_element()
    }
}
