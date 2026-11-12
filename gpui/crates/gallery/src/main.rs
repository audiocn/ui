//! The audiocn GPUI gallery: every docs page and example, running natively.
//!
//! ```sh
//! cargo run -p audiocn-gallery                          # docs-style browser with a sidebar
//! cargo run -p audiocn-gallery -- --demo knob           # open on one page
//! cargo run -p audiocn-gallery -- --demo knob --variant metal   # just one example, centred
//! cargo run -p audiocn-gallery -- --theme dark
//! ```

mod demos;

use std::collections::HashSet;

use audiocn::theme::{ActiveTheme, Theme};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, Context, FontWeight, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    point, px, size,
};
use gpui::{AppContext, InteractiveElement, KeyBinding, actions};

actions!(gallery, [FocusNext, FocusPrev]);

mod fonts {
    include!(concat!(env!("OUT_DIR"), "/fonts.rs"));
}

const HEADING: &str = "Outfit";

struct Gallery {
    selected: usize,
    /// Set by `--variant`: render only that example, centred, without chrome.
    only_variant: Option<String>,
    scroll: ScrollHandle,
    focus: gpui::FocusHandle,
}

impl Gallery {
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

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut list = div()
            .id("sidebar")
            .flex()
            .flex_col()
            .w(px(232.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.sidebar_border)
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .p_3()
            .gap_0p5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_2()
                    .child(
                        div()
                            .font_family(HEADING)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_lg()
                            .child("audiocn"),
                    )
                    .child(
                        div()
                            .id("theme-toggle")
                            .px_2()
                            .py_1()
                            .rounded(theme.radius_md())
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.sidebar_accent))
                            .child(if theme.is_dark { "Light" } else { "Dark" })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                    ),
            );

        let mut last = None;
        for (index, demo) in demos::ALL.iter().enumerate() {
            if last != Some(demo.section) {
                list = list.child(
                    div()
                        .px_2()
                        .pt_4()
                        .pb_1()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child(demo.section.label()),
                );
                last = Some(demo.section);
            }
            let active = index == self.selected;
            list = list.child(
                div()
                    .id(demo.slug)
                    .px_2()
                    .py_1()
                    .rounded(theme.radius_md())
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |d| {
                        d.bg(theme.sidebar_accent)
                            .text_color(theme.sidebar_accent_foreground)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!active, |d| d.hover(|s| s.bg(theme.sidebar_accent)))
                    .child(demo.title)
                    .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx))),
            );
        }
        list
    }

    /// One docs page: title, description, then every example in a preview box.
    fn page(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = &demos::ALL[self.selected];
        let theme = cx.theme().clone();
        let mut page = div()
            .flex()
            .flex_col()
            .gap_10()
            .px_12()
            .py_10()
            .max_w(px(920.))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_family(HEADING)
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(demo.title),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(demo.description),
                    ),
            );
        for variant in demo.variants {
            let element = (demo.render)(variant, window, cx);
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(*variant != "default", |d| {
                        d.child(
                            div()
                                .font_family(HEADING)
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(variant_title(variant)),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .min_h(px(350.))
                            .p_10()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(theme.radius_xl())
                            .bg(theme.background)
                            .children(element),
                    ),
            );
        }
        page
    }
}

fn variant_title(variant: &str) -> String {
    let words = variant.replace('-', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .p_10()
                .children(element)
                .into_any_element();
        }

        root.flex()
            .child(self.sidebar(cx))
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(self.page(window, cx)),
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let slug = flag("--demo");
    let variant = flag("--variant");
    let dark = flag("--theme").as_deref() == Some("dark");
    // Used by scripts/shot.sh to find the window.
    let title = std::env::var("AUDIOCN_GALLERY_TITLE").unwrap_or_else(|_| "audiocn · GPUI".into());

    let selected = slug
        .as_deref()
        .and_then(|s| demos::ALL.iter().position(|d| d.slug == s))
        .unwrap_or(0);
    let unknown: HashSet<_> = slug.iter().filter(|s| demos::find(s).is_none()).collect();
    if !unknown.is_empty() {
        eprintln!("unknown demo: {unknown:?}");
    }

    gpui_platform::application().run(move |cx: &mut App| {
        load_fonts(cx);
        audiocn::init(cx);
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
                    scroll: ScrollHandle::new(),
                    focus: cx.focus_handle(),
                });
                window.focus(&view.read(cx).focus.clone(), cx);
                view
            },
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
