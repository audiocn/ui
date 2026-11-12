//! Port of shadcn `components/ui/skeleton.tsx`.

use gpui::{App, Div, IntoElement, Refineable, RenderOnce, StyleRefinement, Styled, Window, div};

use super::styles::radius_2xl;
use crate::motion::{Easing, Instant};
use crate::theme::ActiveTheme;

/// A loading placeholder: `bg-muted animate-pulse rounded-2xl`. Size it with
/// the `Styled` methods.
///
/// ```ignore
/// Skeleton::new().h(px(16.0)).w(px(250.0))
/// ```
#[derive(IntoElement, Default)]
pub struct Skeleton {
    style: StyleRefinement,
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Tailwind `animate-pulse`: `pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite`,
/// keyframes `50% { opacity: 0.5 }`.
pub(crate) fn pulse_opacity(now: Instant) -> f32 {
    let t = (now.duration_since(epoch()).as_secs_f32() / 2.0).fract();
    let ease = Easing::CubicBezier(0.4, 0.0, 0.6, 1.0);
    if t < 0.5 {
        1.0 - 0.5 * ease.apply(t * 2.0)
    } else {
        0.5 + 0.5 * ease.apply((t - 0.5) * 2.0)
    }
}

impl RenderOnce for Skeleton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        window.request_animation_frame();
        let t = cx.theme();
        let mut el: Div = div()
            .bg(t.muted)
            .rounded(radius_2xl(t))
            .opacity(pulse_opacity(Instant::now()));
        el.style().refine(&self.style);
        el
    }
}
