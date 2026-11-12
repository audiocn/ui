//! Port of `components/ui/lcd-display.tsx`.
//!
//! An LCD screen in a bezel: backlit with dark pixels in the light theme, and
//! glowing ink on dark glass in the dark one. Every `--lcd-display-*` CSS
//! variable is a builder option on [`LcdDisplay`] (see [`LcdDisplayVars`]),
//! with the same defaults, and every part reads the surrounding display's
//! settings like React's context ([`use_lcd_display`]).
//!
//! **How the dot matrix is drawn.** CSS masks the screen's content with a
//! radial-gradient dot every `--lcd-display-pitch` and paints unlit dots
//! behind it. GPUI has no masks, so the port reproduces what the browser
//! rasterises at the window's scale factor: it samples the same gradients at
//! device-pixel centres ([`unlit_dot_pattern`], [`mask_hole_size`]), paints
//! the unlit dots as rows cut into dots, and punches the content's mask holes
//! (glass-coloured squares at the grid intersections) over every lit part. At
//! 1x the browser shows no holes and a uniform faint wash; at 2x a dot grid;
//! the port does the same. The glow (`drop-shadow(0 0 3px glow)`) is drawn as
//! blurred shadows under shapes and soft copies under text.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, Corners, Element, ElementId, FocusHandle,
    FontWeight, GlobalElementId, Hsla, InspectorElementId, InteractiveElement, IntoElement,
    KeyDownEvent, LayoutId, ParentElement, Pixels, Point, Refineable, RenderOnce, ShapedLine,
    SharedString, StatefulInteractiveElement, Style, StyleRefinement, Styled, TextRun, Window,
    anchored, canvas, deferred, div, fill, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder, px, size,
};

use crate::motion::{Duration, Easing, Instant, Transition};
use crate::signal::reduced_motion::prefers_reduced_motion;
use crate::theme::{ActiveTheme, Theme, mix, oklch, to_oklab};
use crate::ui::styles::{provide, radius_lg, radius_md, radius_sm, ring_overlay, use_context};

// ---------------------------------------------------------------------------
// Variants

/// The ink colour (`variant`). `Primary` follows the theme's `--primary`; the
/// others are fixed hardware inks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplayVariant {
    #[default]
    Primary,
    Amber,
    Green,
    Cyan,
    Red,
}

impl LcdDisplayVariant {
    /// `--lcd-display-ink` for this variant.
    pub fn ink(self, theme: &Theme) -> Hsla {
        match self {
            Self::Primary => theme.primary,
            Self::Amber => oklch(0.76, 0.13, 52.0),
            Self::Cyan => oklch(0.84, 0.12, 205.0),
            Self::Green => oklch(0.85, 0.19, 142.0),
            Self::Red => oklch(0.68, 0.2, 27.0),
        }
    }
}

/// The text size (`size`); rows, separator and status follow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplaySize {
    /// `text-sm`.
    Sm,
    /// `text-lg`.
    #[default]
    Default,
    /// `text-2xl`.
    Lg,
}

impl LcdDisplaySize {
    /// The font size; the display sets `leading-none`, so it is also the line height.
    pub fn text_size(self) -> Pixels {
        match self {
            Self::Sm => px(14.0),
            Self::Default => px(18.0),
            Self::Lg => px(24.0),
        }
    }
}

/// `light` is a backlit screen with dark pixels, `dark` an unlit screen with
/// glowing ink, and `auto` follows the theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplayAppearance {
    #[default]
    Auto,
    Light,
    Dark,
}

impl LcdDisplayAppearance {
    /// Whether this appearance draws the dark screen under `theme`.
    pub fn is_dark(self, theme: &Theme) -> bool {
        match self {
            Self::Auto => theme.is_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// `state` of [`LcdDisplayStatus`]: `Idle` blinks slowly, `Busy` quickly,
/// `Off` shows nothing lit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplayStatusState {
    #[default]
    Idle,
    Busy,
    Off,
}

// ---------------------------------------------------------------------------
// CSS variables

/// The `--lcd-display-*` variables a caller may set. `None` keeps the value the
/// variant and appearance give it. The lit, dim and unlit pixels are mixed from
/// the ink, so a custom colour needs only [`ink`](Self::ink), in both
/// appearances.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LcdDisplayVars {
    /// `--lcd-display-ink`.
    pub ink: Option<Hsla>,
    /// `--lcd-display-lit`: the ink as drawn (darkened on the light screen).
    pub lit: Option<Hsla>,
    /// `--lcd-display-ink-dim`: labels. Default: lit at 40%.
    pub ink_dim: Option<Hsla>,
    /// `--lcd-display-ink-unlit`: the unlit dots. Default: lit at 7%.
    pub ink_unlit: Option<Hsla>,
    /// `--lcd-display-glass`.
    pub glass: Option<Hsla>,
    /// `--lcd-display-glow`.
    pub glow: Option<Hsla>,
    /// `--lcd-display-bezel`: the top of the bezel gradient.
    pub bezel: Option<Hsla>,
    /// `--lcd-display-bezel-edge`: the bottom of the bezel gradient.
    pub bezel_edge: Option<Hsla>,
    /// `--lcd-display-bezel-ring`.
    pub bezel_ring: Option<Hsla>,
    /// `--lcd-display-bezel-shadow`.
    pub bezel_shadow: Option<Vec<BoxShadow>>,
    /// `--lcd-display-recess`: the glass's inset shadow.
    pub recess: Option<Vec<BoxShadow>>,
    /// `--lcd-display-scanline`.
    pub scanline: Option<Hsla>,
    /// `--lcd-display-sheen`.
    pub sheen: Option<Hsla>,
    /// `--lcd-display-font`: a font family. Default: the theme's `--font-mono`.
    pub font: Option<SharedString>,
    /// `--lcd-display-pitch`: the dot grid. Default 2px.
    pub pitch: Option<Pixels>,
    /// The font weight the screen's text uses (`font-weight` on the display).
    /// Default normal.
    pub font_weight: Option<FontWeight>,
}

/// Every `--lcd-display-*` variable, resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct LcdDisplayPalette {
    pub ink: Hsla,
    pub lit: Hsla,
    pub ink_dim: Hsla,
    pub ink_unlit: Hsla,
    pub glass: Hsla,
    pub glow: Hsla,
    pub bezel: Hsla,
    pub bezel_edge: Hsla,
    pub bezel_ring: Hsla,
    pub bezel_shadow: Vec<BoxShadow>,
    pub recess: Vec<BoxShadow>,
    pub scanline: Hsla,
    pub sheen: Hsla,
    pub font: SharedString,
    pub pitch: Pixels,
    pub font_weight: FontWeight,
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    gpui::Rgba {
        r: r / 255.0,
        g: g / 255.0,
        b: b / 255.0,
        a,
    }
    .into()
}

/// A CSS `box-shadow` (the blur is CSS's; GPUI's blur radius is a sigma).
fn css_shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla, inset: bool) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur / 2.0),
        spread_radius: px(spread),
        inset,
    }
}

/// Whether `oklch(l c h)` is inside sRGB, before any clamping.
fn in_srgb_gamut(l: f32, c: f32, h: f32) -> bool {
    let h = h.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let ok = |x: f32| (-1e-4..=1.0 + 1e-4).contains(&x);
    ok(r) && ok(g) && ok(bl)
}

/// `oklch(from color l calc(c * chroma) h)`, gamut-mapped into sRGB the way
/// CSS does: by lowering the chroma, keeping the lightness and hue.
fn relative_oklch(color: Hsla, lightness: f32, chroma: f32) -> Hsla {
    let (_, a, b, _) = to_oklab(color);
    let mut c = (a * a + b * b).sqrt() * chroma;
    let h = b.atan2(a).to_degrees();
    if !in_srgb_gamut(lightness, c, h) {
        let (mut low, mut high) = (0.0, c);
        for _ in 0..20 {
            let mid = (low + high) / 2.0;
            if in_srgb_gamut(lightness, mid, h) {
                low = mid;
            } else {
                high = mid;
            }
        }
        c = low;
    }
    oklch(lightness, c, h)
}

/// Resolves the display's variables for a variant and appearance, like the
/// cascade of `lcdDisplayVariants` plus any caller overrides.
pub fn resolve_lcd_display_palette(
    variant: LcdDisplayVariant,
    appearance: LcdDisplayAppearance,
    vars: &LcdDisplayVars,
    theme: &Theme,
) -> LcdDisplayPalette {
    let dark = appearance.is_dark(theme);
    let ink = vars.ink.unwrap_or_else(|| variant.ink(theme));
    let black = gpui::black();
    let lit = vars.lit.unwrap_or_else(|| {
        if dark {
            ink
        } else {
            // color-mix(in oklab, ink, black 62%)
            mix(ink, black, 0.62)
        }
    });
    let glass = vars.glass.unwrap_or_else(|| {
        if dark {
            oklch(0.15, 0.004, 200.0)
        } else {
            relative_oklch(ink, 0.93, 0.35)
        }
    });
    let glow = vars
        .glow
        .unwrap_or_else(|| lit.opacity(if dark { 0.6 } else { 0.22 }));
    let (bezel, bezel_edge, bezel_ring, bezel_shadow, recess, scanline, sheen) = if dark {
        (
            oklch(0.33, 0.004, 250.0),
            oklch(0.24, 0.004, 250.0),
            rgba(0.0, 0.0, 0.0, 0.7),
            vec![
                css_shadow(0.0, 1.0, 0.0, 0.0, rgba(255.0, 255.0, 255.0, 0.06), true),
                css_shadow(0.0, 2.0, 6.0, 0.0, rgba(0.0, 0.0, 0.0, 0.5), false),
            ],
            vec![
                css_shadow(0.0, 2.0, 10.0, 0.0, rgba(0.0, 0.0, 0.0, 0.9), true),
                css_shadow(0.0, 0.0, 0.0, 1.0, rgba(0.0, 0.0, 0.0, 0.8), true),
            ],
            rgba(0.0, 0.0, 0.0, 0.22),
            rgba(255.0, 255.0, 255.0, 0.05),
        )
    } else {
        (
            oklch(0.95, 0.003, 250.0),
            oklch(0.85, 0.004, 250.0),
            rgba(0.0, 0.0, 0.0, 0.14),
            vec![
                css_shadow(0.0, 1.0, 0.0, 0.0, rgba(255.0, 255.0, 255.0, 0.8), true),
                css_shadow(0.0, 1.0, 3.0, 0.0, rgba(0.0, 0.0, 0.0, 0.18), false),
            ],
            vec![
                css_shadow(0.0, 1.0, 4.0, 0.0, rgba(0.0, 0.0, 0.0, 0.28), true),
                css_shadow(0.0, 0.0, 0.0, 1.0, rgba(0.0, 0.0, 0.0, 0.2), true),
            ],
            rgba(0.0, 0.0, 0.0, 0.04),
            rgba(255.0, 255.0, 255.0, 0.35),
        )
    };
    LcdDisplayPalette {
        ink,
        lit,
        ink_dim: vars.ink_dim.unwrap_or_else(|| lit.opacity(0.4)),
        ink_unlit: vars.ink_unlit.unwrap_or_else(|| lit.opacity(0.07)),
        glass,
        glow,
        bezel: vars.bezel.unwrap_or(bezel),
        bezel_edge: vars.bezel_edge.unwrap_or(bezel_edge),
        bezel_ring: vars.bezel_ring.unwrap_or(bezel_ring),
        bezel_shadow: vars.bezel_shadow.clone().unwrap_or(bezel_shadow),
        recess: vars.recess.clone().unwrap_or(recess),
        scanline: vars.scanline.unwrap_or(scanline),
        sheen: vars.sheen.unwrap_or(sheen),
        font: vars.font.clone().unwrap_or_else(|| theme.font_mono.clone()),
        pitch: vars.pitch.unwrap_or(px(2.0)),
        font_weight: vars.font_weight.unwrap_or(FontWeight::NORMAL),
    }
}

/// `tracking-[0.12em]` on the display.
pub(crate) const TRACKING_EM: f32 = 0.12;

// ---------------------------------------------------------------------------
// Context

/// The surrounding display's settings, which parts on its screen read
/// (`useLcdDisplay`).
#[derive(Clone)]
pub struct LcdDisplayContext {
    pub appearance: LcdDisplayAppearance,
    pub variant: LcdDisplayVariant,
    pub size: LcdDisplaySize,
    pub matrix: bool,
    pub scanlines: bool,
    /// The resolved `--lcd-display-*` variables.
    pub palette: LcdDisplayPalette,
    pub(crate) grid: DotGrid,
}

impl LcdDisplayContext {
    /// What a part outside a display falls back to (`DEFAULT_CONTEXT`).
    pub fn fallback(cx: &App) -> Self {
        Self::new(
            LcdDisplayAppearance::Auto,
            LcdDisplayVariant::Primary,
            LcdDisplaySize::Default,
            true,
            true,
            &LcdDisplayVars::default(),
            cx.theme(),
        )
    }

    fn new(
        appearance: LcdDisplayAppearance,
        variant: LcdDisplayVariant,
        size: LcdDisplaySize,
        matrix: bool,
        scanlines: bool,
        vars: &LcdDisplayVars,
        theme: &Theme,
    ) -> Self {
        let palette = resolve_lcd_display_palette(variant, appearance, vars, theme);
        Self {
            appearance,
            variant,
            size,
            matrix,
            scanlines,
            grid: DotGrid::new(palette.pitch, palette.glass, matrix),
            palette,
        }
    }

    /// The font size (and line height) of the display.
    pub fn text_size(&self) -> Pixels {
        self.size.text_size()
    }

    /// `tracking-[0.12em]`, in pixels of the display's font size.
    pub fn tracking(&self) -> Pixels {
        self.text_size() * TRACKING_EM
    }

    /// A copy with a fresh dot grid, for a screen of its own (the popup).
    fn with_own_grid(&self) -> Self {
        let mut copy = self.clone();
        copy.grid = DotGrid::new(self.palette.pitch, self.palette.glass, self.matrix);
        copy
    }
}

/// The surrounding display's settings, or `None` outside one (`useLcdDisplay`).
/// Call it while rendering a part placed on a display's screen, like `LcdMeter`.
pub fn use_lcd_display() -> Option<LcdDisplayContext> {
    use_context::<LcdDisplayContext>()
}

/// The display context, or the defaults outside a display.
pub(crate) fn lcd_context(cx: &App) -> LcdDisplayContext {
    use_lcd_display().unwrap_or_else(|| LcdDisplayContext::fallback(cx))
}

#[derive(Clone)]
struct RowContext {
    label: Rc<RefCell<Option<SharedString>>>,
}

// ---------------------------------------------------------------------------
// The dot grid

/// Where a screen's dot grid starts (its top-left corner), written by the
/// screen while it prepaints and read by the parts on it while they paint.
#[derive(Clone)]
pub(crate) struct DotGrid {
    origin: Rc<Cell<Option<Point<Pixels>>>>,
    pub(crate) pitch: Pixels,
    pub(crate) glass: Hsla,
    /// The screen's dot mask is on (`matrix`).
    pub(crate) matrix: bool,
}

impl DotGrid {
    fn new(pitch: Pixels, glass: Hsla, matrix: bool) -> Self {
        Self {
            origin: Rc::new(Cell::new(None)),
            pitch,
            glass,
            matrix,
        }
    }

    pub(crate) fn origin(&self) -> Option<Point<Pixels>> {
        self.origin.get()
    }

    fn set_origin(&self, origin: Point<Pixels>) {
        self.origin.set(Some(origin));
    }

    /// Punches the screen's mask holes over `bounds` (when the mask is on).
    pub(crate) fn paint_holes(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        if !self.matrix {
            return;
        }
        if let Some(origin) = self.origin() {
            paint_mask_holes(bounds, origin, self.pitch, self.glass, window);
        }
    }
}

/// `before:`'s unlit dots as the browser rasterises them at a scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DotPattern {
    /// Every device pixel is covered alike: a uniform wash at this alpha.
    Uniform(f32),
    /// An opaque square per cell, `offset` from the cell's corner, `size` wide.
    Square { offset: f32, size: f32 },
}

/// The alpha of `radial-gradient(circle, c from%, transparent to%)` at
/// `distance` from the centre of a tile whose farthest corner is `ray` away.
fn radial_alpha(distance: f32, ray: f32, from: f32, to: f32) -> f32 {
    let (a, b) = (ray * from, ray * to);
    if distance <= a {
        1.0
    } else if distance >= b {
        0.0
    } else {
        (b - distance) / (b - a)
    }
}

/// Device pixels per cell, at least one.
fn device_cells(pitch: f32, scale: f32) -> usize {
    ((pitch * scale).round() as usize).max(1)
}

/// The unlit dots (`radial-gradient(circle, ink-unlit 45%, transparent 55%)`
/// on a `pitch` square tile), sampled at device-pixel centres.
pub(crate) fn unlit_dot_pattern(pitch: f32, scale: f32) -> DotPattern {
    let n = device_cells(pitch, scale);
    let step = pitch / n as f32;
    let ray = pitch * std::f32::consts::FRAC_1_SQRT_2;
    let half = pitch / 2.0;
    let alpha = |i: usize, j: usize| {
        let x = (i as f32 + 0.5) * step - half;
        let y = (j as f32 + 0.5) * step - half;
        radial_alpha((x * x + y * y).sqrt(), ray, 0.45, 0.55)
    };
    let first = alpha(0, 0);
    let uniform = (0..n).all(|i| (0..n).all(|j| (alpha(i, j) - first).abs() < 1e-3));
    if uniform {
        return DotPattern::Uniform(first);
    }
    let middle = n / 2;
    let covered: Vec<usize> = (0..n).filter(|&i| alpha(i, middle) >= 0.5).collect();
    match (covered.first(), covered.last()) {
        (Some(&start), Some(&end)) => DotPattern::Square {
            offset: start as f32 * step,
            size: (end - start + 1) as f32 * step,
        },
        _ => DotPattern::Uniform(0.0),
    }
}

/// Half the side of the hole the content mask
/// (`radial-gradient(circle, #000 58%, transparent 64%)`) leaves at each grid
/// intersection, sampled at device-pixel centres. 0 when it leaves none (1x).
pub(crate) fn mask_hole_size(pitch: f32, scale: f32) -> f32 {
    let n = device_cells(pitch, scale);
    let step = pitch / n as f32;
    let ray = pitch * std::f32::consts::FRAC_1_SQRT_2;
    let half = pitch / 2.0;
    let mut holes = 0;
    for i in 0..n / 2 {
        let offset = half - (i as f32 + 0.5) * step;
        let distance = offset * std::f32::consts::SQRT_2;
        if radial_alpha(distance, ray, 0.58, 0.64) < 0.5 {
            holes += 1;
        } else {
            break;
        }
    }
    holes as f32 * step
}

/// How far a rounded rectangle's edge is inset at `distance` from its nearest
/// edge along the other axis (0 past the corner radius).
fn corner_inset(distance: f32, radius: f32) -> f32 {
    if distance >= radius || radius <= 0.0 {
        return 0.0;
    }
    let d = radius - distance.max(0.0);
    radius - (radius * radius - d * d).max(0.0).sqrt()
}

/// A horizontal band `[y0, y1)` of `bounds`, trimmed to a rounded shape.
fn rounded_band(bounds: Bounds<Pixels>, y0: f32, y1: f32, radius: f32) -> Option<Bounds<Pixels>> {
    let top = f32::from(bounds.origin.y);
    let height = f32::from(bounds.size.height);
    let (y0, y1) = (y0.max(0.0), y1.min(height));
    if y1 <= y0 {
        return None;
    }
    let nearest = y0.min(height - y1);
    let inset = corner_inset(nearest, radius);
    let width = f32::from(bounds.size.width) - inset * 2.0;
    (width > 0.0).then(|| {
        Bounds::new(
            point(bounds.origin.x + px(inset), px(top + y0)),
            size(px(width), px(y1 - y0)),
        )
    })
}

/// A vertical band `[x0, x1)` of `bounds`, trimmed to a rounded shape.
fn rounded_column(bounds: Bounds<Pixels>, x0: f32, x1: f32, radius: f32) -> Option<Bounds<Pixels>> {
    let left = f32::from(bounds.origin.x);
    let width = f32::from(bounds.size.width);
    let (x0, x1) = (x0.max(0.0), x1.min(width));
    if x1 <= x0 {
        return None;
    }
    let nearest = x0.min(width - x1);
    let inset = corner_inset(nearest, radius);
    let height = f32::from(bounds.size.height) - inset * 2.0;
    (height > 0.0).then(|| {
        Bounds::new(
            point(px(left + x0), bounds.origin.y + px(inset)),
            size(px(x1 - x0), px(height)),
        )
    })
}

/// The unlit dots over a screen, on a grid starting at its corner.
fn paint_unlit_dots(
    bounds: Bounds<Pixels>,
    radius: f32,
    pitch: f32,
    unlit: Hsla,
    glass: Hsla,
    window: &mut Window,
) {
    if pitch <= 0.0 {
        return;
    }
    match unlit_dot_pattern(pitch, window.scale_factor()) {
        DotPattern::Uniform(alpha) => {
            if alpha > 0.0 {
                window.paint_quad(
                    fill(bounds, unlit.opacity(alpha)).corner_radii(Corners::all(px(radius))),
                );
            }
        }
        DotPattern::Square { offset, size: side } => {
            let height = f32::from(bounds.size.height);
            let width = f32::from(bounds.size.width);
            // Rows of dots, then the glass between them cut back in.
            let mut y = 0.0;
            while y < height {
                if let Some(band) = rounded_band(bounds, y + offset, y + offset + side, radius) {
                    window.paint_quad(fill(band, unlit));
                }
                y += pitch;
            }
            let mut x = -pitch;
            while x < width {
                let from = x + offset + side;
                let to = x + pitch + offset;
                if let Some(column) = rounded_column(bounds, from, to, radius) {
                    window.paint_quad(fill(column, glass));
                }
                x += pitch;
            }
        }
    }
}

/// Glass-coloured squares at every grid intersection inside `bounds`: what
/// the content's dot mask lets through between dots.
pub(crate) fn paint_mask_holes(
    bounds: Bounds<Pixels>,
    origin: Point<Pixels>,
    pitch: Pixels,
    glass: Hsla,
    window: &mut Window,
) {
    let pitch = f32::from(pitch);
    if pitch <= 0.0 {
        return;
    }
    let half = mask_hole_size(pitch, window.scale_factor());
    if half <= 0.0 {
        return;
    }
    let (ox, oy) = (f32::from(origin.x), f32::from(origin.y));
    let (left, top) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (right, bottom) = (
        left + f32::from(bounds.size.width),
        top + f32::from(bounds.size.height),
    );
    let first_x = ((left - half - ox) / pitch).ceil() as i32;
    let last_x = ((right + half - ox) / pitch).floor() as i32;
    let first_y = ((top - half - oy) / pitch).ceil() as i32;
    let last_y = ((bottom + half - oy) / pitch).floor() as i32;
    for j in first_y..=last_y {
        let y = oy + j as f32 * pitch;
        let y0 = (y - half).max(top);
        let y1 = (y + half).min(bottom);
        if y1 <= y0 {
            continue;
        }
        for i in first_x..=last_x {
            let x = ox + i as f32 * pitch;
            let x0 = (x - half).max(left);
            let x1 = (x + half).min(right);
            if x1 <= x0 {
                continue;
            }
            window.paint_quad(fill(
                Bounds::new(point(px(x0), px(y0)), size(px(x1 - x0), px(y1 - y0))),
                glass,
            ));
        }
    }
}

/// `drop-shadow(0 0 3px glow)` under a shape.
pub(crate) fn glow_shadow(glow: Hsla) -> BoxShadow {
    BoxShadow {
        color: glow,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(GLOW_SIGMA),
        spread_radius: px(0.0),
        inset: false,
    }
}

/// The glow's Gaussian sigma (the filter's 3px blur).
const GLOW_SIGMA: f32 = 1.5;

/// Paints a lit shape with its glow.
pub(crate) fn paint_lit_quad(
    bounds: Bounds<Pixels>,
    radius: Pixels,
    color: Hsla,
    glow: Option<Hsla>,
    window: &mut Window,
) {
    if let Some(glow) = glow.filter(|glow| glow.a > 0.0 && color.a > 0.0) {
        let glow = glow.opacity(color.a);
        window.paint_drop_shadows(bounds, Corners::all(radius), &[glow_shadow(glow)]);
    }
    window.paint_quad(fill(bounds, color).corner_radii(Corners::all(radius)));
}

// ---------------------------------------------------------------------------
// Text

/// Single-line LCD text: the display's font with `tracking-[0.12em]`,
/// `leading-none`, a glow, and optionally the screen's mask holes on top.
pub(crate) struct LcdText {
    text: SharedString,
    font: SharedString,
    font_size: Pixels,
    line_height: Pixels,
    tracking: Pixels,
    weight: FontWeight,
    color: Hsla,
    glow: Option<Hsla>,
    holes: Option<DotGrid>,
}

impl LcdText {
    /// Text in the display's font at its size, uppercased like `uppercase`.
    pub(crate) fn new(text: impl Into<SharedString>, ctx: &LcdDisplayContext, color: Hsla) -> Self {
        let text: SharedString = text.into();
        Self {
            text: text.to_uppercase().into(),
            font: ctx.palette.font.clone(),
            font_size: ctx.text_size(),
            line_height: ctx.text_size(),
            tracking: ctx.tracking(),
            weight: ctx.palette.font_weight,
            color,
            glow: Some(ctx.palette.glow),
            holes: Some(ctx.grid.clone()),
        }
    }

    pub(crate) fn font_size(mut self, font_size: Pixels) -> Self {
        self.font_size = font_size;
        self.line_height = font_size;
        self
    }

    pub(crate) fn holes(mut self, holes: Option<DotGrid>) -> Self {
        self.holes = holes;
        self
    }

    pub(crate) fn glow(mut self, glow: Option<Hsla>) -> Self {
        self.glow = glow;
        self
    }

    fn shape(&self, window: &Window) -> ShapedLine {
        shape_lcd_text(&self.text, &self.font, self.weight, self.font_size, window)
    }
}

fn shape_lcd_text(
    text: &SharedString,
    family: &SharedString,
    weight: FontWeight,
    font_size: Pixels,
    window: &Window,
) -> ShapedLine {
    let mut font = gpui::font(family.clone());
    font.weight = weight;
    let run = TextRun {
        len: text.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.clone(), font_size, &[run], None)
}

/// The width LCD text takes: its advance plus the tracking after every character.
pub(crate) fn lcd_text_width(
    text: &str,
    palette: &LcdDisplayPalette,
    font_size: Pixels,
    tracking: Pixels,
    window: &Window,
) -> Pixels {
    let text: SharedString = text.to_uppercase().into();
    let line = shape_lcd_text(&text, &palette.font, palette.font_weight, font_size, window);
    line.width + tracking * text.chars().count() as f32
}

/// Offsets and weights of the soft copies that draw text's glow: three rings
/// approximating the filter's Gaussian.
const GLOW_TAPS: [(f32, f32, f32); 24] = {
    const R: [f32; 3] = [1.5, 3.0, 4.5];
    const W: [f32; 3] = [0.075, 0.04, 0.015];
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let mut taps = [(0.0, 0.0, 0.0); 24];
    let mut ring = 0;
    while ring < 3 {
        let (r, w) = (R[ring], W[ring]);
        let dirs = [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (D, D),
            (-D, D),
            (D, -D),
            (-D, -D),
        ];
        let mut i = 0;
        while i < 8 {
            taps[ring * 8 + i] = (dirs[i].0 * r, dirs[i].1 * r, w);
            i += 1;
        }
        ring += 1;
    }
    taps
};

#[allow(clippy::too_many_arguments)]
fn paint_shaped(
    line: &ShapedLine,
    text: &str,
    origin: Point<Pixels>,
    line_height: Pixels,
    tracking: Pixels,
    color: Hsla,
    glow: Option<Hsla>,
    window: &mut Window,
) {
    let padding_top = (line_height - line.ascent - line.descent) / 2.0;
    let baseline = origin.y + padding_top + line.ascent;
    let mut glyphs = Vec::new();
    for run in &line.runs {
        for glyph in &run.glyphs {
            let chars = text.get(..glyph.index).map_or(0, |s| s.chars().count());
            let x = origin.x + glyph.position.x + tracking * chars as f32;
            glyphs.push((run.font_id, glyph.id, point(x, baseline + glyph.position.y)));
        }
    }
    if let Some(glow) = glow.filter(|glow| glow.a > 0.0 && color.a > 0.0) {
        let glow = glow.opacity(color.a);
        for (dx, dy, weight) in GLOW_TAPS {
            let tap = glow.opacity(weight);
            for (font_id, glyph_id, at) in &glyphs {
                let at = point(at.x + px(dx), at.y + px(dy));
                window
                    .paint_glyph(at, *font_id, *glyph_id, line.font_size, tap)
                    .ok();
            }
        }
    }
    for (font_id, glyph_id, at) in glyphs {
        window
            .paint_glyph(at, font_id, glyph_id, line.font_size, color)
            .ok();
    }
}

impl IntoElement for LcdText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for LcdText {
    type RequestLayoutState = ShapedLine;
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
    ) -> (LayoutId, ShapedLine) {
        let line = self.shape(window);
        let width = line.width + self.tracking * self.text.chars().count() as f32;
        let mut style = Style::default();
        style.size.width = width.into();
        style.size.height = self.line_height.into();
        style.flex_shrink = 0.0;
        (window.request_layout(style, [], cx), line)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut ShapedLine,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        line: &mut ShapedLine,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        paint_shaped(
            line,
            &self.text,
            bounds.origin,
            self.line_height,
            self.tracking,
            self.color,
            self.glow,
            window,
        );
        if let Some(grid) = &self.holes {
            grid.paint_holes(bounds, window);
        }
    }
}

/// Paints LCD text at `origin` (for canvases that lay text out themselves).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_lcd_text(
    text: &str,
    palette: &LcdDisplayPalette,
    font_size: Pixels,
    tracking: Pixels,
    origin: Point<Pixels>,
    color: Hsla,
    glow: Option<Hsla>,
    window: &mut Window,
) -> Bounds<Pixels> {
    let text: SharedString = text.to_uppercase().into();
    let line = shape_lcd_text(&text, &palette.font, palette.font_weight, font_size, window);
    paint_shaped(
        &line, &text, origin, font_size, tracking, color, glow, window,
    );
    Bounds::new(
        origin,
        size(
            line.width + tracking * text.chars().count() as f32,
            font_size,
        ),
    )
}

// ---------------------------------------------------------------------------
// Change flicker

/// Two frames dimmed before the fade back starts (`useChangeFlicker`).
const FLICKER_HOLD_MS: f64 = 33.0;
/// `transition-opacity duration-150`.
const FLICKER_FADE_MS: f64 = 150.0;
/// `data-changed:opacity-35`.
const FLICKER_OPACITY: f32 = 0.35;

/// Dims a part for a moment when its text changes, like segments redrawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Flicker {
    shown: Option<SharedString>,
    changed_at_ms: Option<f64>,
}

impl Flicker {
    /// Records the text shown at `now_ms`; a change starts the flicker.
    pub(crate) fn update(&mut self, text: &SharedString, now_ms: f64) {
        match &self.shown {
            Some(shown) if shown == text => {}
            Some(_) => {
                self.shown = Some(text.clone());
                self.changed_at_ms = Some(now_ms);
            }
            None => self.shown = Some(text.clone()),
        }
    }

    /// Whether the part is dimmed (`data-changed`) at `now_ms`.
    #[cfg(test)]
    pub(crate) fn changed(&self, now_ms: f64) -> bool {
        self.changed_at_ms
            .is_some_and(|at| now_ms - at < FLICKER_HOLD_MS)
    }

    /// The opacity at `now_ms`, and whether it is still animating.
    pub(crate) fn opacity(&self, now_ms: f64, reduced_motion: bool) -> (f32, bool) {
        let Some(at) = self.changed_at_ms else {
            return (1.0, false);
        };
        if reduced_motion {
            return (1.0, false);
        }
        let elapsed = now_ms - at;
        if elapsed < FLICKER_HOLD_MS {
            return (FLICKER_OPACITY, true);
        }
        let t = ((elapsed - FLICKER_HOLD_MS) / FLICKER_FADE_MS) as f32;
        if t >= 1.0 {
            return (1.0, false);
        }
        let eased = Easing::STANDARD.apply(t);
        (FLICKER_OPACITY + (1.0 - FLICKER_OPACITY) * eased, true)
    }
}

fn now_ms() -> f64 {
    crate::audio::frame_loop::now_ms()
}

/// Updates a keyed flicker and returns the opacity to draw with.
fn flicker_opacity(id: ElementId, text: &SharedString, window: &mut Window, cx: &mut App) -> f32 {
    let state = window.use_keyed_state(id, cx, |_, _| Flicker::default());
    let now = now_ms();
    let reduced = prefers_reduced_motion(cx);
    let (opacity, animating) = state.update(cx, |flicker, _| {
        flicker.update(text, now);
        flicker.opacity(now, reduced)
    });
    if animating {
        window.request_animation_frame();
    }
    opacity
}

// ---------------------------------------------------------------------------
// The screen

/// The glass: unlit dots behind, scanlines in front, the content between.
fn lcd_screen(ctx: &LcdDisplayContext, content: gpui::Div, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    let radius = f32::from(radius_md(theme));
    let palette = ctx.palette.clone();
    let grid = ctx.grid.clone();
    let matrix = ctx.matrix;
    let scanlines = ctx.scanlines;
    let back_palette = palette.clone();
    div()
        .relative()
        .overflow_hidden()
        .rounded(px(radius))
        .bg(palette.glass)
        .child(
            canvas(
                move |bounds, _, _| {
                    grid.set_origin(bounds.origin);
                },
                move |bounds, _, window, _| {
                    let p = &back_palette;
                    if matrix {
                        paint_unlit_dots(
                            bounds,
                            radius,
                            f32::from(p.pitch),
                            p.ink_unlit,
                            p.glass,
                            window,
                        );
                    }
                    window.paint_inset_shadows(bounds, Corners::all(px(radius)), &p.recess);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(content.relative())
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    paint_sheen(bounds, radius, &palette, scanlines, window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}

/// `after:`: the sheen, and the scanlines when on.
fn paint_sheen(
    bounds: Bounds<Pixels>,
    radius: f32,
    palette: &LcdDisplayPalette,
    scanlines: bool,
    window: &mut Window,
) {
    let height = f32::from(bounds.size.height);
    // linear-gradient(to bottom, sheen, transparent 45%)
    let sheen = Bounds::new(bounds.origin, size(bounds.size.width, px(height * 0.45)));
    window.paint_quad(
        fill(
            sheen,
            linear_gradient(
                180.0,
                linear_color_stop(palette.sheen, 0.0),
                linear_color_stop(palette.sheen.opacity(0.0), 1.0),
            ),
        )
        .corner_radii(Corners {
            top_left: px(radius),
            top_right: px(radius),
            bottom_left: px(0.0),
            bottom_right: px(0.0),
        }),
    );
    if scanlines {
        // repeating-linear-gradient(to bottom, transparent 0 2px, scanline 2px 3px)
        let mut y = 2.0;
        while y < height {
            if let Some(band) = rounded_band(bounds, y, y + 1.0, radius) {
                window.paint_quad(fill(band, palette.scanline));
            }
            y += 3.0;
        }
    }
}

// ---------------------------------------------------------------------------
// LcdDisplay

/// An LCD screen in a bezel: backlit with dark pixels in the light theme, and
/// glowing ink on dark glass in the dark one. Its ink is
/// [`ink`](LcdDisplay::ink) (`--lcd-display-ink`).
///
/// ```ignore
/// LcdDisplay::new("tempo")
///     .variant(LcdDisplayVariant::Amber)
///     .child(
///         LcdDisplayRow::new()
///             .child(LcdDisplayLabel::new("Tempo"))
///             .child(LcdDisplayValue::new("tempo-value", "128.0")),
///     )
/// ```
#[derive(IntoElement)]
pub struct LcdDisplay {
    id: ElementId,
    variant: LcdDisplayVariant,
    appearance: LcdDisplayAppearance,
    size: LcdDisplaySize,
    matrix: bool,
    scanlines: bool,
    vars: LcdDisplayVars,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LcdDisplay {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            variant: LcdDisplayVariant::Primary,
            appearance: LcdDisplayAppearance::Auto,
            size: LcdDisplaySize::Default,
            matrix: true,
            scanlines: true,
            vars: LcdDisplayVars::default(),
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// The ink colour. Default `Primary`.
    pub fn variant(mut self, variant: LcdDisplayVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Default `Auto`.
    pub fn appearance(mut self, appearance: LcdDisplayAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    /// The text size; rows, separator and status follow it. Default `Default`.
    pub fn size(mut self, size: LcdDisplaySize) -> Self {
        self.size = size;
        self
    }

    /// Break the text into dots on a `pitch` grid. Default true.
    pub fn matrix(mut self, matrix: bool) -> Self {
        self.matrix = matrix;
        self
    }

    /// Faint horizontal scanlines over the glass. Default true.
    pub fn scanlines(mut self, scanlines: bool) -> Self {
        self.scanlines = scanlines;
        self
    }

    /// Sets every `--lcd-display-*` variable at once.
    pub fn vars(mut self, vars: LcdDisplayVars) -> Self {
        self.vars = vars;
        self
    }

    /// `--lcd-display-ink`.
    pub fn ink(mut self, ink: impl Into<Hsla>) -> Self {
        self.vars.ink = Some(ink.into());
        self
    }

    /// `--lcd-display-lit`.
    pub fn lit(mut self, lit: impl Into<Hsla>) -> Self {
        self.vars.lit = Some(lit.into());
        self
    }

    /// `--lcd-display-ink-dim`.
    pub fn ink_dim(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.ink_dim = Some(color.into());
        self
    }

    /// `--lcd-display-ink-unlit`.
    pub fn ink_unlit(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.ink_unlit = Some(color.into());
        self
    }

    /// `--lcd-display-glass`.
    pub fn glass(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.glass = Some(color.into());
        self
    }

    /// `--lcd-display-glow`.
    pub fn glow(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.glow = Some(color.into());
        self
    }

    /// `--lcd-display-bezel`.
    pub fn bezel(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.bezel = Some(color.into());
        self
    }

    /// `--lcd-display-bezel-edge`.
    pub fn bezel_edge(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.bezel_edge = Some(color.into());
        self
    }

    /// `--lcd-display-bezel-ring`.
    pub fn bezel_ring(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.bezel_ring = Some(color.into());
        self
    }

    /// `--lcd-display-bezel-shadow`.
    pub fn bezel_shadow(mut self, shadow: Vec<BoxShadow>) -> Self {
        self.vars.bezel_shadow = Some(shadow);
        self
    }

    /// `--lcd-display-recess`.
    pub fn recess(mut self, shadow: Vec<BoxShadow>) -> Self {
        self.vars.recess = Some(shadow);
        self
    }

    /// `--lcd-display-scanline`.
    pub fn scanline(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.scanline = Some(color.into());
        self
    }

    /// `--lcd-display-sheen`.
    pub fn sheen(mut self, color: impl Into<Hsla>) -> Self {
        self.vars.sheen = Some(color.into());
        self
    }

    /// `--lcd-display-font`: a font family, e.g. `cx.theme().font_doto`.
    pub fn font(mut self, family: impl Into<SharedString>) -> Self {
        self.vars.font = Some(family.into());
        self
    }

    /// `--lcd-display-pitch`. Default 2px.
    pub fn pitch(mut self, pitch: Pixels) -> Self {
        self.vars.pitch = Some(pitch);
        self
    }

    /// The weight of the screen's text. Default normal.
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.vars.font_weight = Some(weight);
        self
    }
}

impl ParentElement for LcdDisplay {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for LcdDisplay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// `ring-1 ring-color` as a spread shadow.
fn ring(color: Hsla) -> BoxShadow {
    css_shadow(0.0, 0.0, 0.0, 1.0, color, false)
}

impl RenderOnce for LcdDisplay {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = LcdDisplayContext::new(
            self.appearance,
            self.variant,
            self.size,
            self.matrix,
            self.scanlines,
            &self.vars,
            cx.theme(),
        );
        let p = &ctx.palette;
        let text_size = ctx.text_size();
        let mut shadows = vec![ring(p.bezel_ring)];
        shadows.extend(p.bezel_shadow.iter().cloned());
        let content = div()
            .flex()
            .flex_col()
            .px(px(16.0))
            .py(px(8.0))
            .children(self.children);
        let screen = lcd_screen(&ctx, content, cx);
        let mut root = div()
            .id(self.id)
            .role(gpui::Role::Group)
            .rounded(radius_lg(cx.theme()))
            .bg(linear_gradient(
                180.0,
                linear_color_stop(p.bezel, 0.0),
                linear_color_stop(p.bezel_edge, 1.0),
            ))
            .p(px(6.0))
            .font_family(p.font.clone())
            .text_size(text_size)
            .line_height(text_size)
            .text_color(p.lit)
            .shadow(shadows)
            .child(screen);
        root.style().refine(&self.style);
        provide(ctx, root)
    }
}

// ---------------------------------------------------------------------------
// Rows

/// One line of the screen: a label on the left, a value on the right.
#[derive(IntoElement)]
pub struct LcdDisplayRow {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl LcdDisplayRow {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl Default for LcdDisplayRow {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for LcdDisplayRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for LcdDisplayRow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LcdDisplayRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(32.0))
            .py(px(12.0))
            .children(self.children);
        row.style().refine(&self.style);
        provide(
            RowContext {
                label: Rc::new(RefCell::new(None)),
            },
            row,
        )
    }
}

/// Dim text that names a row, and the select in it.
#[derive(IntoElement)]
pub struct LcdDisplayLabel {
    text: SharedString,
    color: Option<Hsla>,
    style: StyleRefinement,
}

impl LcdDisplayLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            color: None,
            style: StyleRefinement::default(),
        }
    }

    /// Overrides `--lcd-display-ink-dim` for this label.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl Styled for LcdDisplayLabel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LcdDisplayLabel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = lcd_context(cx);
        // The row's select is named by this label.
        if let Some(row) = use_context::<RowContext>() {
            *row.label.borrow_mut() = Some(self.text.clone());
        }
        let color = self.color.unwrap_or(ctx.palette.ink_dim);
        let mut el = div().flex().child(LcdText::new(self.text, &ctx, color));
        el.style().refine(&self.style);
        el
    }
}

/// Bright, glowing text that flickers when it changes.
#[derive(IntoElement)]
pub struct LcdDisplayValue {
    id: ElementId,
    text: SharedString,
    live: bool,
    color: Option<Hsla>,
    style: StyleRefinement,
}

impl LcdDisplayValue {
    /// The id keeps the flicker's state; unique within the display.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            live: false,
            color: None,
            style: StyleRefinement::default(),
        }
    }

    /// Announce changes to screen readers (`aria-live="polite"`). Default false.
    /// GPUI's accessibility tree has no live regions yet; the value is exposed
    /// as a labelled status instead.
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// Overrides `--lcd-display-lit` for this value.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl Styled for LcdDisplayValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LcdDisplayValue {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = lcd_context(cx);
        let opacity = flicker_opacity(self.id.clone(), &self.text, window, cx);
        let color = self.color.unwrap_or(ctx.palette.lit).opacity(opacity);
        let mut el = div()
            .id(self.id)
            .flex()
            .when(self.live, |el| {
                el.role(gpui::Role::Status).aria_label(self.text.clone())
            })
            .child(LcdText::new(self.text, &ctx, color));
        el.style().refine(&self.style);
        el
    }
}

/// A dotted rule between rows.
#[derive(IntoElement)]
pub struct LcdDisplaySeparator {
    style: StyleRefinement,
}

impl LcdDisplaySeparator {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl Default for LcdDisplaySeparator {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for LcdDisplaySeparator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The separator's dots (`radial-gradient(circle, dim 45%, transparent 55%)`
/// on a `2·pitch × pitch` tile), sampled per device pixel: the covered span
/// of a tile row, as (offset, width) in pixels.
fn separator_dot(pitch: f32, scale: f32) -> (f32, f32) {
    let n = device_cells(pitch * 2.0, scale);
    let step = pitch * 2.0 / n as f32;
    let ray = (pitch * pitch + pitch * pitch / 4.0).sqrt();
    let y = if device_cells(pitch, scale).is_multiple_of(2) {
        step / 2.0
    } else {
        0.0
    };
    let covered: Vec<usize> = (0..n)
        .filter(|&i| {
            let x = (i as f32 + 0.5) * step - pitch;
            radial_alpha((x * x + y * y).sqrt(), ray, 0.45, 0.55) >= 0.5
        })
        .collect();
    match (covered.first(), covered.last()) {
        (Some(&a), Some(&b)) => (a as f32 * step, (b - a + 1) as f32 * step),
        _ => (0.0, 0.0),
    }
}

impl RenderOnce for LcdDisplaySeparator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = lcd_context(cx);
        let pitch = ctx.palette.pitch;
        let dim = ctx.palette.ink_dim;
        let glow = ctx.palette.glow;
        let grid = ctx.grid.clone();
        let mut el = div().w_full().h(pitch).flex_shrink_0().child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let p = f32::from(pitch);
                    let (offset, width) = separator_dot(p, window.scale_factor());
                    if width > 0.0 {
                        let tile = p * 2.0;
                        let total = f32::from(bounds.size.width);
                        let mut x = 0.0;
                        while x < total {
                            let x0 = x + offset;
                            let x1 = (x0 + width).min(total);
                            if x1 > x0 {
                                let dot = Bounds::new(
                                    point(bounds.origin.x + px(x0), bounds.origin.y),
                                    size(px(x1 - x0), bounds.size.height),
                                );
                                paint_lit_quad(dot, px(0.0), dim, Some(glow), window);
                            }
                            x += tile;
                        }
                    }
                    grid.paint_holes(bounds, window);
                },
            )
            .size_full(),
        );
        el.style().refine(&self.style);
        el
    }
}

/// `animate-pulse`: opacity 1 → 0.5 → 1 with `cubic-bezier(0.4, 0, 0.6, 1)`.
fn pulse_opacity(elapsed_ms: f64, period_ms: f64) -> f32 {
    let t = (elapsed_ms.rem_euclid(period_ms) / period_ms) as f32;
    let curve = Easing::CubicBezier(0.4, 0.0, 0.6, 1.0);
    if t < 0.5 {
        1.0 - 0.5 * curve.apply(t * 2.0)
    } else {
        0.5 + 0.5 * curve.apply((t - 0.5) * 2.0)
    }
}

/// The blinking pixel in the bottom-right corner of the screen.
#[derive(IntoElement)]
pub struct LcdDisplayStatus {
    state: LcdDisplayStatusState,
    style: StyleRefinement,
}

impl LcdDisplayStatus {
    pub fn new() -> Self {
        Self {
            state: LcdDisplayStatusState::Idle,
            style: StyleRefinement::default(),
        }
    }

    /// Default `Idle`.
    pub fn state(mut self, state: LcdDisplayStatusState) -> Self {
        self.state = state;
        self
    }
}

impl Default for LcdDisplayStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for LcdDisplayStatus {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LcdDisplayStatus {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = lcd_context(cx);
        let reduced = prefers_reduced_motion(cx);
        let (color, opacity) = match self.state {
            LcdDisplayStatusState::Off => (ctx.palette.ink_unlit, 1.0),
            state => {
                let period = if state == LcdDisplayStatusState::Busy {
                    600.0
                } else {
                    2000.0
                };
                if reduced {
                    (ctx.palette.lit, 1.0)
                } else {
                    window.request_animation_frame();
                    (ctx.palette.lit, pulse_opacity(now_ms(), period))
                }
            }
        };
        let glow = ctx.palette.glow;
        let grid = ctx.grid.clone();
        let mut el = div()
            .absolute()
            .right(px(8.0))
            .bottom(px(8.0))
            .size(px(6.0))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        paint_lit_quad(bounds, px(0.0), color.opacity(opacity), Some(glow), window);
                        grid.paint_holes(bounds, window);
                    },
                )
                .size_full(),
            );
        el.style().refine(&self.style);
        el
    }
}

// ---------------------------------------------------------------------------
// Select

type ValueHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;
type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

/// Which side of the trigger the popup opens on. Default `Bottom`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplaySelectSide {
    Top,
    Right,
    #[default]
    Bottom,
    Left,
}

/// How the popup lines up with the trigger along its side. Default `End`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LcdDisplaySelectAlign {
    Start,
    Center,
    #[default]
    End,
}

/// One choice: a value and the text shown for it.
#[derive(Clone, Debug, PartialEq)]
pub struct LcdDisplaySelectOption {
    pub value: SharedString,
    pub label: SharedString,
    pub disabled: bool,
}

/// Where the highlight moves for a key, among the enabled choices. Arrow keys
/// stop at the ends; `Home` / `End` jump there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListMove {
    Next,
    Previous,
    First,
    Last,
}

/// The highlight after a [`ListMove`], skipping disabled choices.
fn move_highlight(disabled: &[bool], current: Option<usize>, movement: ListMove) -> Option<usize> {
    let enabled = |index: &usize| !disabled.get(*index).copied().unwrap_or(true);
    let count = disabled.len();
    match movement {
        ListMove::First => (0..count).find(enabled),
        ListMove::Last => (0..count).rev().find(enabled),
        ListMove::Next => match current {
            None => (0..count).find(enabled),
            Some(current) => (current + 1..count).find(enabled).or(Some(current)),
        },
        ListMove::Previous => match current {
            None => (0..count).rev().find(enabled),
            Some(current) => (0..current).rev().find(enabled).or(Some(current)),
        },
    }
}

/// Typeahead: the first enabled choice after `current` (wrapping) whose label
/// starts with `query`, ignoring case.
fn typeahead_match(
    labels: &[SharedString],
    disabled: &[bool],
    query: &str,
    current: Option<usize>,
) -> Option<usize> {
    let query = query.to_lowercase();
    if query.is_empty() || labels.is_empty() {
        return None;
    }
    let count = labels.len();
    // A repeated single letter cycles; a longer query keeps the current match.
    let start = match current {
        Some(current) if query.chars().count() == 1 => current + 1,
        Some(current) => current,
        None => 0,
    };
    (0..count)
        .map(|offset| (start + offset) % count)
        .find(|&index| {
            !disabled.get(index).copied().unwrap_or(false)
                && labels[index].to_lowercase().starts_with(&query)
        })
}

/// Base UI's typeahead resets after this pause.
const TYPEAHEAD_RESET_MS: f64 = 750.0;
/// `duration-100` on the popup.
const POPUP_FADE: Duration = Duration::from_millis(100);
/// `duration-150` on the chevron.
const CHEVRON_TURN: Duration = Duration::from_millis(150);

struct SelectState {
    value: Option<SharedString>,
    open: bool,
    highlighted: Option<usize>,
    trigger_focus: FocusHandle,
    list_focus: FocusHandle,
    trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    popup_width: Rc<Cell<Option<Pixels>>>,
    fade: Transition,
    chevron: Transition,
    flicker: Flicker,
    query: String,
    query_at_ms: f64,
    started: bool,
}

impl SelectState {
    /// Opens or closes the popup: fades it, turns the chevron, and on open
    /// highlights the selected choice.
    fn apply_open(
        &mut self,
        open: bool,
        reduced: bool,
        options: &[LcdDisplaySelectOption],
    ) -> bool {
        if self.open == open {
            return false;
        }
        self.open = open;
        let duration = |d: Duration| if reduced { Duration::ZERO } else { d };
        let target = if open { 1.0 } else { 0.0 };
        self.fade
            .animate_to(target, duration(POPUP_FADE), Easing::EASE);
        self.chevron
            .animate_to(target, duration(CHEVRON_TURN), Easing::STANDARD);
        if open {
            let disabled: Vec<bool> = options.iter().map(|o| o.disabled).collect();
            self.highlighted = self
                .value
                .as_ref()
                .and_then(|v| options.iter().position(|o| &o.value == v))
                .or_else(|| move_highlight(&disabled, None, ListMove::First));
            self.query.clear();
        }
        true
    }
}

/// A row value the user can change: Base UI's `Select` drawn on the screen.
/// Takes a [`LcdDisplaySelectTrigger`] and a [`LcdDisplaySelectContent`].
///
/// ```ignore
/// LcdDisplaySelect::new("model")
///     .items([("opus", "Opus 5.5"), ("sonnet", "Sonnet 5.5")])
///     .value(model)
///     .on_value_change(|value, _, cx| ...)
///     .child(LcdDisplaySelectTrigger::new())
///     .child(LcdDisplaySelectContent::new().children([
///         LcdDisplaySelectItem::new("opus", "Opus 5.5"),
///         LcdDisplaySelectItem::new("sonnet", "Sonnet 5.5"),
///     ]))
/// ```
#[derive(IntoElement)]
pub struct LcdDisplaySelect {
    id: ElementId,
    items: Vec<(SharedString, SharedString)>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    disabled: bool,
    on_value_change: Option<ValueHandler>,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenHandler>,
    trigger: Option<LcdDisplaySelectTrigger>,
    content: Option<LcdDisplaySelectContent>,
}

/// A part of a [`LcdDisplaySelect`]: its trigger or its content.
pub enum LcdDisplaySelectPart {
    Trigger(LcdDisplaySelectTrigger),
    Content(LcdDisplaySelectContent),
}

impl From<LcdDisplaySelectTrigger> for LcdDisplaySelectPart {
    fn from(trigger: LcdDisplaySelectTrigger) -> Self {
        Self::Trigger(trigger)
    }
}

impl From<LcdDisplaySelectContent> for LcdDisplaySelectPart {
    fn from(content: LcdDisplaySelectContent) -> Self {
        Self::Content(content)
    }
}

impl LcdDisplaySelect {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            value: None,
            default_value: None,
            disabled: false,
            on_value_change: None,
            open: None,
            default_open: false,
            on_open_change: None,
            trigger: None,
            content: None,
        }
    }

    /// Controlled open state of the popup.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Whether the popup starts open when uncontrolled. Default false.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Values and the labels the trigger shows for them (`items`).
    pub fn items<V, L>(mut self, items: impl IntoIterator<Item = (V, L)>) -> Self
    where
        V: Into<SharedString>,
        L: Into<SharedString>,
    {
        self.items = items
            .into_iter()
            .map(|(value, label)| (value.into(), label.into()))
            .collect();
        self
    }

    /// Controlled value.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Initial value when uncontrolled.
    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_value_change(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(handler));
        self
    }

    /// Adds the trigger or the content.
    pub fn child(mut self, part: impl Into<LcdDisplaySelectPart>) -> Self {
        match part.into() {
            LcdDisplaySelectPart::Trigger(trigger) => self.trigger = Some(trigger),
            LcdDisplaySelectPart::Content(content) => self.content = Some(content),
        }
        self
    }
}

/// The selected value and a pixel chevron. Named by the row's label.
#[derive(Default)]
pub struct LcdDisplaySelectTrigger {
    placeholder: Option<SharedString>,
    text: Option<SharedString>,
    aria_label: Option<SharedString>,
    style: StyleRefinement,
}

impl LcdDisplaySelectTrigger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shown while nothing is selected.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Text shown instead of the selected value (React's `children`).
    pub fn child(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Names the trigger outside a row (`aria-label`).
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl Styled for LcdDisplaySelectTrigger {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The list of choices, drawn as a small screen in the display's ink.
pub struct LcdDisplaySelectContent {
    align: LcdDisplaySelectAlign,
    align_offset: f32,
    side: LcdDisplaySelectSide,
    side_offset: f32,
    items: Vec<LcdDisplaySelectItem>,
    style: StyleRefinement,
}

impl Default for LcdDisplaySelectContent {
    fn default() -> Self {
        Self {
            align: LcdDisplaySelectAlign::End,
            align_offset: 0.0,
            side: LcdDisplaySelectSide::Bottom,
            side_offset: 8.0,
            items: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl LcdDisplaySelectContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default `End`.
    pub fn align(mut self, align: LcdDisplaySelectAlign) -> Self {
        self.align = align;
        self
    }

    /// Default 0.
    pub fn align_offset(mut self, offset: f32) -> Self {
        self.align_offset = offset;
        self
    }

    /// Default `Bottom`.
    pub fn side(mut self, side: LcdDisplaySelectSide) -> Self {
        self.side = side;
        self
    }

    /// Default 8.
    pub fn side_offset(mut self, offset: f32) -> Self {
        self.side_offset = offset;
        self
    }

    pub fn child(mut self, item: LcdDisplaySelectItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = LcdDisplaySelectItem>) -> Self {
        self.items.extend(items);
        self
    }
}

impl Styled for LcdDisplaySelectContent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One choice. Highlighted choices invert, like an LCD menu.
#[derive(Clone, Debug, PartialEq)]
pub struct LcdDisplaySelectItem {
    value: SharedString,
    label: SharedString,
    disabled: bool,
}

impl LcdDisplaySelectItem {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Where the popup goes, as `anchored` wants it: a point and the popup's
/// corner placed there.
fn popup_anchor(
    trigger: Bounds<Pixels>,
    side: LcdDisplaySelectSide,
    align: LcdDisplaySelectAlign,
    side_offset: f32,
    align_offset: f32,
    popup_width: Option<Pixels>,
) -> (Point<Pixels>, gpui::Anchor) {
    use LcdDisplaySelectAlign as A;
    use LcdDisplaySelectSide as S;
    use gpui::Anchor;
    let (so, ao) = (px(side_offset), px(align_offset));
    match side {
        S::Bottom | S::Top => {
            let y = if side == S::Bottom {
                trigger.bottom() + so
            } else {
                trigger.top() - so
            };
            let (x, corner) = match (align, side) {
                (A::Start, S::Bottom) => (trigger.left() + ao, Anchor::TopLeft),
                (A::Start, _) => (trigger.left() + ao, Anchor::BottomLeft),
                (A::End, S::Bottom) => (trigger.right() - ao, Anchor::TopRight),
                (A::End, _) => (trigger.right() - ao, Anchor::BottomRight),
                (A::Center, S::Bottom) => (trigger.center().x + ao, Anchor::TopCenter),
                (A::Center, _) => (trigger.center().x + ao, Anchor::BottomCenter),
            };
            let _ = popup_width;
            (point(x, y), corner)
        }
        S::Left | S::Right => {
            let x = if side == S::Right {
                trigger.right() + so
            } else {
                trigger.left() - so
            };
            let (y, corner) = match (align, side) {
                (A::Start, S::Right) => (trigger.top() + ao, Anchor::TopLeft),
                (A::Start, _) => (trigger.top() + ao, Anchor::TopRight),
                (A::End, S::Right) => (trigger.bottom() - ao, Anchor::BottomLeft),
                (A::End, _) => (trigger.bottom() - ao, Anchor::BottomRight),
                (A::Center, S::Right) => (trigger.center().y + ao, Anchor::TopLeft),
                (A::Center, _) => (trigger.center().y + ao, Anchor::TopRight),
            };
            (point(x, y), corner)
        }
    }
}

/// The 5 × 3 pixel chevron, pixel by pixel.
const CHEVRON: [(f32, f32); 5] = [(0.0, 0.0), (4.0, 0.0), (1.0, 1.0), (3.0, 1.0), (2.0, 2.0)];

fn paint_chevron(bounds: Bounds<Pixels>, turn: f32, color: Hsla, glow: Hsla, window: &mut Window) {
    // viewBox 0 0 5 3 in a 10 × 6 box: 2px pixels, turned about the centre.
    let unit = f32::from(bounds.size.width) / 5.0;
    let center = bounds.center();
    let (sin, cos) = (turn * std::f32::consts::PI).sin_cos();
    for (x, y) in CHEVRON {
        let dx = (x + 0.5 - 2.5) * unit;
        let dy = (y + 0.5 - 1.5) * unit;
        let rx = dx * cos - dy * sin;
        let ry = dx * sin + dy * cos;
        let pixel = Bounds::new(
            point(
                center.x + px(rx - unit / 2.0),
                center.y + px(ry - unit / 2.0),
            ),
            size(px(unit), px(unit)),
        );
        paint_lit_quad(pixel, px(0.0), color, Some(glow), window);
    }
}

impl RenderOnce for LcdDisplaySelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = lcd_context(cx);
        let row_label = use_context::<RowContext>().and_then(|row| row.label.borrow().clone());
        let initial = self.value.clone().or_else(|| self.default_value.clone());
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, cx: &mut Context<SelectState>| {
                SelectState {
                    value: initial,
                    open: false,
                    highlighted: None,
                    trigger_focus: cx.focus_handle().tab_stop(true),
                    list_focus: cx.focus_handle(),
                    trigger_bounds: Rc::new(Cell::new(None)),
                    popup_width: Rc::new(Cell::new(None)),
                    fade: Transition::new(0.0),
                    chevron: Transition::new(0.0),
                    flicker: Flicker::default(),
                    query: String::new(),
                    query_at_ms: 0.0,
                    started: false,
                }
            });
        if let Some(value) = self.value.clone() {
            state.update(cx, |s, _| s.value = Some(value));
        }

        let content = self.content.unwrap_or_default();
        let trigger = self.trigger.unwrap_or_default();
        let options: Rc<Vec<LcdDisplaySelectOption>> = Rc::new(
            content
                .items
                .iter()
                .map(|item| LcdDisplaySelectOption {
                    value: item.value.clone(),
                    label: item.label.clone(),
                    disabled: item.disabled,
                })
                .collect(),
        );
        let label_for = |value: &SharedString| -> SharedString {
            self.items
                .iter()
                .find(|(v, _)| v == value)
                .map(|(_, label)| label.clone())
                .or_else(|| {
                    options
                        .iter()
                        .find(|o| &o.value == value)
                        .map(|o| o.label.clone())
                })
                .unwrap_or_else(|| value.clone())
        };

        let reduced = prefers_reduced_motion(cx);
        let wanted_open = self.open;
        let default_open = self.default_open;
        state.update(cx, |s, _| {
            let first = !s.started;
            s.started = true;
            match wanted_open {
                Some(open) => {
                    s.apply_open(open, reduced || first, &options);
                }
                None if first && default_open => {
                    s.apply_open(true, true, &options);
                }
                None => {}
            }
        });
        let now = Instant::now();
        let (value, open, highlighted, fade, turn, animating, trigger_focus, list_focus) = {
            let s = state.read(cx);
            (
                s.value.clone(),
                s.open,
                s.highlighted,
                s.fade.value(now),
                s.chevron.value(now),
                s.fade.is_animating(now) || s.chevron.is_animating(now),
                s.trigger_focus.clone(),
                s.list_focus.clone(),
            )
        };
        if animating {
            window.request_animation_frame();
        }
        let (trigger_bounds, popup_width) = {
            let s = state.read(cx);
            (s.trigger_bounds.clone(), s.popup_width.clone())
        };

        // The trigger's text, and its flicker when it changes.
        let placeholder = value.is_none() && trigger.text.is_none();
        let shown: SharedString = trigger
            .text
            .clone()
            .or_else(|| value.as_ref().map(label_for))
            .or_else(|| trigger.placeholder.clone())
            .unwrap_or_default();
        let ms = now_ms();
        let (flicker, flicker_animating) = state.update(cx, |s, _| {
            s.flicker.update(&shown, ms);
            s.flicker.opacity(ms, reduced)
        });
        if flicker_animating {
            window.request_animation_frame();
        }

        let p = ctx.palette.clone();
        let disabled = self.disabled;
        let text_color = if disabled || placeholder {
            p.ink_dim
        } else {
            p.lit
        }
        .opacity(flicker);
        let focus_visible =
            !disabled && trigger_focus.is_focused(window) && window.last_input_was_keyboard();

        let set_open = {
            let state = state.clone();
            let options = options.clone();
            let controlled_open = self.open.is_some();
            let on_open_change = self.on_open_change.clone();
            move |open: bool, window: &mut Window, cx: &mut App| {
                if state.read(cx).open == open {
                    return;
                }
                if !controlled_open {
                    state.update(cx, |s, cx| {
                        s.apply_open(open, reduced, &options);
                        cx.notify();
                    });
                }
                if let Some(handler) = on_open_change.as_ref() {
                    handler(open, window, cx);
                }
                let (trigger_focus, list_focus) = {
                    let s = state.read(cx);
                    (s.trigger_focus.clone(), s.list_focus.clone())
                };
                if open {
                    window.focus(&list_focus, cx);
                } else {
                    window.focus(&trigger_focus, cx);
                }
            }
        };
        let set_open = Rc::new(set_open);

        let controlled = self.value.is_some();
        let handler = self.on_value_change.clone();
        let select = {
            let state = state.clone();
            let set_open = set_open.clone();
            move |value: SharedString, window: &mut Window, cx: &mut App| {
                let changed = state.read(cx).value.as_ref() != Some(&value);
                if !controlled {
                    state.update(cx, |s, cx| {
                        s.value = Some(value.clone());
                        cx.notify();
                    });
                }
                set_open(false, window, cx);
                if changed && let Some(handler) = handler.as_ref() {
                    handler(value, window, cx);
                }
            }
        };
        let select = Rc::new(select);

        let aria_label = trigger.aria_label.clone().or(row_label);
        let glow = p.glow;
        let lit_for_ring = p.lit;
        let mut trigger_el = div()
            .id("trigger")
            .relative()
            .flex()
            .items_center()
            .gap(px(12.0))
            .rounded(radius_sm(cx.theme()))
            .mx(px(-4.0))
            .px(px(4.0))
            .py(px(2.0))
            .role(gpui::Role::ComboBox)
            .aria_expanded(open)
            .when_some(aria_label, |el, label| el.aria_label(label))
            .when_some(value.clone(), |el, value| el.aria_value(label_for(&value)))
            .child(LcdText::new(shown.clone(), &ctx, text_color))
            .child({
                let chevron_color = text_color;
                let grid = ctx.grid.clone();
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        paint_chevron(bounds, turn, chevron_color, glow, window);
                        grid.paint_holes(bounds, window);
                    },
                )
                .w(px(10.0))
                .h(px(6.0))
                .flex_shrink_0()
            })
            .child({
                let trigger_bounds = trigger_bounds.clone();
                canvas(
                    move |bounds, _, _| trigger_bounds.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            })
            .when(focus_visible, |el| {
                el.child(ring_overlay(radius_sm(cx.theme()), 1.0, lit_for_ring))
            });
        trigger_el.style().refine(&trigger.style);
        let trigger_el = if disabled {
            trigger_el
        } else {
            let toggle = set_open.clone();
            let keys = set_open.clone();
            let state_for_click = state.clone();
            trigger_el
                .track_focus(&trigger_focus)
                .on_click(move |_, window, cx| {
                    let open = state_for_click.read(cx).open;
                    toggle(!open, window, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "enter" | "space" | "down" | "up" => {
                            keys(true, window, cx);
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                })
        };

        let show_popup = open || fade > 0.0;
        if show_popup && trigger_bounds.get().is_none() {
            // Opened before the trigger was laid out: place it next frame.
            window.request_animation_frame();
        }
        let popup = match (show_popup, trigger_bounds.get()) {
            (true, Some(anchor_bounds)) => Some(render_popup(
                &ctx,
                content,
                options.clone(),
                PopupProps {
                    anchor_bounds,
                    popup_width,
                    value: value.clone(),
                    highlighted,
                    open,
                    fade,
                    list_focus,
                    state: state.clone(),
                    set_open: set_open.clone(),
                    select,
                },
                cx,
            )),
            _ => None,
        };

        div()
            .flex()
            .child(trigger_el)
            .when_some(popup, |el, popup| el.child(popup))
    }
}

struct PopupProps {
    anchor_bounds: Bounds<Pixels>,
    popup_width: Rc<Cell<Option<Pixels>>>,
    value: Option<SharedString>,
    highlighted: Option<usize>,
    open: bool,
    fade: f32,
    list_focus: FocusHandle,
    state: gpui::Entity<SelectState>,
    set_open: OpenHandler,
    select: ValueHandler,
}

fn render_popup(
    display: &LcdDisplayContext,
    content: LcdDisplaySelectContent,
    options: Rc<Vec<LcdDisplaySelectOption>>,
    props: PopupProps,
    cx: &App,
) -> AnyElement {
    let ctx = display.with_own_grid();
    let p = ctx.palette.clone();
    let theme = cx.theme();
    let (position, corner) = popup_anchor(
        props.anchor_bounds,
        content.side,
        content.align,
        content.side_offset,
        content.align_offset,
        props.popup_width.get(),
    );

    let mut list = div().flex().flex_col();
    for (index, option) in options.iter().enumerate() {
        let selected = props.value.as_ref() == Some(&option.value);
        let highlighted = props.highlighted == Some(index);
        let color = if highlighted {
            p.glass
        } else if selected {
            p.lit
        } else {
            p.ink_dim
        };
        let bg = highlighted.then_some(p.lit);
        let radius = radius_sm(theme);
        let glow = p.glow;
        let grid = ctx.grid.clone();
        let hover_state = props.state.clone();
        let select = props.select.clone();
        let value = option.value.clone();
        let item = div()
            .id(("item", index))
            .relative()
            .flex()
            .items_center()
            .gap(px(8.0))
            .rounded(radius)
            .px(px(8.0))
            .py(px(8.0))
            .role(gpui::Role::ListBoxOption)
            .aria_selected(selected)
            .aria_label(option.label.clone())
            .when(option.disabled, |el| el.opacity(0.4))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if let Some(bg) = bg {
                            paint_lit_quad(bounds, radius, bg, Some(glow), window);
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(
                // Keeps its width when hidden, so the choices stay aligned.
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if selected {
                            paint_lit_quad(bounds, px(0.0), color, Some(glow), window);
                        }
                    },
                )
                .size(px(6.0))
                .flex_shrink_0(),
            )
            .child(LcdText::new(option.label.clone(), &ctx, color).holes(None))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| grid.paint_holes(bounds, window),
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let item = if option.disabled {
            item
        } else {
            item.on_mouse_move(move |_, _, cx| {
                hover_state.update(cx, |s, cx| {
                    if s.highlighted != Some(index) {
                        s.highlighted = Some(index);
                        cx.notify();
                    }
                });
            })
            .on_click(move |_, window, cx| select(value.clone(), window, cx))
        };
        list = list.child(item);
    }

    let content_div = div().flex().flex_col().p(px(4.0)).child(list);
    let screen = lcd_screen(&ctx, content_div, cx);

    let mut shadows = vec![ring(p.bezel_ring)];
    // shadow-xl
    shadows.push(css_shadow(
        0.0,
        20.0,
        25.0,
        -5.0,
        gpui::black().opacity(0.1),
        false,
    ));
    shadows.push(css_shadow(
        0.0,
        8.0,
        10.0,
        -6.0,
        gpui::black().opacity(0.1),
        false,
    ));

    let keys_state = props.state.clone();
    let keys_options = options.clone();
    let keys_select = props.select.clone();
    let keys_open = props.set_open.clone();
    let out_open = props.set_open.clone();
    let anchor = props.anchor_bounds;
    let popup_width = props.popup_width.clone();
    let text_size = ctx.text_size();
    let mut popup = div()
        .id("lcd-display-select-content")
        .track_focus(&props.list_focus)
        .role(gpui::Role::ListBox)
        .min_w(props.anchor_bounds.size.width)
        .rounded(radius_md(theme))
        .bg(p.bezel_edge)
        .p(px(4.0))
        .font_family(p.font.clone())
        .text_size(text_size)
        .line_height(text_size)
        .shadow(shadows)
        .opacity(props.fade)
        .child(screen)
        .child(
            canvas(
                move |bounds, _, _| popup_width.set(Some(bounds.size.width)),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    popup.style().refine(&content.style);
    let popup = if props.open {
        popup
            .on_mouse_down_out(move |event, window, cx| {
                // The trigger toggles itself.
                if !anchor.contains(&event.position) {
                    out_open(false, window, cx);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let disabled: Vec<bool> = keys_options.iter().map(|o| o.disabled).collect();
                let movement = match event.keystroke.key.as_str() {
                    "down" => Some(ListMove::Next),
                    "up" => Some(ListMove::Previous),
                    "home" | "pageup" => Some(ListMove::First),
                    "end" | "pagedown" => Some(ListMove::Last),
                    _ => None,
                };
                cx.stop_propagation();
                if let Some(movement) = movement {
                    keys_state.update(cx, |s, cx| {
                        s.highlighted = move_highlight(&disabled, s.highlighted, movement);
                        cx.notify();
                    });
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" | "tab" => keys_open(false, window, cx),
                    "enter" | "space" => {
                        let highlighted = keys_state.read(cx).highlighted;
                        if let Some(option) = highlighted.and_then(|i| keys_options.get(i))
                            && !option.disabled
                        {
                            keys_select(option.value.clone(), window, cx);
                        }
                    }
                    _ => {
                        let Some(typed) = event.keystroke.key_char.clone() else {
                            return;
                        };
                        let labels: Vec<SharedString> =
                            keys_options.iter().map(|o| o.label.clone()).collect();
                        let now = now_ms();
                        keys_state.update(cx, |s, cx| {
                            if now - s.query_at_ms > TYPEAHEAD_RESET_MS {
                                s.query.clear();
                            }
                            s.query_at_ms = now;
                            // Repeating one letter cycles through its matches.
                            if !(s.query.chars().count() == 1 && s.query == typed) {
                                s.query.push_str(&typed);
                            }
                            if let Some(index) =
                                typeahead_match(&labels, &disabled, &s.query, s.highlighted)
                            {
                                s.highlighted = Some(index);
                                cx.notify();
                            }
                        });
                    }
                }
            })
    } else {
        popup
    };

    deferred(
        anchored()
            .position(position)
            .anchor(corner)
            .snap_to_window_with_margin(px(8.0))
            .child(popup),
    )
    .with_priority(1)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn lcd_display_sets_the_ink_from_the_variant() {
        let theme = Theme::light();
        let green = resolve_lcd_display_palette(
            LcdDisplayVariant::Green,
            LcdDisplayAppearance::Auto,
            &LcdDisplayVars::default(),
            &theme,
        );
        assert_eq!(green.ink, oklch(0.85, 0.19, 142.0));
        // The theme's primary colour by default.
        let primary = resolve_lcd_display_palette(
            LcdDisplayVariant::default(),
            LcdDisplayAppearance::default(),
            &LcdDisplayVars::default(),
            &theme,
        );
        assert_eq!(primary.ink, theme.primary);
    }

    #[test]
    fn lcd_display_sets_the_text_from_the_size() {
        assert_eq!(LcdDisplaySize::Sm.text_size(), px(14.0));
        assert_eq!(LcdDisplaySize::default().text_size(), px(18.0));
        assert_eq!(LcdDisplaySize::Lg.text_size(), px(24.0));
    }

    #[test]
    fn lcd_display_follows_the_theme_unless_an_appearance_is_pinned() {
        let light = Theme::light();
        let dark = Theme::dark();
        let vars = LcdDisplayVars::default();
        let ink = LcdDisplayVariant::Amber.ink(&light);
        // Backlit with dark pixels in the light theme...
        let auto_light = resolve_lcd_display_palette(
            LcdDisplayVariant::Amber,
            LcdDisplayAppearance::Auto,
            &vars,
            &light,
        );
        assert_eq!(auto_light.lit, mix(ink, gpui::black(), 0.62));
        // ...and the glowing set in the dark one.
        let auto_dark = resolve_lcd_display_palette(
            LcdDisplayVariant::Amber,
            LcdDisplayAppearance::Auto,
            &vars,
            &dark,
        );
        assert_eq!(auto_dark.lit, ink);
        let pinned_dark = resolve_lcd_display_palette(
            LcdDisplayVariant::Amber,
            LcdDisplayAppearance::Dark,
            &vars,
            &light,
        );
        assert_eq!(pinned_dark.lit, ink);
        assert_eq!(pinned_dark.glass, oklch(0.15, 0.004, 200.0));
        let pinned_light = resolve_lcd_display_palette(
            LcdDisplayVariant::Amber,
            LcdDisplayAppearance::Light,
            &vars,
            &dark,
        );
        assert_eq!(pinned_light.lit, auto_light.lit);
    }

    #[test]
    fn lcd_display_mixes_every_pixel_from_one_custom_ink() {
        let theme = Theme::dark();
        let vars = LcdDisplayVars {
            ink: Some(theme.chart_1),
            ..Default::default()
        };
        let p = resolve_lcd_display_palette(
            LcdDisplayVariant::Primary,
            LcdDisplayAppearance::Auto,
            &vars,
            &theme,
        );
        assert_eq!(p.lit, theme.chart_1);
        assert!(close(p.ink_dim.a, 0.4));
        assert!(close(p.ink_unlit.a, 0.07));
        assert!(close(p.glow.a, 0.6));
        assert_eq!(p.pitch, px(2.0));
        assert_eq!(p.font, theme.font_mono);
    }

    #[test]
    fn lcd_display_tints_the_light_glass_with_the_ink() {
        let theme = Theme::light();
        let p = resolve_lcd_display_palette(
            LcdDisplayVariant::Red,
            LcdDisplayAppearance::Light,
            &LcdDisplayVars::default(),
            &theme,
        );
        let (l, a, b, _) = to_oklab(p.glass);
        assert!((l - 0.93).abs() < 0.01);
        // The tint keeps the ink's hue, mapped into sRGB at that lightness.
        let chroma = (a * a + b * b).sqrt();
        assert!(chroma > 0.03 && chroma <= 0.2 * 0.35 + 0.001);
        assert!((b.atan2(a).to_degrees() - 27.0).abs() < 1.0);
    }

    #[test]
    fn lcd_display_drops_the_dots_at_1x_and_shows_them_at_2x() {
        // At 1x every device pixel sits half inside the unlit dot: a wash.
        assert_eq!(unlit_dot_pattern(2.0, 1.0), DotPattern::Uniform(0.5));
        assert_eq!(mask_hole_size(2.0, 1.0), 0.0);
        // At 2x, a 1px dot in the middle of each 2px cell, and 1px holes.
        assert_eq!(
            unlit_dot_pattern(2.0, 2.0),
            DotPattern::Square {
                offset: 0.5,
                size: 1.0
            }
        );
        assert_eq!(mask_hole_size(2.0, 2.0), 0.5);
    }

    #[test]
    fn lcd_display_separator_dots_every_other_cell() {
        assert_eq!(separator_dot(2.0, 1.0), (1.0, 2.0));
        assert_eq!(separator_dot(2.0, 2.0), (1.0, 2.0));
    }

    #[test]
    fn lcd_display_flickers_a_value_when_its_text_changes() {
        let mut flicker = Flicker::default();
        flicker.update(&"Low".into(), 0.0);
        assert!(!flicker.changed(0.0));
        assert_eq!(flicker.opacity(0.0, false), (1.0, false));
        flicker.update(&"High".into(), 10.0);
        assert!(flicker.changed(10.0));
        assert_eq!(flicker.opacity(10.0, false).0, FLICKER_OPACITY);
        // Gone after the two frames, and faded back after the transition.
        assert!(!flicker.changed(110.0));
        assert_eq!(flicker.opacity(400.0, false), (1.0, false));
        // The same text again does nothing.
        flicker.update(&"High".into(), 500.0);
        assert!(!flicker.changed(500.0));
        // Reduced motion keeps it at full opacity.
        flicker.update(&"Low".into(), 600.0);
        assert_eq!(flicker.opacity(600.0, true), (1.0, false));
    }

    #[test]
    fn lcd_display_blinks_the_status_pixel() {
        assert!(close(pulse_opacity(0.0, 2000.0), 1.0));
        assert!(close(pulse_opacity(1000.0, 2000.0), 0.5));
        assert!(close(pulse_opacity(300.0, 600.0), 0.5));
        assert!(close(pulse_opacity(2000.0, 2000.0), 1.0));
    }

    #[test]
    fn lcd_display_select_moves_the_highlight_over_enabled_choices() {
        let disabled = [false, true, false, false];
        assert_eq!(move_highlight(&disabled, None, ListMove::Next), Some(0));
        assert_eq!(move_highlight(&disabled, Some(0), ListMove::Next), Some(2));
        assert_eq!(move_highlight(&disabled, Some(3), ListMove::Next), Some(3));
        assert_eq!(
            move_highlight(&disabled, Some(2), ListMove::Previous),
            Some(0)
        );
        assert_eq!(move_highlight(&disabled, Some(0), ListMove::Last), Some(3));
        assert_eq!(move_highlight(&disabled, Some(3), ListMove::First), Some(0));
    }

    #[test]
    fn lcd_display_select_finds_a_choice_by_typing() {
        let labels: Vec<SharedString> =
            vec!["Low".into(), "Med".into(), "High".into(), "Ultra".into()];
        let disabled = [false; 4];
        assert_eq!(typeahead_match(&labels, &disabled, "h", None), Some(2));
        assert_eq!(typeahead_match(&labels, &disabled, "M", Some(0)), Some(1));
        assert_eq!(typeahead_match(&labels, &disabled, "x", None), None);
    }

    #[test]
    fn lcd_display_select_opens_below_the_trigger_aligned_to_its_end() {
        let trigger = Bounds::new(point(px(100.0), px(50.0)), size(px(80.0), px(22.0)));
        let (at, corner) = popup_anchor(
            trigger,
            LcdDisplaySelectSide::default(),
            LcdDisplaySelectAlign::default(),
            8.0,
            0.0,
            None,
        );
        assert_eq!(at, point(px(180.0), px(80.0)));
        assert_eq!(corner, gpui::Anchor::TopRight);
    }
}
