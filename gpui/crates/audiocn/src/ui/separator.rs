//! Port of shadcn `components/ui/separator.tsx` (Base UI `Separator`).

use gpui::{
    App, Div, IntoElement, Refineable, RenderOnce, StyleRefinement, Styled, Window, div, px,
};

use crate::Orientation;
use crate::theme::ActiveTheme;

/// A 1px `bg-border` rule.
///
/// ```ignore
/// Separator::new()                      // h-px w-full
/// Separator::vertical()                 // w-px self-stretch
/// ```
#[derive(IntoElement, Default)]
pub struct Separator {
    orientation: Orientation,
    style: StyleRefinement,
}

impl Separator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertical() -> Self {
        Self::new().orientation(Orientation::Vertical)
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }
}

impl Styled for Separator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Separator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let el: Div = div().flex_shrink_0().bg(cx.theme().border);
        let mut el = match self.orientation {
            Orientation::Horizontal => el.h(px(1.0)).w_full(),
            Orientation::Vertical => el.w(px(1.0)).self_stretch(),
        };
        el.style().refine(&self.style);
        el
    }
}
