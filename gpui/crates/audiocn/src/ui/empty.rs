//! Port of shadcn `components/ui/empty.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px, relative,
};

use super::icons::Icon;
use super::styles::{TextSize, parent_styled, radius_3xl, radius_xl};
use crate::theme::ActiveTheme;

macro_rules! simple {
    ($ty:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(IntoElement, Default)]
        pub struct $ty {
            children: Vec<AnyElement>,
            style: StyleRefinement,
        }
        impl $ty {
            pub fn new() -> Self {
                Self::default()
            }
        }
        parent_styled!($ty);
    };
}

simple!(
    Empty,
    "An empty state: `flex flex-col items-center justify-center gap-4 p-12 text-center`.\n\n```ignore\nEmpty::new().child(EmptyHeader::new()\n    .child(EmptyMedia::new().variant(EmptyMediaVariant::Icon).icon(IconName::MusicNotes))\n    .child(EmptyTitle::new(\"No tracks\"))\n    .child(EmptyDescription::new(\"Add a file to get started.\")))\n```"
);

impl RenderOnce for Empty {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .rounded(radius_3xl(cx.theme()))
            .p(px(48.0))
            .text_center()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

simple!(EmptyHeader, "`flex max-w-sm flex-col items-center gap-2`.");

impl RenderOnce for EmptyHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .max_w(px(384.0))
            .items_center()
            .gap(px(8.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `variant` of [`EmptyMedia`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmptyMediaVariant {
    #[default]
    Default,
    /// `bg-muted size-10 rounded-xl`, icon `size-5`.
    Icon,
}

/// The illustration / icon slot of an [`EmptyHeader`].
#[derive(IntoElement, Default)]
pub struct EmptyMedia {
    variant: EmptyMediaVariant,
    icon: Option<Icon>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl EmptyMedia {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn variant(mut self, variant: EmptyMediaVariant) -> Self {
        self.variant = variant;
        self
    }

    /// An icon, sized for the variant (`size-5` for [`EmptyMediaVariant::Icon`]).
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}
parent_styled!(EmptyMedia);

impl RenderOnce for EmptyMedia {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let is_icon = self.variant == EmptyMediaVariant::Icon;
        let mut el: Div = div()
            .mb(px(8.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .when(is_icon, |el| {
                el.bg(t.muted)
                    .text_color(t.foreground)
                    .size(px(40.0))
                    .rounded(radius_xl(t))
            })
            .when_some(self.icon, |el, icon| {
                el.child(if is_icon { icon.size(px(20.0)) } else { icon })
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

macro_rules! text_part {
    ($ty:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(IntoElement)]
        pub struct $ty {
            children: Vec<AnyElement>,
            style: StyleRefinement,
        }
        impl $ty {
            pub fn new(text: impl Into<SharedString>) -> Self {
                Self {
                    children: vec![text.into().into_any_element()],
                    style: StyleRefinement::default(),
                }
            }
        }
        parent_styled!($ty);
    };
}

text_part!(
    EmptyTitle,
    "`font-heading text-lg font-medium tracking-tight`."
);

impl RenderOnce for EmptyTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_lg_()
            .font_weight(FontWeight::MEDIUM)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

text_part!(EmptyDescription, "`text-muted-foreground text-sm/relaxed`.");

impl RenderOnce for EmptyDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_size(px(14.0))
            .line_height(relative(1.625))
            .text_color(cx.theme().muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

simple!(
    EmptyContent,
    "`flex w-full max-w-sm min-w-0 flex-col items-center gap-4 text-sm`."
);

impl RenderOnce for EmptyContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(384.0))
            .min_w_0()
            .items_center()
            .gap(px(16.0))
            .text_sm_()
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}
