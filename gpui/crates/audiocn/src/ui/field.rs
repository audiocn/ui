//! Port of shadcn `components/ui/field.tsx`.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, Refineable, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px, relative,
};

use super::separator::Separator;
use super::styles::{TextSize, parent_styled, provide, use_context};
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
            /// No text (add children with `.child`).
            pub fn empty() -> Self {
                Self {
                    children: Vec::new(),
                    style: StyleRefinement::default(),
                }
            }
        }
        parent_styled!($ty);
    };
}

/// State a [`Field`] passes to its parts (`group-data-[disabled]/field`).
#[derive(Clone, Copy, Default)]
struct FieldCx {
    disabled: bool,
}

#[derive(Clone, Copy)]
struct InFieldGroup;

simple!(FieldSet, "`<fieldset>`: `flex flex-col gap-6`.");

impl RenderOnce for FieldSet {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// `variant` of [`FieldLegend`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldLegendVariant {
    #[default]
    Legend,
    Label,
}

/// `<legend>`: `mb-3 font-medium`, `text-base` (legend) or `text-sm` (label).
#[derive(IntoElement)]
pub struct FieldLegend {
    variant: FieldLegendVariant,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl FieldLegend {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            variant: FieldLegendVariant::Legend,
            children: vec![text.into().into_any_element()],
            style: StyleRefinement::default(),
        }
    }

    pub fn variant(mut self, variant: FieldLegendVariant) -> Self {
        self.variant = variant;
        self
    }
}
parent_styled!(FieldLegend);

impl RenderOnce for FieldLegend {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .mb(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .map(|el| match self.variant {
                FieldLegendVariant::Legend => el.text_base_(),
                FieldLegendVariant::Label => el.text_sm_(),
            })
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

simple!(
    FieldGroup,
    "`flex w-full flex-col gap-6` (`gap-4` when nested in another group)."
);

impl RenderOnce for FieldGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let nested = use_context::<InFieldGroup>().is_some();
        let mut el: Div = div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(if nested { 16.0 } else { 24.0 }))
            .children(self.children);
        el.style().refine(&self.style);
        provide(InFieldGroup, el)
    }
}

/// `orientation` of [`Field`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldOrientation {
    #[default]
    Vertical,
    Horizontal,
    /// Vertical; GPUI has no container queries, so this lays out like `Vertical`.
    Responsive,
}

/// A form field (`role="group"`): label, control, description, error.
///
/// ```ignore
/// Field::new().child(FieldLabel::new("Gain")).child(Input::new("gain"))
///     .child(FieldDescription::new("In dB."))
/// ```
#[derive(IntoElement, Default)]
pub struct Field {
    orientation: FieldOrientation,
    invalid: bool,
    disabled: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Field {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn orientation(mut self, orientation: FieldOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// `data-invalid`: `text-destructive`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// `data-disabled`: labels at 50% opacity.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
parent_styled!(Field);

impl RenderOnce for Field {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let destructive = cx.theme().destructive;
        let mut el: Div = div()
            .flex()
            .w_full()
            .gap(px(12.0))
            .map(|el| match self.orientation {
                FieldOrientation::Horizontal => el.flex_row().items_center(),
                _ => el.flex_col(),
            })
            .when(self.invalid, |el| el.text_color(destructive))
            .children(self.children);
        el.style().refine(&self.style);
        provide(
            FieldCx {
                disabled: self.disabled,
            },
            el,
        )
    }
}

simple!(
    FieldContent,
    "`flex flex-1 flex-col gap-1 leading-snug` (label + description next to a control)."
);

impl RenderOnce for FieldContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(4.0))
            .line_height(relative(1.375))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

text_part!(
    FieldLabel,
    "A [`Label`](super::Label) for a field: `flex w-fit gap-2 leading-snug text-sm font-medium`."
);

impl RenderOnce for FieldLabel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let disabled = use_context::<FieldCx>().is_some_and(|f| f.disabled);
        let mut el: Div = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(14.0))
            .line_height(relative(1.375))
            .font_weight(FontWeight::MEDIUM)
            .when(disabled, |el| el.opacity(0.5))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

text_part!(
    FieldTitle,
    "`flex w-fit items-center gap-2 text-sm font-medium` (a non-`<label>` title)."
);

impl RenderOnce for FieldTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let disabled = use_context::<FieldCx>().is_some_and(|f| f.disabled);
        let mut el: Div = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_sm_()
            .font_weight(FontWeight::MEDIUM)
            .when(disabled, |el| el.opacity(0.5))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

text_part!(
    FieldDescription,
    "`text-muted-foreground text-left text-sm leading-normal font-normal`."
);

impl RenderOnce for FieldDescription {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut el: Div = div()
            .text_left()
            .text_size(px(14.0))
            .line_height(relative(1.5))
            .font_weight(FontWeight::NORMAL)
            .text_color(cx.theme().muted_foreground)
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

/// A horizontal rule between fields, optionally with centred text.
#[derive(IntoElement, Default)]
pub struct FieldSeparator {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl FieldSeparator {
    pub fn new() -> Self {
        Self::default()
    }
}
parent_styled!(FieldSeparator);

impl RenderOnce for FieldSeparator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let has_content = !self.children.is_empty();
        let mut el: Div = div()
            .relative()
            .my(px(-8.0))
            .h(px(20.0))
            .text_sm_()
            .flex()
            .justify_center()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(10.0))
                    .child(Separator::new()),
            )
            .when(has_content, |el| {
                el.child(
                    div()
                        .relative()
                        .px(px(8.0))
                        .bg(t.background)
                        .text_color(t.muted_foreground)
                        .children(self.children),
                )
            });
        el.style().refine(&self.style);
        el
    }
}

/// Validation message(s): `text-destructive text-sm font-normal`. Renders
/// nothing when empty. Several distinct errors render as a bulleted list.
#[derive(IntoElement, Default)]
pub struct FieldError {
    errors: Vec<SharedString>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl FieldError {
    pub fn new() -> Self {
        Self::default()
    }

    /// The `errors` prop (`{ message }[]`); duplicates are removed.
    pub fn errors(mut self, errors: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        for e in errors {
            let e = e.into();
            if !e.is_empty() && !self.errors.contains(&e) {
                self.errors.push(e);
            }
        }
        self
    }
}
parent_styled!(FieldError);

impl RenderOnce for FieldError {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let destructive = cx.theme().destructive;
        let mut el: Div = div()
            .text_sm_()
            .font_weight(FontWeight::NORMAL)
            .text_color(destructive);
        if !self.children.is_empty() {
            el = el.children(self.children);
        } else if self.errors.len() == 1 {
            el = el.child(self.errors[0].clone());
        } else if self.errors.len() > 1 {
            el = el.child(div().ml(px(16.0)).flex().flex_col().gap(px(4.0)).children(
                self.errors.into_iter().map(|e| {
                    div()
                        .flex()
                        .gap(px(6.0))
                        .child(div().child("•"))
                        .child(div().child(e))
                }),
            ));
        } else {
            return div().hidden().into_any_element();
        }
        el.style().refine(&self.style);
        el.into_any_element()
    }
}
