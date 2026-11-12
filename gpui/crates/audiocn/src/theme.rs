//! Theming: the GPUI counterpart of the CSS variables audiocn is styled with.
//!
//! The React components use shadcn/ui's tokens (`--background`, `--primary`,
//! `--muted-foreground`, `--border`, `--destructive`, ...) plus audiocn's own
//! (`--meter-ok`, `--channel-mute`, ...). [`Theme`] holds every one of them
//! under the same name in snake_case, as a GPUI global. Components read it with
//! [`ActiveTheme::theme`]:
//!
//! ```ignore
//! use audiocn::theme::ActiveTheme;
//! div().bg(cx.theme().muted).text_color(cx.theme().muted_foreground)
//! ```
//!
//! [`Theme::light`] / [`Theme::dark`] are the exact values of audiocn's
//! `app/globals.css` (shadcn "stone"). [`Theme::apply_css`] reads any shadcn
//! theme's CSS (`--name: oklch(...)` declarations), so any shadcn theme
//! restyles audiocn here too.
//!
//! Without a global, components fall back to [`Theme::light`].

use gpui::{App, Global, Hsla, Pixels, Rgba, px};

/// Converts a CSS `oklch(l c h)` colour to GPUI's [`Hsla`].
/// `l` is 0..1, `c` is chroma, `h` is degrees.
pub fn oklch(l: f32, c: f32, h: f32) -> Hsla {
    oklcha(l, c, h, 1.0)
}

/// [`oklch`] with alpha (`oklch(l c h / alpha)`).
pub fn oklcha(l: f32, c: f32, h: f32, alpha: f32) -> Hsla {
    let (r, g, b) = oklch_to_srgb(l, c, h);
    Rgba { r, g, b, a: alpha }.into()
}

fn oklch_to_srgb(l: f32, c: f32, h: f32) -> (f32, f32, f32) {
    let h = h.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();

    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;

    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;

    (encode_srgb(r), encode_srgb(g), encode_srgb(bl))
}

fn encode_srgb(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn decode_srgb(x: f32) -> f32 {
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a colour to OKLab `(l, a, b)` and alpha.
pub fn to_oklab(color: Hsla) -> (f32, f32, f32, f32) {
    let rgba = color.to_rgb();
    let (r, g, b) = (
        decode_srgb(rgba.r),
        decode_srgb(rgba.g),
        decode_srgb(rgba.b),
    );
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        rgba.a,
    )
}

fn from_oklab(l: f32, a: f32, b: f32, alpha: f32) -> Hsla {
    let c = (a * a + b * b).sqrt();
    let h = b.atan2(a).to_degrees();
    oklcha(l, c, h, alpha)
}

/// CSS `color-mix(in oklab, a, b weight_b)`: `weight_b` is b's share (0..1).
/// Alpha is premultiplied like the CSS spec, so mixing with `transparent`
/// fades the colour instead of greying it.
pub fn mix(a: Hsla, b: Hsla, weight_b: f32) -> Hsla {
    let t = weight_b.clamp(0.0, 1.0);
    let (l1, a1, b1, al1) = to_oklab(a);
    let (l2, a2, b2, al2) = to_oklab(b);
    let alpha = al1 * (1.0 - t) + al2 * t;
    if alpha <= 0.0 {
        return gpui::transparent_black();
    }
    let w1 = al1 * (1.0 - t) / alpha;
    let w2 = al2 * t / alpha;
    from_oklab(
        l1 * w1 + l2 * w2,
        a1 * w1 + a2 * w2,
        b1 * w1 + b2 * w2,
        alpha,
    )
}

/// CSS `color-mix(in oklab, color pct%, transparent)`: the colour at `pct` opacity.
pub fn fade(color: Hsla, opacity: f32) -> Hsla {
    color.opacity(opacity)
}

/// `color-mix(in oklch, color, black amount)`.
pub fn mix_black(color: Hsla, amount: f32) -> Hsla {
    mix(color, gpui::black(), amount)
}

/// Every CSS variable audiocn's components read, named after the variable.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub is_dark: bool,

    // shadcn/ui
    pub background: Hsla,
    pub foreground: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub chart_1: Hsla,
    pub chart_2: Hsla,
    pub chart_3: Hsla,
    pub chart_4: Hsla,
    pub chart_5: Hsla,
    pub sidebar: Hsla,
    pub sidebar_foreground: Hsla,
    pub sidebar_primary: Hsla,
    pub sidebar_primary_foreground: Hsla,
    pub sidebar_accent: Hsla,
    pub sidebar_accent_foreground: Hsla,
    pub sidebar_border: Hsla,
    pub sidebar_ring: Hsla,
    /// `--radius` (0.625rem = 10px). Tailwind's `rounded-sm/md/lg/xl` derive from it.
    pub radius: Pixels,

    // audiocn
    pub meter_ok: Hsla,
    pub meter_warn: Hsla,
    pub meter_clip: Hsla,
    pub meter_ok_foreground: Hsla,
    pub meter_warn_foreground: Hsla,
    pub meter_clip_foreground: Hsla,
    pub channel_mute: Hsla,
    pub channel_solo: Hsla,
    pub channel_monitor: Hsla,
    pub channel_mute_foreground: Hsla,
    pub channel_solo_foreground: Hsla,
    pub channel_monitor_foreground: Hsla,

    /// `--font-sans` family name (DM Sans on audiocn.dev).
    pub font_sans: gpui::SharedString,
    /// `--font-mono` family name (Geist Mono on audiocn.dev).
    pub font_mono: gpui::SharedString,
    /// `--font-doto`, the dot-matrix face the LCD display uses (Doto on audiocn.dev).
    pub font_doto: gpui::SharedString,
}

impl Global for Theme {}

impl Theme {
    /// `:root` in `app/globals.css`.
    pub fn light() -> Self {
        let destructive = oklch(0.577, 0.245, 27.325);
        let meter_ok = oklch(0.696, 0.17, 162.48);
        let meter_warn = oklch(0.795, 0.184, 86.047);
        let channel_solo = oklch(0.795, 0.184, 86.047);
        let channel_monitor = oklch(0.623, 0.214, 259.815);
        Self {
            is_dark: false,
            background: oklch(1.0, 0.0, 0.0),
            foreground: oklch(0.147, 0.004, 49.25),
            card: oklch(1.0, 0.0, 0.0),
            card_foreground: oklch(0.147, 0.004, 49.25),
            popover: oklch(1.0, 0.0, 0.0),
            popover_foreground: oklch(0.147, 0.004, 49.25),
            primary: oklch(0.216, 0.006, 56.043),
            primary_foreground: oklch(0.985, 0.001, 106.423),
            secondary: oklch(0.97, 0.001, 106.424),
            secondary_foreground: oklch(0.216, 0.006, 56.043),
            muted: oklch(0.97, 0.001, 106.424),
            muted_foreground: oklch(0.5, 0.013, 58.071),
            accent: oklch(0.97, 0.001, 106.424),
            accent_foreground: oklch(0.216, 0.006, 56.043),
            destructive,
            border: oklch(0.923, 0.003, 48.717),
            input: oklch(0.923, 0.003, 48.717),
            ring: oklch(0.709, 0.01, 56.259),
            chart_1: oklch(0.808, 0.114, 19.571),
            chart_2: oklch(0.637, 0.237, 25.331),
            chart_3: oklch(0.577, 0.245, 27.325),
            chart_4: oklch(0.505, 0.213, 27.518),
            chart_5: oklch(0.444, 0.177, 26.899),
            sidebar: oklch(0.985, 0.001, 106.423),
            sidebar_foreground: oklch(0.147, 0.004, 49.25),
            sidebar_primary: oklch(0.216, 0.006, 56.043),
            sidebar_primary_foreground: oklch(0.985, 0.001, 106.423),
            sidebar_accent: oklch(0.97, 0.001, 106.424),
            sidebar_accent_foreground: oklch(0.216, 0.006, 56.043),
            sidebar_border: oklch(0.923, 0.003, 48.717),
            sidebar_ring: oklch(0.709, 0.01, 56.259),
            radius: px(10.0),
            meter_ok,
            meter_warn,
            meter_clip: destructive,
            meter_ok_foreground: mix_black(meter_ok, 0.4),
            meter_warn_foreground: mix_black(meter_warn, 0.4),
            meter_clip_foreground: mix_black(destructive, 0.4),
            channel_mute: destructive,
            channel_solo,
            channel_monitor,
            channel_mute_foreground: mix_black(destructive, 0.4),
            channel_solo_foreground: mix_black(channel_solo, 0.4),
            channel_monitor_foreground: mix_black(channel_monitor, 0.4),
            font_sans: "DM Sans".into(),
            font_mono: "Geist Mono".into(),
            font_doto: "Doto".into(),
        }
    }

    /// `.dark` in `app/globals.css`.
    pub fn dark() -> Self {
        let destructive = oklch(0.704, 0.191, 22.216);
        let meter_ok = oklch(0.765, 0.177, 163.223);
        let meter_warn = oklch(0.852, 0.199, 91.936);
        let channel_solo = oklch(0.852, 0.199, 91.936);
        let channel_monitor = oklch(0.707, 0.165, 254.624);
        Self {
            is_dark: true,
            background: oklch(0.147, 0.004, 49.25),
            foreground: oklch(0.985, 0.001, 106.423),
            card: oklch(0.216, 0.006, 56.043),
            card_foreground: oklch(0.985, 0.001, 106.423),
            popover: oklch(0.216, 0.006, 56.043),
            popover_foreground: oklch(0.985, 0.001, 106.423),
            primary: oklch(0.923, 0.003, 48.717),
            primary_foreground: oklch(0.216, 0.006, 56.043),
            secondary: oklch(0.268, 0.007, 34.298),
            secondary_foreground: oklch(0.985, 0.001, 106.423),
            muted: oklch(0.268, 0.007, 34.298),
            muted_foreground: oklch(0.709, 0.01, 56.259),
            accent: oklch(0.268, 0.007, 34.298),
            accent_foreground: oklch(0.985, 0.001, 106.423),
            destructive,
            border: oklcha(1.0, 0.0, 0.0, 0.10),
            input: oklcha(1.0, 0.0, 0.0, 0.15),
            ring: oklch(0.553, 0.013, 58.071),
            chart_1: oklch(0.808, 0.114, 19.571),
            chart_2: oklch(0.637, 0.237, 25.331),
            chart_3: oklch(0.577, 0.245, 27.325),
            chart_4: oklch(0.505, 0.213, 27.518),
            chart_5: oklch(0.444, 0.177, 26.899),
            sidebar: oklch(0.216, 0.006, 56.043),
            sidebar_foreground: oklch(0.985, 0.001, 106.423),
            sidebar_primary: oklch(0.488, 0.243, 264.376),
            sidebar_primary_foreground: oklch(0.985, 0.001, 106.423),
            sidebar_accent: oklch(0.268, 0.007, 34.298),
            sidebar_accent_foreground: oklch(0.985, 0.001, 106.423),
            sidebar_border: oklcha(1.0, 0.0, 0.0, 0.10),
            sidebar_ring: oklch(0.553, 0.013, 58.071),
            radius: px(10.0),
            meter_ok,
            meter_warn,
            meter_clip: destructive,
            meter_ok_foreground: meter_ok,
            meter_warn_foreground: meter_warn,
            meter_clip_foreground: destructive,
            channel_mute: destructive,
            channel_solo,
            channel_monitor,
            channel_mute_foreground: destructive,
            channel_solo_foreground: channel_solo,
            channel_monitor_foreground: channel_monitor,
            font_sans: "DM Sans".into(),
            font_mono: "Geist Mono".into(),
            font_doto: "Doto".into(),
        }
    }

    /// Tailwind `rounded-sm` (`--radius - 4px`).
    pub fn radius_sm(&self) -> Pixels {
        self.radius - px(4.0)
    }
    /// Tailwind `rounded-md` (`--radius - 2px`).
    pub fn radius_md(&self) -> Pixels {
        self.radius - px(2.0)
    }
    /// Tailwind `rounded-lg` (`--radius`).
    pub fn radius_lg(&self) -> Pixels {
        self.radius
    }
    /// Tailwind `rounded-xl` (`--radius + 4px`).
    pub fn radius_xl(&self) -> Pixels {
        self.radius + px(4.0)
    }

    /// Applies a shadcn theme's CSS: every `--name: value;` declaration whose
    /// name matches a field (e.g. `--muted-foreground`) and whose value is
    /// `oklch(...)`, `#rrggbb` or `var(--other)` overrides that field. Pass the
    /// contents of one block (`:root { ... }` or `.dark { ... }`).
    /// `--meter-clip` / `--channel-mute` default to `--destructive`; if the CSS
    /// changes `--destructive` but not those, they follow it.
    pub fn apply_css(&mut self, css: &str) {
        let mut set_clip = false;
        let mut set_mute = false;
        for decl in css.split(';') {
            // Drop any selector before the declaration (`:root { --x: ...`).
            let decl = decl.rsplit('{').next().unwrap_or(decl);
            let Some((name, value)) = decl.split_once(':') else {
                continue;
            };
            let Some(name) = name
                .trim()
                .rsplit("--")
                .next()
                .filter(|_| name.contains("--"))
            else {
                continue;
            };
            let value = value.trim().trim_end_matches('}').trim();
            if name == "radius" {
                if let Some(rem) = value
                    .strip_suffix("rem")
                    .and_then(|v| v.parse::<f32>().ok())
                {
                    self.radius = px(rem * 16.0);
                } else if let Some(p) = value.strip_suffix("px").and_then(|v| v.parse::<f32>().ok())
                {
                    self.radius = px(p);
                }
                continue;
            }
            let color = match self.resolve_css_color(value) {
                Some(color) => color,
                None => continue,
            };
            set_clip |= name == "meter-clip";
            set_mute |= name == "channel-mute";
            if let Some(slot) = self.slot_mut(name) {
                *slot = color;
            }
        }
        if !set_clip {
            self.meter_clip = self.destructive;
        }
        if !set_mute {
            self.channel_mute = self.destructive;
        }
    }

    fn resolve_css_color(&mut self, value: &str) -> Option<Hsla> {
        if let Some(var) = value
            .strip_prefix("var(--")
            .and_then(|v| v.strip_suffix(')'))
        {
            return self.slot_mut(var).map(|slot| *slot);
        }
        parse_css_color(value)
    }

    fn slot_mut(&mut self, name: &str) -> Option<&mut Hsla> {
        Some(match name {
            "background" => &mut self.background,
            "foreground" => &mut self.foreground,
            "card" => &mut self.card,
            "card-foreground" => &mut self.card_foreground,
            "popover" => &mut self.popover,
            "popover-foreground" => &mut self.popover_foreground,
            "primary" => &mut self.primary,
            "primary-foreground" => &mut self.primary_foreground,
            "secondary" => &mut self.secondary,
            "secondary-foreground" => &mut self.secondary_foreground,
            "muted" => &mut self.muted,
            "muted-foreground" => &mut self.muted_foreground,
            "accent" => &mut self.accent,
            "accent-foreground" => &mut self.accent_foreground,
            "destructive" => &mut self.destructive,
            "border" => &mut self.border,
            "input" => &mut self.input,
            "ring" => &mut self.ring,
            "chart-1" => &mut self.chart_1,
            "chart-2" => &mut self.chart_2,
            "chart-3" => &mut self.chart_3,
            "chart-4" => &mut self.chart_4,
            "chart-5" => &mut self.chart_5,
            "sidebar" => &mut self.sidebar,
            "sidebar-foreground" => &mut self.sidebar_foreground,
            "sidebar-primary" => &mut self.sidebar_primary,
            "sidebar-primary-foreground" => &mut self.sidebar_primary_foreground,
            "sidebar-accent" => &mut self.sidebar_accent,
            "sidebar-accent-foreground" => &mut self.sidebar_accent_foreground,
            "sidebar-border" => &mut self.sidebar_border,
            "sidebar-ring" => &mut self.sidebar_ring,
            "meter-ok" => &mut self.meter_ok,
            "meter-warn" => &mut self.meter_warn,
            "meter-clip" => &mut self.meter_clip,
            "meter-ok-foreground" => &mut self.meter_ok_foreground,
            "meter-warn-foreground" => &mut self.meter_warn_foreground,
            "meter-clip-foreground" => &mut self.meter_clip_foreground,
            "channel-mute" => &mut self.channel_mute,
            "channel-solo" => &mut self.channel_solo,
            "channel-monitor" => &mut self.channel_monitor,
            "channel-mute-foreground" => &mut self.channel_mute_foreground,
            "channel-solo-foreground" => &mut self.channel_solo_foreground,
            "channel-monitor-foreground" => &mut self.channel_monitor_foreground,
            _ => return None,
        })
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

/// Parses `oklch(l c h)`, `oklch(l c h / a%)`, `#rgb`, `#rrggbb` and `#rrggbbaa`.
pub fn parse_css_color(value: &str) -> Option<Hsla> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("oklch(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let (channels, alpha) = match inner.split_once('/') {
            Some((c, a)) => (c, Some(a.trim())),
            None => (inner, None),
        };
        let mut parts = channels.split_whitespace();
        let l = parse_number(parts.next()?)?;
        let c = parse_number(parts.next()?)?;
        let h = parse_number(parts.next()?)?;
        let a = alpha.map_or(Some(1.0), parse_number)?;
        return Some(oklcha(l, c, h, a));
    }
    let hex = value.strip_prefix('#')?;
    let expand = |s: &str| -> Option<u32> { u32::from_str_radix(s, 16).ok() };
    let (r, g, b, a) = match hex.len() {
        3 => {
            let v: Vec<u32> = hex
                .chars()
                .map(|c| expand(&c.to_string()))
                .collect::<Option<_>>()?;
            (v[0] * 17, v[1] * 17, v[2] * 17, 255)
        }
        6 | 8 => (
            expand(&hex[0..2])?,
            expand(&hex[2..4])?,
            expand(&hex[4..6])?,
            if hex.len() == 8 {
                expand(&hex[6..8])?
            } else {
                255
            },
        ),
        _ => return None,
    };
    Some(
        Rgba {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
        .into(),
    )
}

fn parse_number(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        return pct.parse::<f32>().ok().map(|v| v / 100.0);
    }
    if s == "none" {
        return Some(0.0);
    }
    s.parse().ok()
}

/// Access to the audiocn theme, like `cx.theme()` in other GPUI libraries.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

static FALLBACK: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.try_global::<Theme>()
            .unwrap_or_else(|| FALLBACK.get_or_init(Theme::light))
    }
}

/// Sets the global theme (and refreshes every window).
pub fn set_theme(theme: Theme, cx: &mut App) {
    cx.set_global(theme);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn oklch_white_and_black() {
        let white = oklch(1.0, 0.0, 0.0).to_rgb();
        assert!(close(white.r, 1.0) && close(white.g, 1.0) && close(white.b, 1.0));
        let black = oklch(0.0, 0.0, 0.0).to_rgb();
        assert!(close(black.r, 0.0) && close(black.g, 0.0) && close(black.b, 0.0));
    }

    #[test]
    fn oklab_round_trip() {
        let c = oklch(0.696, 0.17, 162.48);
        let (l, a, b, alpha) = to_oklab(c);
        let back = from_oklab(l, a, b, alpha).to_rgb();
        let orig = c.to_rgb();
        assert!(close(back.r, orig.r) && close(back.g, orig.g) && close(back.b, orig.b));
    }

    #[test]
    fn apply_css_overrides_and_follows_destructive() {
        let mut theme = Theme::light();
        theme.apply_css(
            ":root { --primary: oklch(0.5 0.215 262.881); --destructive: #ff0000; --radius: 0.5rem; }",
        );
        assert_eq!(theme.primary, oklch(0.5, 0.215, 262.881));
        assert_eq!(theme.meter_clip, theme.destructive);
        assert_eq!(theme.radius, px(8.0));
    }

    #[test]
    fn parses_alpha() {
        let c = parse_css_color("oklch(1 0 0 / 10%)").unwrap();
        assert!(close(c.a, 0.1));
    }
}
