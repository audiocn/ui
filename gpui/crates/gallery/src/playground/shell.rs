//! The playground window: component list, stage, props panel, code and events.

use std::collections::HashMap;
use std::rc::Rc;

use audiocn::components::{
    ParameterSlider, ParameterSliderControl, ParameterSliderHeader, ParameterSliderInput,
    ParameterSliderLabel,
};
use audiocn::signal::reduced_motion::{prefers_reduced_motion, set_reduced_motion};
use audiocn::theme::{ActiveTheme, Theme};
use audiocn::ui::{
    Button, ButtonSize, ButtonVariant, Input, Select, SelectContent, SelectItem, SelectSize,
    SelectTrigger, SelectValue, Switch, SwitchSize,
};
use audiocn::{AudioConfig, AudioSize, Orientation};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext, ClipboardItem, Context, Entity, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, relative,
};

use super::source::{self, SourceKind, SourceSettings, StaticEmitters};
use super::{
    COLOR_TOKENS, Control, ControlKind, Ctx, PageState, Props, SPECS, SourceUse, Spec, Value,
    all_specs, generate_code,
};

const THEME_PRESETS: &[&str] = &["stone", "ocean", "rose", "forest", "violet", "mono"];
const GLOBALS_CSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../app/globals.css"
));

/// Which value `AudioConfig` gets from the toolbar ("unset" leaves it `None`).
#[derive(Clone, Debug, Default)]
struct ConfigSettings {
    size: usize,        // 0 unset, 1 sm, 2 default, 3 lg
    orientation: usize, // 0 unset, 1 horizontal, 2 vertical
    disabled: bool,
    dimmed: bool,
}

impl ConfigSettings {
    fn config(&self) -> AudioConfig {
        AudioConfig {
            size: match self.size {
                1 => Some(AudioSize::Sm),
                2 => Some(AudioSize::Default),
                3 => Some(AudioSize::Lg),
                _ => None,
            },
            orientation: match self.orientation {
                1 => Some(Orientation::Horizontal),
                2 => Some(Orientation::Vertical),
                _ => None,
            },
            disabled: self.disabled,
            dimmed: self.dimmed,
            ..Default::default()
        }
    }
}

/// The playground view.
pub struct Playground {
    selected: &'static str,
    pages: HashMap<&'static str, Entity<PageState>>,
    source: SourceSettings,
    emitters: StaticEmitters,
    config: ConfigSettings,
    preset: usize,
    stage_width: f32,
    stage_fill: bool,
    on_exit: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    list_scroll: ScrollHandle,
    panel_scroll: ScrollHandle,
}

impl Playground {
    pub fn new(initial: Option<&str>) -> Self {
        let selected = initial
            .and_then(super::find)
            .or_else(|| all_specs().next())
            .map(|s| s.slug)
            .unwrap_or("level-meter");
        Self {
            selected,
            pages: HashMap::new(),
            source: SourceSettings::default(),
            emitters: StaticEmitters::default(),
            config: ConfigSettings::default(),
            preset: 0,
            stage_width: 720.0,
            stage_fill: false,
            on_exit: None,
            list_scroll: ScrollHandle::new(),
            panel_scroll: ScrollHandle::new(),
        }
    }

    /// Shows a "Docs" button that calls `f` (to leave the playground).
    pub fn on_exit(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_exit = Some(Rc::new(f));
        self
    }

    fn spec(&self) -> &'static Spec {
        super::find(self.selected).unwrap_or_else(|| all_specs().next().expect("specs"))
    }

    fn page(&mut self, spec: &'static Spec, cx: &mut Context<Self>) -> Entity<PageState> {
        if let Some(page) = self.pages.get(spec.slug) {
            return page.clone();
        }
        let props = Props::from_controls(&(spec.controls)());
        let page = cx.new(|_| PageState::new(props));
        cx.observe(&page, |_, _, cx| cx.notify()).detach();
        self.pages.insert(spec.slug, page.clone());
        page
    }

    fn apply_theme(&self, dark: bool, cx: &mut App) {
        let mut theme = if dark { Theme::dark() } else { Theme::light() };
        let preset = THEME_PRESETS[self.preset];
        if preset != "stone" {
            if let Some(block) = css_block(&format!("[data-theme=\"{preset}\"]")) {
                theme.apply_css(block);
            }
            if dark {
                if let Some(block) = css_block(&format!(".dark[data-theme=\"{preset}\"]")) {
                    theme.apply_css(block);
                }
            }
        }
        audiocn::theme::set_theme(theme, cx);
    }

    // ----- layout -------------------------------------------------------

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let dark = t.is_dark;
        let preset_items: Vec<SelectItem> = THEME_PRESETS
            .iter()
            .map(|p| SelectItem::new(*p, capitalize(p)))
            .collect();
        div()
            .flex()
            .items_center()
            .gap_4()
            .h(px(52.))
            .px_4()
            .flex_none()
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("audiocn"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(t.muted_foreground)
                            .child("Playground"),
                    ),
            )
            .child(div().flex_1())
            .child(labelled(
                "Theme",
                Select::new("pg-theme-preset")
                    .value(THEME_PRESETS[self.preset])
                    .on_value_change(by_value(cx.listener(|this, v: &SharedString, _, cx| {
                        this.preset = THEME_PRESETS
                            .iter()
                            .position(|p| *p == v.as_ref())
                            .unwrap_or(0);
                        let dark = cx.theme().is_dark;
                        this.apply_theme(dark, cx);
                        cx.notify();
                    })))
                    .trigger(
                        SelectTrigger::new()
                            .size(SelectSize::Sm)
                            .child(SelectValue::new()),
                    )
                    .content(SelectContent::new().children(preset_items)),
                &t,
            ))
            .child(labelled(
                "Dark",
                Switch::new("pg-dark")
                    .size(SwitchSize::Sm)
                    .checked(dark)
                    .on_checked_change(by_value(cx.listener(|this, on: &bool, _, cx| {
                        this.apply_theme(*on, cx);
                        cx.notify();
                    }))),
                &t,
            ))
            .child(labelled(
                "Reduced motion",
                Switch::new("pg-reduced-motion")
                    .size(SwitchSize::Sm)
                    .checked(prefers_reduced_motion(cx))
                    .on_checked_change(|on, _, cx| set_reduced_motion(on, cx)),
                &t,
            ))
            .when_some(self.on_exit.clone(), |el, exit| {
                el.child(
                    Button::new("pg-exit")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .child("Docs")
                        .on_click(move |_, window, cx| exit(window, cx)),
                )
            })
    }

    fn list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut list = div()
            .id("pg-list")
            .flex()
            .flex_col()
            .w(px(220.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .border_r_1()
            .border_color(t.sidebar_border)
            .bg(t.sidebar)
            .p_2();
        for (group, specs) in SPECS {
            list = list.child(
                div()
                    .px_2()
                    .pt_4()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.muted_foreground)
                    .child(*group),
            );
            for spec in specs.iter() {
                let active = spec.slug == self.selected;
                let slug = spec.slug;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("pg-item-{slug}")))
                        .px_2()
                        .py_1()
                        .rounded(t.radius_md())
                        .text_sm()
                        .cursor_pointer()
                        .when(active, |d| {
                            d.bg(t.sidebar_accent)
                                .text_color(t.sidebar_accent_foreground)
                                .font_weight(FontWeight::MEDIUM)
                        })
                        .when(!active, |d| d.hover(|s| s.bg(t.sidebar_accent)))
                        .child(spec.title())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = slug;
                            cx.notify();
                        })),
                );
            }
        }
        list
    }

    fn panel(
        &self,
        spec: &'static Spec,
        controls: &[Control],
        page: &Entity<PageState>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = cx.theme().clone();
        let props = page.read(cx).props.clone();
        let mut panel = div()
            .id("pg-panel")
            .flex()
            .flex_col()
            .gap_5()
            .w(px(320.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .track_scroll(&self.panel_scroll)
            .border_l_1()
            .border_color(t.border)
            .p_4();

        // Actions.
        let reset_page = page.clone();
        let reset_controls = controls.to_vec();
        let remount_page = page.clone();
        panel = panel.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("pg-reset")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .child("Reset props")
                        .on_click(move |_, _, cx| {
                            reset_page.update(cx, |s, cx| {
                                s.props = Props::from_controls(&reset_controls);
                                s.generation += 1;
                                cx.notify();
                            })
                        }),
                )
                .child(
                    Button::new("pg-remount")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .child("Remount")
                        .on_click(move |_, _, cx| {
                            remount_page.update(cx, |s, cx| {
                                s.generation += 1;
                                cx.notify();
                            })
                        }),
                ),
        );

        // Source.
        if spec.source != SourceUse::None {
            panel = panel.child(self.source_section(cx));
        }

        // Shared AudioConfig.
        panel = panel.child(self.config_section(cx));

        // Props, by group.
        let mut groups: Vec<&'static str> = Vec::new();
        for c in controls {
            if !groups.contains(&c.group) {
                groups.push(c.group);
            }
        }
        for group in groups {
            let mut section = section(group, &t);
            for c in controls.iter().filter(|c| c.group == group) {
                section = section.child(control_row(spec.slug, c, &props, page, &t));
            }
            panel = panel.child(section);
        }
        panel
    }

    fn source_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let s = &self.source;
        let kinds: Vec<SelectItem> = SourceKind::all()
            .into_iter()
            .map(|k| SelectItem::new(k.key(), k.label()))
            .collect();
        section("Source", &t)
            .child(field(
                "Signal",
                None,
                Select::new("pg-source-kind")
                    .value(s.kind.key())
                    .on_value_change(by_value(cx.listener(|this, v: &SharedString, _, cx| {
                        this.source.kind = SourceKind::from_key(v);
                        cx.notify();
                    })))
                    .trigger(
                        SelectTrigger::new()
                            .size(SelectSize::Sm)
                            .child(SelectValue::new()),
                    )
                    .content(SelectContent::new().children(kinds))
                    .into_any_element(),
                &t,
            ))
            .child(field(
                "Channels",
                None,
                Select::new("pg-source-channels")
                    .value(if s.channels == 1 { "1" } else { "2" })
                    .on_value_change(by_value(cx.listener(|this, v: &SharedString, _, cx| {
                        this.source.channels = if v.as_ref() == "1" { 1 } else { 2 };
                        cx.notify();
                    })))
                    .trigger(
                        SelectTrigger::new()
                            .size(SelectSize::Sm)
                            .child(SelectValue::new()),
                    )
                    .content(
                        SelectContent::new()
                            .child(SelectItem::new("1", "Mono"))
                            .child(SelectItem::new("2", "Stereo")),
                    )
                    .into_any_element(),
                &t,
            ))
            .when(
                matches!(
                    s.kind,
                    SourceKind::Speech | SourceKind::Music | SourceKind::Tone | SourceKind::Noise
                ),
                |el| {
                    el.child(num_slider(
                        "pg-source-gain",
                        "Gain (dB)",
                        s.gain_db,
                        -48.0,
                        12.0,
                        0.5,
                        cx.listener(|this, v: &f32, _, cx| {
                            this.source.gain_db = *v;
                            cx.notify();
                        }),
                    ))
                },
            )
            .when(s.kind == SourceKind::Static, |el| {
                el.child(num_slider(
                    "pg-source-peak",
                    "Peak (dB)",
                    s.static_peak_db,
                    -72.0,
                    6.0,
                    0.1,
                    cx.listener(|this, v: &f32, _, cx| {
                        this.source.static_peak_db = *v;
                        cx.notify();
                    }),
                ))
                .child(num_slider(
                    "pg-source-rms",
                    "RMS (dB)",
                    s.static_rms_db,
                    -72.0,
                    6.0,
                    0.1,
                    cx.listener(|this, v: &f32, _, cx| {
                        this.source.static_rms_db = *v;
                        cx.notify();
                    }),
                ))
            })
    }

    fn config_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let c = &self.config;
        section("AudioConfig", &t)
            .child(field(
                "Size",
                None,
                choice_select(
                    "pg-config-size",
                    c.size,
                    &["unset", "sm", "default", "lg"],
                    cx.listener(|this, i: &usize, _, cx| {
                        this.config.size = *i;
                        cx.notify();
                    }),
                ),
                &t,
            ))
            .child(field(
                "Orientation",
                None,
                choice_select(
                    "pg-config-orientation",
                    c.orientation,
                    &["unset", "horizontal", "vertical"],
                    cx.listener(|this, i: &usize, _, cx| {
                        this.config.orientation = *i;
                        cx.notify();
                    }),
                ),
                &t,
            ))
            .child(switch_row(
                "pg-config-disabled",
                "Disabled",
                c.disabled,
                cx.listener(|this, on: &bool, _, cx| {
                    this.config.disabled = *on;
                    cx.notify();
                }),
                &t,
            ))
            .child(switch_row(
                "pg-config-dimmed",
                "Dimmed",
                c.dimmed,
                cx.listener(|this, on: &bool, _, cx| {
                    this.config.dimmed = *on;
                    cx.notify();
                }),
                &t,
            ))
    }

    fn stage(&self, element: AnyElement, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(div().flex_1().child(num_slider(
                        "pg-stage-width",
                        "Stage width (px)",
                        self.stage_width,
                        200.0,
                        1100.0,
                        10.0,
                        cx.listener(|this, v: &f32, _, cx| {
                            this.stage_width = *v;
                            cx.notify();
                        }),
                    )))
                    .child(switch_row(
                        "pg-stage-fill",
                        "Fill",
                        self.stage_fill,
                        cx.listener(|this, on: &bool, _, cx| {
                            this.stage_fill = *on;
                            cx.notify();
                        }),
                        &t,
                    )),
            )
            .child(
                div().flex().justify_center().child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(self.stage_fill, |d| d.w_full())
                        .when(!self.stage_fill, |d| d.w(px(self.stage_width)))
                        .min_h(px(320.))
                        .p_10()
                        .border_1()
                        .border_color(t.border)
                        .rounded(t.radius_xl())
                        .bg(t.background)
                        .text_color(t.foreground.opacity(0.9))
                        .text_size(px(14.))
                        .line_height(relative(20. / 14.))
                        .child(element),
                ),
            )
    }

    fn code_and_log(
        &self,
        spec: &'static Spec,
        controls: &[Control],
        page: &Entity<PageState>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = cx.theme().clone();
        let state = page.read(cx);
        let code: SharedString = generate_code(spec, controls, &state.props).into();
        let entries: Vec<_> = state.log.iter().rev().take(200).cloned().collect();
        let count = state.log.len();
        let copy = code.clone();
        let clear = page.clone();
        let mono = t.font_mono.clone();
        div()
            .flex()
            .gap_4()
            .min_h(px(240.))
            .child(
                card(
                    "Code",
                    Button::new("pg-copy-code")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Xs)
                        .child("Copy")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()))
                        }),
                    &t,
                )
                .flex_1()
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_xs()
                        .whitespace_normal()
                        .children(
                            code.lines().map(|l| {
                                div().child(SharedString::from(l.replace(' ', "\u{00a0}")))
                            }),
                        ),
                ),
            )
            .child(
                card(
                    &format!("Events ({count})"),
                    Button::new("pg-clear-log")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Xs)
                        .child("Clear")
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |s, cx| {
                                s.log.clear();
                                cx.notify();
                            })
                        }),
                    &t,
                )
                .flex_1()
                .child(
                    div()
                        .id("pg-log")
                        .flex()
                        .flex_col()
                        .max_h(px(200.))
                        .overflow_y_scroll()
                        .font_family(mono)
                        .text_xs()
                        .when(entries.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_color(t.muted_foreground)
                                    .child("Interact with the component to see its callbacks."),
                            )
                        })
                        .children(entries.into_iter().map(|e| {
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(64.))
                                        .flex_none()
                                        .text_color(t.muted_foreground)
                                        .child(format!("{:.2}s", e.at_ms / 1000.0)),
                                )
                                .child(div().text_color(t.primary).child(e.event))
                                .child(div().text_color(t.foreground).child(e.detail))
                        })),
                ),
            )
    }
}

impl Render for Playground {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spec = self.spec();
        let controls = (spec.controls)();
        let page = self.page(spec, cx);
        let (meter, visual) = source::resolve(&self.source, &self.emitters, window, cx);
        let state = page.read(cx);
        let props = state.props.clone();
        let ctx = Ctx {
            state: page.clone(),
            slug: spec.slug,
            generation: state.generation,
            meter,
            visual,
            config: self.config.config(),
            channels: self.source.channels,
        };
        let element = (spec.render)(&props, &ctx, window, cx);
        let t = cx.theme().clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.foreground)
            .font_family(t.font_sans.clone())
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.list(cx))
                    .child(
                        div()
                            .id("pg-main")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(spec.title()),
                                    )
                                    .child(
                                        div()
                                            .font_family(t.font_mono.clone())
                                            .text_sm()
                                            .text_color(t.muted_foreground)
                                            .child(format!("audiocn::{}", spec.component)),
                                    ),
                            )
                            .child(self.stage(element, cx))
                            .child(self.code_and_log(spec, &controls, &page, cx)),
                    )
                    .child(self.panel(spec, &controls, &page, cx)),
            )
    }
}

// ----- panel widgets -------------------------------------------------------

/// Adapts a `cx.listener` (which takes `&T`) to callbacks that take `T`.
fn by_value<T: 'static>(
    f: impl Fn(&T, &mut Window, &mut App) + 'static,
) -> impl Fn(T, &mut Window, &mut App) + 'static {
    move |value, window, cx| f(&value, window, cx)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Finds `selector { ... }` in audiocn's globals.css and returns the body.
fn css_block(selector: &str) -> Option<&'static str> {
    let mut from = 0;
    while let Some(i) = GLOBALS_CSS[from..].find(selector) {
        let start = from + i;
        // Must be the whole selector (".dark[data-theme=..]" contains "[data-theme=..]").
        let before = GLOBALS_CSS[..start].chars().last();
        let rest = &GLOBALS_CSS[start + selector.len()..];
        if matches!(before, None | Some('\n')) && rest.trim_start().starts_with('{') {
            let open = start + selector.len() + rest.find('{')?;
            let close = open + GLOBALS_CSS[open..].find('}')?;
            return Some(&GLOBALS_CSS[open + 1..close]);
        }
        from = start + selector.len();
    }
    None
}

fn section(title: &str, t: &Theme) -> gpui::Div {
    div().flex().flex_col().gap_3().child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.muted_foreground)
            .child(title.to_uppercase()),
    )
}

fn card(title: &str, action: impl IntoElement, t: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .border_1()
        .border_color(t.border)
        .rounded(t.radius_lg())
        .bg(t.card)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(24.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .child(action),
        )
}

fn labelled(label: &str, control: impl IntoElement, t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(t.muted_foreground)
                .child(label.to_string()),
        )
        .child(control)
}

fn field(label: &str, help: Option<&str>, control: AnyElement, t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
        .when_some(help, |el, help| {
            el.child(
                div()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(help.to_string()),
            )
        })
        .child(control)
}

fn switch_row(
    id: &str,
    label: &str,
    checked: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
    t: &Theme,
) -> impl IntoElement {
    let _ = t;
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
        .child(
            Switch::new(SharedString::from(id.to_string()))
                .size(SwitchSize::Sm)
                .checked(checked)
                .on_checked_change(move |on, w, cx| on_change(&on, w, cx)),
        )
}

fn num_slider(
    id: &str,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    on_change: impl Fn(&f32, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    ParameterSlider::new(SharedString::from(id.to_string()))
        .min(min)
        .max(max)
        .step(step)
        .value(value.clamp(min, max))
        .on_value_change(move |v, w, cx| on_change(&v, w, cx))
        .child(
            ParameterSliderHeader::new()
                .child(ParameterSliderLabel::new(label.to_string()))
                .child(ParameterSliderInput::new()),
        )
        .child(ParameterSliderControl::new())
}

fn choice_select(
    id: &str,
    index: usize,
    options: &[&str],
    on_change: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let labels: Vec<String> = options.iter().map(|s| s.to_string()).collect();
    let items: Vec<SelectItem> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| SelectItem::new(i.to_string(), l.clone()))
        .collect();
    Select::new(SharedString::from(id.to_string()))
        .value(index.to_string())
        .on_value_change(move |v, w, cx| {
            if let Ok(i) = v.parse::<usize>() {
                on_change(&i, w, cx);
            }
        })
        .trigger(
            SelectTrigger::new()
                .size(SelectSize::Sm)
                .child(SelectValue::new()),
        )
        .content(SelectContent::new().children(items))
        .into_any_element()
}

/// One prop control, writing straight into the page state.
fn control_row(
    slug: &'static str,
    c: &Control,
    props: &Props,
    page: &Entity<PageState>,
    t: &Theme,
) -> AnyElement {
    let key = c.key;
    let remount = c.remount;
    let id = format!("pg-ctl-{slug}-{key}");
    let write = {
        let page = page.clone();
        move |value: Value, cx: &mut App| {
            page.update(cx, |s, cx| {
                s.props.set(key, value);
                if remount {
                    s.generation += 1;
                }
                cx.notify();
            })
        }
    };
    let body: AnyElement = match &c.kind {
        ControlKind::Num { min, max, step } => num_slider(
            &id,
            c.label,
            props.num(key),
            *min,
            *max,
            *step,
            move |v, _, cx| write(Value::Num(*v), cx),
        )
        .into_any_element(),
        ControlKind::Bool => switch_row(
            &id,
            c.label,
            props.bool(key),
            move |on, _, cx| write(Value::Bool(*on), cx),
            t,
        )
        .into_any_element(),
        ControlKind::Choice(options) => {
            let labels: Vec<&str> = options.iter().map(|o| o.0).collect();
            field(
                c.label,
                None,
                choice_select(&id, props.index(key), &labels, move |i, _, cx| {
                    write(Value::Choice(*i), cx)
                }),
                t,
            )
            .into_any_element()
        }
        ControlKind::Color => {
            let current = match props.get(key) {
                Some(Value::Color(i)) => *i,
                _ => 0,
            };
            field(
                c.label,
                None,
                choice_select(&id, current, COLOR_TOKENS, move |i, _, cx| {
                    write(Value::Color(*i), cx)
                }),
                t,
            )
            .into_any_element()
        }
        ControlKind::Text => field(
            c.label,
            None,
            Input::new(SharedString::from(id.clone()))
                .value(props.text(key))
                .on_change(move |text, _, cx| write(Value::Text(text), cx))
                .into_any_element(),
            t,
        )
        .into_any_element(),
    };
    match c.help {
        Some(help)
            if !matches!(
                c.kind,
                ControlKind::Choice(_) | ControlKind::Color | ControlKind::Text
            ) =>
        {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(body)
                .child(div().text_xs().text_color(t.muted_foreground).child(help))
                .into_any_element()
        }
        _ => body,
    }
}
