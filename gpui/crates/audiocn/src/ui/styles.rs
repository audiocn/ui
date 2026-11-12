//! Shared Tailwind → GPUI helpers for the primitives: the radius scale of
//! `app/globals.css`, shadows, focus rings, and a tiny React-context
//! equivalent ([`Provider`] / [`use_context`]) so sub-parts (`CardContent`,
//! `TabsTrigger`, `Kbd` inside a tooltip, ...) can read their parent's props.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Div, Element, ElementId, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Styled, Window, div, point, px,
};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Radius scale (`--radius-*` in app/globals.css)

/// `rounded-sm`: `--radius * 0.6`.
pub(crate) fn radius_sm(t: &Theme) -> Pixels {
    t.radius * 0.6
}
/// `rounded-md`: `--radius * 0.8`.
pub(crate) fn radius_md(t: &Theme) -> Pixels {
    t.radius * 0.8
}
/// `rounded-lg`: `--radius`.
pub(crate) fn radius_lg(t: &Theme) -> Pixels {
    t.radius
}
/// `rounded-xl`: `--radius * 1.4`.
pub(crate) fn radius_xl(t: &Theme) -> Pixels {
    t.radius * 1.4
}
/// `rounded-2xl`: `--radius * 1.8`.
pub(crate) fn radius_2xl(t: &Theme) -> Pixels {
    t.radius * 1.8
}
/// `rounded-3xl`: `--radius * 2.2`.
pub(crate) fn radius_3xl(t: &Theme) -> Pixels {
    t.radius * 2.2
}
/// `rounded-4xl`: `--radius * 2.6`.
pub(crate) fn radius_4xl(t: &Theme) -> Pixels {
    t.radius * 2.6
}

/// A CSS radius on a box of the given height: browsers shrink radii that
/// don't fit, so a `rounded-2xl` 32px button is a pill.
pub(crate) fn fit_radius(radius: Pixels, height: Pixels) -> Pixels {
    radius.min(height * 0.5)
}

// ---------------------------------------------------------------------------
// Shadows

/// A CSS `box-shadow` (blur is the CSS blur radius; GPUI's is a sigma).
pub(crate) fn css_shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur / 2.0),
        spread_radius: px(spread),
        inset: false,
    }
}

fn black(alpha: f32) -> Hsla {
    gpui::black().opacity(alpha)
}

/// Tailwind v4 `shadow-xs`.
pub(crate) fn shadow_xs() -> Vec<BoxShadow> {
    vec![css_shadow(0.0, 1.0, 2.0, 0.0, black(0.05))]
}

/// Tailwind v4 `shadow-sm`.
pub(crate) fn shadow_sm() -> Vec<BoxShadow> {
    vec![
        css_shadow(0.0, 1.0, 3.0, 0.0, black(0.1)),
        css_shadow(0.0, 1.0, 2.0, -1.0, black(0.1)),
    ]
}

/// Tailwind v4 `shadow-md`.
#[allow(dead_code)]
pub(crate) fn shadow_md() -> Vec<BoxShadow> {
    vec![
        css_shadow(0.0, 4.0, 6.0, -1.0, black(0.1)),
        css_shadow(0.0, 2.0, 4.0, -2.0, black(0.1)),
    ]
}

/// Tailwind v4 `shadow-lg`.
pub(crate) fn shadow_lg() -> Vec<BoxShadow> {
    vec![
        css_shadow(0.0, 10.0, 15.0, -3.0, black(0.1)),
        css_shadow(0.0, 4.0, 6.0, -4.0, black(0.1)),
    ]
}

/// `ring-N ring-color` as a spread shadow. Only for opaque elements (GPUI
/// shadows are not clipped to the outside of the box); use [`ring_overlay`]
/// on transparent ones.
pub(crate) fn ring_shadow(width: f32, color: Hsla) -> BoxShadow {
    css_shadow(0.0, 0.0, 0.0, width, color)
}

/// [`ring_overlay`] for a host with a `border_width` border. Absolute
/// children are placed from the padding box, so the plain overlay would cover
/// the border; a CSS ring (a box shadow) sits outside the border box.
pub(crate) fn ring_overlay_bordered(
    radius: Pixels,
    width: f32,
    border_width: f32,
    color: Hsla,
) -> Div {
    let w = px(width);
    let inset = -px(width + border_width);
    div()
        .absolute()
        .top(inset)
        .left(inset)
        .right(inset)
        .bottom(inset)
        .rounded(radius + w)
        .border(w)
        .border_color(color)
}

/// `ring-N ring-color` drawn as an absolutely positioned border around the
/// parent (which must not clip its overflow). `radius` is the parent's.
pub(crate) fn ring_overlay(radius: Pixels, width: f32, color: Hsla) -> Div {
    let w = px(width);
    div()
        .absolute()
        .top(-w)
        .left(-w)
        .right(-w)
        .bottom(-w)
        .rounded(radius + w)
        .border(w)
        .border_color(color)
}

/// The standard focus-visible / aria-invalid ring of the primitives:
/// `focus-visible:ring-3 ring-ring/30`, `aria-invalid:ring-destructive/20`
/// (`dark:ring-destructive/40`). Returns the ring colour, if any.
pub(crate) fn focus_ring_color(
    t: &Theme,
    focus_visible: bool,
    invalid: bool,
    ring_alpha: f32,
) -> Option<Hsla> {
    if invalid {
        Some(t.destructive.opacity(if t.is_dark { 0.4 } else { 0.2 }))
    } else if focus_visible {
        Some(t.ring.opacity(ring_alpha))
    } else {
        None
    }
}

/// Text size + line height of a Tailwind text class (`text-sm` = 14/20).
pub(crate) trait TextSize: Styled + Sized {
    fn text_xs_(self) -> Self {
        self.text_size(px(12.0)).line_height(px(16.0))
    }
    fn text_sm_(self) -> Self {
        self.text_size(px(14.0)).line_height(px(20.0))
    }
    fn text_base_(self) -> Self {
        self.text_size(px(16.0)).line_height(px(24.0))
    }
    fn text_lg_(self) -> Self {
        self.text_size(px(18.0)).line_height(px(28.0))
    }
}
impl<T: Styled> TextSize for T {}

/// `bg-x/NN` when the theme is light, `dark:bg-x/MM` when dark.
pub(crate) fn by_mode(t: &Theme, light: Hsla, dark: Hsla) -> Hsla {
    if t.is_dark { dark } else { light }
}

// ---------------------------------------------------------------------------
// Context (React context for RenderOnce sub-parts)

thread_local! {
    static CONTEXT: RefCell<Vec<Rc<dyn Any>>> = const { RefCell::new(Vec::new()) };
}

/// Makes `value` visible to [`use_context`] in every descendant that renders
/// (lays out) inside `child`. Deferred (overlay) content is laid out later, so
/// it does not see the context: pass values to it explicitly.
pub(crate) struct Provider {
    value: Rc<dyn Any>,
    child: Option<AnyElement>,
}

pub(crate) fn provide<T: 'static>(value: T, child: impl IntoElement) -> Provider {
    Provider {
        value: Rc::new(value),
        child: Some(child.into_any_element()),
    }
}

/// The nearest value of type `T` provided by an ancestor [`Provider`].
pub(crate) fn use_context<T: Clone + 'static>() -> Option<T> {
    CONTEXT.with(|stack| {
        stack
            .borrow()
            .iter()
            .rev()
            .find_map(|v| v.downcast_ref::<T>().cloned())
    })
}

fn with_context<R>(value: &Rc<dyn Any>, f: impl FnOnce() -> R) -> R {
    CONTEXT.with(|stack| stack.borrow_mut().push(value.clone()));
    let result = f();
    CONTEXT.with(|stack| {
        stack.borrow_mut().pop();
    });
    result
}

impl IntoElement for Provider {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Provider {
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
        let mut child = self.child.take().expect("Provider laid out twice");
        let id = with_context(&self.value, || child.request_layout(window, cx));
        (id, child)
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
        with_context(&self.value, || child.prepaint(window, cx));
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
        with_context(&self.value, || child.paint(window, cx));
    }
}

/// `ParentElement` (into `self.children`) + `Styled` (into `self.style`).
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
pub(crate) use parent_styled;
