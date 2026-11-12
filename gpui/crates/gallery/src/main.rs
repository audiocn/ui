//! The audiocn GPUI gallery: every docs page and example, natively and in the browser.
//!
//! ```sh
//! cargo run -p audiocn-gallery                          # docs-style browser with a sidebar
//! cargo run -p audiocn-gallery -- --demo knob           # open on one page
//! cargo run -p audiocn-gallery -- --demo knob --variant metal   # just one example, centred
//! cargo run -p audiocn-gallery -- --theme dark
//! ```
//!
//! On the web (`scripts/build-wasm.sh`), `index.html?demo=<slug>&variant=<name>&theme=light|dark`
//! renders one example centred; without `variant` it renders that slug's whole docs page
//! (no sidebar); without `demo` it shows the sidebar browser.

#![cfg_attr(target_family = "wasm", no_main)]

mod demos;
mod playground;

use std::collections::HashSet;

use audiocn::theme::{ActiveTheme, Theme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, Context, FontWeight, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    point, px, relative, size,
};
use gpui::{AppContext, Entity, InteractiveElement, KeyBinding, actions};

actions!(gallery, [FocusNext, FocusPrev]);

mod fonts {
    include!(concat!(env!("OUT_DIR"), "/fonts.rs"));
}

struct Gallery {
    selected: usize,
    /// Set by `--variant`: render only that example, centred, without chrome.
    only_variant: Option<String>,
    /// Show the sidebar (off on the web when a `demo` is given).
    sidebar: bool,
    scroll: ScrollHandle,
    focus: gpui::FocusHandle,
    /// Set while the playground (props panel for every component) is shown.
    playground: Option<Entity<playground::Playground>>,
}

impl Gallery {
    /// Opens the playground on `slug` (or the current page's component).
    fn open_playground(&mut self, slug: Option<&str>, cx: &mut Context<Self>) {
        let slug = slug
            .map(str::to_string)
            .unwrap_or_else(|| demos::ALL[self.selected].slug.to_string());
        let gallery = cx.entity().downgrade();
        self.playground = Some(cx.new(|_| {
            playground::Playground::new(Some(&slug)).on_exit(move |_, cx| {
                gallery
                    .update(cx, |g, cx| {
                        g.playground = None;
                        cx.notify();
                    })
                    .ok();
            })
        }));
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let theme = if cx.theme().is_dark {
            Theme::light()
        } else {
            Theme::dark()
        };
        audiocn::theme::set_theme(theme, cx);
    }

    /// The fumadocs sidebar of audiocn.dev: `bg-sidebar border-e w-[268px]`,
    /// a `p-4 pb-2` header, then `text-sm` groups of `rounded-lg p-2` links.
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .pb_2()
            .child(
                div()
                    .text_size(px(15.))
                    .line_height(px(21.43))
                    .font_weight(FontWeight::MEDIUM)
                    .child("audiocn"),
            )
            .child(
                div()
                    .id("theme-toggle")
                    .px_2()
                    .py_1()
                    .rounded(theme.radius_lg())
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(theme.accent.opacity(0.5))
                            .text_color(theme.accent_foreground)
                    })
                    .child(if theme.is_dark { "Light" } else { "Dark" })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
            )
            .child(
                div()
                    .id("playground-toggle")
                    .px_2()
                    .py_1()
                    .rounded(theme.radius_lg())
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(theme.accent.opacity(0.5))
                            .text_color(theme.accent_foreground)
                    })
                    .child("Playground")
                    .on_click(cx.listener(|this, _, _, cx| this.open_playground(None, cx))),
            );

        let mut list = div()
            .id("sidebar-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_0p5()
            .px_4()
            .pt_4()
            .pb_4();
        let mut last = None;
        for (index, demo) in demos::ALL.iter().enumerate() {
            let group = demos::group(demo.slug);
            if last != Some(group) {
                // `mt-6 mb-1 px-2 first:mt-0` (the list's `gap-0.5` adds 2px).
                list = list.child(
                    div()
                        .when(last.is_some(), |d| d.mt(px(22.)))
                        .mb(px(2.))
                        .px_2()
                        .text_color(theme.foreground)
                        .child(group),
                );
                last = Some(group);
            }
            let active = index == self.selected;
            list = list.child(
                div()
                    .id(demo.slug)
                    .flex_none()
                    .p_2()
                    .rounded(theme.radius_lg())
                    .cursor_pointer()
                    .when(active, |d| {
                        d.bg(theme.primary.opacity(0.1)).text_color(theme.primary)
                    })
                    .when(!active, |d| {
                        d.text_color(theme.muted_foreground).hover(|s| {
                            s.bg(theme.accent.opacity(0.5))
                                .text_color(theme.accent_foreground)
                        })
                    })
                    .child(docs::title(demo.slug).unwrap_or(demo.title))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx))),
            );
        }

        div()
            .flex()
            .flex_col()
            .w(px(268.))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .text_color(theme.foreground)
            .text_size(px(14.))
            .line_height(relative(20. / 14.))
            .child(header)
            .child(list)
    }

    /// One docs page, laid out like audiocn.dev's fumadocs article
    /// (`max-w-[900px] px-6 pt-8 gap-4`): the title, the description, then
    /// every example under its docs heading in a preview box.
    fn page(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = &demos::ALL[self.selected];
        let theme = cx.theme().clone();
        let mut page = div()
            .flex()
            .flex_col()
            .px_6()
            .pt_8()
            .pb_6()
            .max_w(px(900.))
            .w_full()
            // `text-[1.75em] font-semibold`
            .child(
                div()
                    .text_size(px(28.))
                    .line_height(px(42.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(docs::title(demo.slug).unwrap_or(demo.title)),
            )
            // `mb-8 text-lg text-fd-muted-foreground`, after the article's `gap-4`.
            .child(
                div()
                    .mt_4()
                    .text_lg()
                    .line_height(px(28.))
                    .text_color(theme.muted_foreground)
                    .child(docs::description(demo.slug).unwrap_or(demo.description)),
            );
        let padding = preview_padding(demo.slug);
        for (index, variant) in demo.variants.iter().enumerate() {
            let element = (demo.render)(variant, window, cx);
            let heading = docs::heading(demo.slug, variant);
            // The hero sits `mb-8` + `gap-4` under the description; later
            // examples follow their heading (`mt-8 mb-3`, preview `my-6`).
            let top = if index == 0 { px(48.) } else { px(24.) };
            page = page.when_some(heading.filter(|_| index > 0), |page, heading| {
                page.child(
                    div()
                        .mt(px(56.))
                        .text_xl()
                        .line_height(px(32.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(heading),
                )
            });
            page = page.child(
                // `bg-background flex min-h-72 w-full justify-center
                // items-center rounded-xl border p-4 sm:p-10`
                div()
                    .mt(top)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .min_h(px(288.))
                    .p(padding)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius_xl())
                    .bg(theme.background)
                    // Inherited from the docs `.prose` wrapper: `--tw-prose-body`
                    // is `foreground` at 90%.
                    .text_color(theme.foreground.opacity(0.9))
                    .text_size(px(14.))
                    .line_height(relative(20. / 14.))
                    .children(element),
            );
        }
        page
    }
}

fn variant_title(variant: &str) -> String {
    // Acronyms the docs headings keep upper-case ("CSS level").
    let words = variant
        .split('-')
        .map(|word| match word {
            "css" | "rms" | "lcd" | "db" => word.to_uppercase(),
            _ => word.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `p-10`, or the `className` a docs page gives its previews
/// (`system-audio-mixer`: `p-4 sm:p-6`).
fn preview_padding(slug: &str) -> gpui::Pixels {
    match slug {
        "system-audio-mixer" => px(24.),
        _ => px(40.),
    }
}

/// Example headings, read from the docs pages (`content/docs/**/<slug>.mdx`).
mod docs {
    macro_rules! pages {
        ($($dir:literal / $slug:literal),* $(,)?) => {
            &[$(($slug, include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"), "/../../../content/docs/", $dir, "/", $slug, ".mdx"
            )))),*]
        };
    }

    const PAGES: &[(&str, &str)] = pages![
        "components" / "audio-device-select",
        "components" / "audio-player",
        "components" / "bar-visualizer",
        "components" / "channel-strip",
        "components" / "channel-toggle",
        "components" / "clip-indicator",
        "components" / "db-readout",
        "components" / "db-scale",
        "components" / "electric-bar-visualizer",
        "components" / "electric-waveform",
        "components" / "fader",
        "components" / "knob",
        "components" / "lcd-display",
        "components" / "lcd-meter",
        "components" / "level-meter",
        "components" / "live-waveform",
        "components" / "mixer",
        "components" / "pan-control",
        "components" / "parameter-slider",
        "components" / "smooth-waveform",
        "components" / "sound-pad",
        "components" / "spectrum",
        "components" / "tempo-control",
        "components" / "track-list",
        "components" / "volume-control",
        "components" / "waveform",
        "blocks" / "mic-setup",
        "blocks" / "music-player",
        "blocks" / "quick-audio-popover",
        "blocks" / "soundboard",
        "blocks" / "system-audio-mixer",
        "blocks" / "system-audio-settings",
    ];

    fn frontmatter(slug: &str, key: &str) -> Option<&'static str> {
        let (_, page) = PAGES.iter().find(|(s, _)| *s == slug)?;
        let body = page.strip_prefix("---")?;
        let end = body.find("\n---")?;
        body[..end].lines().find_map(|line| {
            let value = line.strip_prefix(key)?.strip_prefix(':')?.trim();
            Some(value.trim_matches('"'))
        })
    }

    /// The docs page title (`title:` in the front matter).
    pub fn title(slug: &str) -> Option<&'static str> {
        frontmatter(slug, "title")
    }

    /// The docs page description (`description:` in the front matter).
    pub fn description(slug: &str) -> Option<&'static str> {
        frontmatter(slug, "description")
    }

    /// The heading above `<ComponentPreview name="<slug>-<variant>">` on the
    /// docs page (`default` is `<slug>-demo`).
    pub fn heading(slug: &str, variant: &str) -> Option<String> {
        let (_, page) = PAGES.iter().find(|(s, _)| *s == slug)?;
        let example = if variant == "default" {
            format!("{slug}-demo")
        } else {
            format!("{slug}-{variant}")
        };
        let needle = format!("name=\"{example}\"");
        let mut heading = None;
        for line in page.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                heading = Some(line.trim_start_matches('#').trim().to_string());
            } else if line.starts_with("<ComponentPreview") && line.contains(&needle) {
                return heading.or_else(|| Some(super::variant_title(variant)));
            }
        }
        Some(super::variant_title(variant))
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(playground) = self.playground.clone() {
            return div()
                .track_focus(&self.focus)
                .size_full()
                .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
                .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
                .child(playground)
                .into_any_element();
        }
        let theme = cx.theme().clone();
        let root = div()
            .track_focus(&self.focus)
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_sans.clone())
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx));

        if let Some(variant) = self.only_variant.clone() {
            let demo = &demos::ALL[self.selected];
            let element = (demo.render)(&variant, window, cx);
            return root
                .flex()
                .items_center()
                .justify_center()
                .p(preview_padding(demo.slug))
                .text_color(cx.theme().foreground.opacity(0.9))
                .text_size(px(14.))
                .line_height(relative(20. / 14.))
                .children(element)
                .into_any_element();
        }

        root.flex()
            .when(self.sidebar, |d| d.child(self.sidebar(cx)))
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    // `justify-items-center`: the article is centred in `main`.
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .w_full()
                            .child(self.page(window, cx)),
                    ),
            )
            .into_any_element()
    }
}

fn load_fonts(cx: &App) {
    let fonts = fonts::FONTS
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load fonts: {error:#}");
    }
}

/// What to show, from `--demo/--variant/--theme` natively or `?demo=&variant=&theme=` on the web.
struct Options {
    slug: Option<String>,
    variant: Option<String>,
    dark: bool,
    sidebar: bool,
    /// `--playground [slug]` / `?playground=slug`: open the playground.
    playground: Option<String>,
    /// Used by scripts/shot.sh to find the window.
    title: String,
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    run(
        gpui_platform::application(),
        Options {
            slug: flag("--demo"),
            variant: flag("--variant"),
            dark: flag("--theme").as_deref() == Some("dark"),
            sidebar: true,
            playground: args.iter().position(|a| a == "--playground").map(|i| {
                args.get(i + 1)
                    .filter(|v| !v.starts_with("--"))
                    .cloned()
                    .unwrap_or_default()
            }),
            title: std::env::var("AUDIOCN_GALLERY_TITLE")
                .unwrap_or_else(|_| "audiocn · GPUI".into()),
        },
    );
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();

    let params = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok());
    let get = |key: &str| {
        params
            .as_ref()
            .and_then(|p| p.get(key))
            .filter(|v| !v.is_empty() && v != "-")
    };
    let slug = get("demo");
    // Single-threaded: no SharedArrayBuffer / COOP+COEP headers needed.
    let platform = std::rc::Rc::new(gpui_web::WebPlatform::new(false));
    run(
        gpui::Application::with_platform(platform),
        Options {
            sidebar: slug.is_none(),
            slug,
            variant: get("variant"),
            dark: get("theme").as_deref() == Some("dark"),
            playground: params
                .as_ref()
                .and_then(|p| p.get("playground"))
                .map(|v| if v == "-" { String::new() } else { v }),
            title: "audiocn · GPUI".into(),
        },
    );
}

fn run(app: gpui::Application, options: Options) {
    let Options {
        slug,
        variant,
        dark,
        sidebar,
        playground,
        title,
    } = options;
    let selected = slug
        .as_deref()
        .and_then(|s| demos::ALL.iter().position(|d| d.slug == s))
        .unwrap_or(0);
    let unknown: HashSet<_> = slug.iter().filter(|s| demos::find(s).is_none()).collect();
    if !unknown.is_empty() {
        log::error!("unknown demo: {unknown:?}");
        #[cfg(not(target_family = "wasm"))]
        eprintln!("unknown demo: {unknown:?}");
    }

    app.run(move |cx: &mut App| {
        load_fonts(cx);
        audiocn::init(cx);
        // With the `device` feature, `AudioPlayer::new(..).src(..)` plays for real.
        #[cfg(all(feature = "device", not(target_family = "wasm")))]
        audiocn::io::install_audio_player_backend(cx);
        audiocn::theme::set_theme(if dark { Theme::dark() } else { Theme::light() }, cx);
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
        let window_size = if variant.is_some() {
            size(px(760.), px(520.))
        } else {
            size(px(1280.), px(860.))
        };
        let bounds = Bounds::centered(None, window_size, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| Gallery {
                    selected,
                    only_variant: variant,
                    sidebar,
                    scroll: ScrollHandle::new(),
                    focus: cx.focus_handle(),
                    playground: None,
                });
                if let Some(slug) = playground {
                    view.update(cx, |g, cx| {
                        g.open_playground(Some(slug.as_str()).filter(|s| !s.is_empty()), cx)
                    });
                }
                window.focus(&view.read(cx).focus.clone(), cx);
                view
            },
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
