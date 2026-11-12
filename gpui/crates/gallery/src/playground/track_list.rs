//! Playground page for `TrackList`: data sets, active and playing rows, selection log.

use audiocn::audio::time::format_time;
use audiocn::ui::icons::IconName;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    TrackList, TrackListItem, TrackListItemActions, TrackListItemArtwork, TrackListItemContent,
    TrackListItemDescription, TrackListItemDuration, TrackListItemIndex, TrackListItemTitle,
    TrackListSize, TrackListVariant,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Control, Ctx, Props, SourceUse, Spec, Value};

pub const SPEC: Spec = Spec {
    slug: "track-list",
    component: "TrackList",
    source: SourceUse::None,
    controls,
    render,
};

struct Track {
    id: &'static str,
    title: &'static str,
    artist: &'static str,
    duration: f64,
}

const TRACKS: [Track; 12] = [
    Track {
        id: "night-drive",
        title: "Night Drive",
        artist: "audiocn",
        duration: 19.2,
    },
    Track {
        id: "low-tide",
        title: "Low Tide",
        artist: "audiocn",
        duration: 24.0,
    },
    Track {
        id: "arcade",
        title: "Arcade",
        artist: "audiocn",
        duration: 13.7,
    },
    Track {
        id: "locked",
        title: "Unreleased",
        artist: "Not available",
        duration: 201.0,
    },
    Track {
        id: "glasshouse",
        title: "Glasshouse",
        artist: "audiocn",
        duration: 32.5,
    },
    Track {
        id: "harbour",
        title: "Harbour Lights",
        artist: "field recordings",
        duration: 48.0,
    },
    Track {
        id: "kiln",
        title: "Kiln",
        artist: "audiocn",
        duration: 16.4,
    },
    Track {
        id: "paper",
        title: "Paper Boats",
        artist: "audiocn",
        duration: 27.8,
    },
    Track {
        id: "radio",
        title: "Late Radio",
        artist: "audiocn",
        duration: 41.0,
    },
    Track {
        id: "static",
        title: "Static Garden",
        artist: "audiocn",
        duration: 11.2,
    },
    Track {
        id: "velvet",
        title: "Velvet Hour",
        artist: "audiocn",
        duration: 36.6,
    },
    Track {
        id: "yellow",
        title: "Yellow Line",
        artist: "audiocn",
        duration: 22.1,
    },
];

const DATASETS: &[(&str, &str)] = &[("4 tracks", ""), ("12 tracks", ""), ("empty", "")];

const SIZES: &[(&str, &str)] = &[
    ("default", "TrackListSize::Default"),
    ("sm", "TrackListSize::Sm"),
    ("lg", "TrackListSize::Lg"),
];

const VARIANTS: &[(&str, &str)] = &[
    ("default", "TrackListVariant::Default"),
    ("outline", "TrackListVariant::Outline"),
];

const IDS: &[(&str, &str)] = &[
    ("none", "None"),
    ("night-drive", "\"night-drive\""),
    ("low-tide", "\"low-tide\""),
    ("arcade", "\"arcade\""),
    ("locked", "\"locked\""),
    ("glasshouse", "\"glasshouse\""),
    ("harbour", "\"harbour\""),
    ("kiln", "\"kiln\""),
    ("paper", "\"paper\""),
    ("radio", "\"radio\""),
    ("static", "\"static\""),
    ("velvet", "\"velvet\""),
    ("yellow", "\"yellow\""),
];

const DISABLED: &[(&str, &str)] = &[
    ("none", ""),
    ("last row", ""),
    ("unreleased", ""),
    ("alternate", ""),
];

fn controls() -> Vec<Control> {
    vec![
        Control::choice("dataset", "Tracks", 0, DATASETS)
            .group("Data")
            .no_code()
            .help("4 includes the locked row. 12 is the long list. Empty has no rows."),
        Control::choice("size", "Size", 0, SIZES).group("Props"),
        Control::choice("variant", "Variant", 0, VARIANTS).group("Props"),
        Control::choice("active", "Active", 2, IDS)
            .group("Selection")
            .no_code()
            .help("Click a row to move this. Click again to toggle playing."),
        Control::choice("playing", "Playing", 2, IDS)
            .group("Selection")
            .no_code(),
        Control::choice("disabled_rows", "Disabled rows", 2, DISABLED)
            .group("Selection")
            .no_code(),
        Control::bool("show_index", "Index", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_artwork", "Artwork", false)
            .group("Parts")
            .no_code(),
        Control::text("artwork_src", "Artwork src", "")
            .group("Parts")
            .no_code()
            .help("URL for TrackListItemArtwork. Empty shows a muted tile."),
        Control::bool("show_title", "Title", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_description", "Description", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_duration", "Duration", true)
            .group("Parts")
            .no_code(),
        Control::bool("show_actions", "Actions", true)
            .group("Parts")
            .no_code(),
    ]
}

fn visible_count(label: &str) -> usize {
    match label {
        "12 tracks" => TRACKS.len(),
        "empty" => 0,
        _ => 4,
    }
}

fn size_of(label: &str) -> TrackListSize {
    match label {
        "sm" => TrackListSize::Sm,
        "lg" => TrackListSize::Lg,
        _ => TrackListSize::Default,
    }
}

fn variant_of(label: &str) -> TrackListVariant {
    match label {
        "outline" => TrackListVariant::Outline,
        _ => TrackListVariant::Default,
    }
}

fn render(p: &Props, ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let count = visible_count(p.choice("dataset"));
    let tracks = &TRACKS[..count];
    let active = p.choice("active");
    let playing = p.choice("playing");
    let disabled_rows = p.choice("disabled_rows");
    let mut list = TrackList::new(ctx.id("list"))
        .w_full()
        .size(size_of(p.choice("size")))
        .variant(variant_of(p.choice("variant")));
    for (index, track) in tracks.iter().enumerate() {
        list = list.child(row(
            p,
            ctx,
            RowSpec {
                track,
                index,
                count,
                active,
                playing,
                disabled_rows,
            },
        ));
    }
    list.into_any_element()
}

struct RowSpec<'a> {
    track: &'a Track,
    index: usize,
    count: usize,
    active: &'a str,
    playing: &'a str,
    disabled_rows: &'a str,
}

fn row(p: &Props, ctx: &Ctx, spec: RowSpec<'_>) -> TrackListItem {
    let RowSpec {
        track,
        index,
        count,
        active,
        playing,
        disabled_rows,
    } = spec;
    let disabled = match disabled_rows {
        "last row" => index + 1 == count,
        "unreleased" => track.id == "locked",
        "alternate" => index % 2 == 1,
        _ => false,
    };
    let id = track.id;
    let was_active = active == id;
    let was_playing = playing == id;
    let select = ctx.clone();
    let action = ctx.clone();
    let active_index = index + 1;
    let mut item = TrackListItem::new(ctx.id(id))
        .active(was_active)
        .playing(was_playing)
        .disabled(disabled)
        .on_select(move |_, cx| {
            select.set("active", Value::Choice(active_index), cx);
            let next_playing = if was_active && was_playing {
                0
            } else {
                active_index
            };
            select.set("playing", Value::Choice(next_playing), cx);
            select.log("on_select", id, cx);
        });
    if p.bool("show_index") {
        item = item.child(TrackListItemIndex::new().child((index + 1).to_string()));
    }
    if p.bool("show_artwork") {
        let src = p.text("artwork_src");
        item = if src.is_empty() {
            item.child(
                div()
                    .size(px(36.0))
                    .flex_shrink_0()
                    .rounded(px(6.0))
                    .bg(gpui::rgb(0x808080).opacity(0.2)),
            )
        } else {
            item.child(TrackListItemArtwork::new(src).alt(track.title))
        };
    }
    if p.bool("show_title") || p.bool("show_description") {
        let mut content = TrackListItemContent::new();
        if p.bool("show_title") {
            content = content.child(TrackListItemTitle::new().child(track.title));
        }
        if p.bool("show_description") {
            content = content.child(TrackListItemDescription::new().child(track.artist));
        }
        item = item.child(content);
    }
    if p.bool("show_duration") {
        item = item.child(
            TrackListItemDuration::new().child(format_time(track.duration, Default::default())),
        );
    }
    if p.bool("show_actions") {
        let title = track.title;
        item = item.child(
            TrackListItemActions::new().child(
                Button::new(ctx.id(&format!("{id}/more")))
                    .aria_label(format!("More options for {title}"))
                    .icon(IconName::DotsThree)
                    .size(ButtonSize::IconXs)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| action.log("on_click", format!("actions {id}"), cx)),
            ),
        );
    }
    item
}
