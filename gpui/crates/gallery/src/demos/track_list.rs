//! Gallery page for `track-list`. One variant per docs example: `components/examples/track-list-demo.tsx`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, px};

use audiocn::audio::time::format_time;
use audiocn::ui::icons::IconName;
use audiocn::ui::{Button, ButtonSize, ButtonVariant};
use audiocn::{
    TrackList, TrackListItem, TrackListItemActions, TrackListItemContent, TrackListItemDescription,
    TrackListItemDuration, TrackListItemIndex, TrackListItemTitle, TrackListVariant,
};

use super::{Demo, Section, id};

pub const DEMO: Demo = Demo {
    slug: "track-list",
    title: "Track List",
    description: "A list of tracks with active and playing states and arrow-key navigation.",
    section: Section::Components,
    variants: &["default"],
    render,
};

fn render(variant: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    Some(match variant {
        "default" => demo(window, cx),
        _ => return None,
    })
}

struct Track {
    artist: &'static str,
    duration: f64,
    id: &'static str,
    title: &'static str,
}

const TRACKS: [Track; 4] = [
    Track {
        artist: "audiocn",
        duration: 19.2,
        id: "night-drive",
        title: "Night Drive",
    },
    Track {
        artist: "audiocn",
        duration: 24.0,
        id: "low-tide",
        title: "Low Tide",
    },
    Track {
        artist: "audiocn",
        duration: 13.7,
        id: "arcade",
        title: "Arcade",
    },
    Track {
        artist: "Not available",
        duration: 201.0,
        id: "locked",
        title: "Unreleased",
    },
];

/// `const [current, setCurrent] = useState("low-tide"); const [playing, setPlaying] = useState(true);`
struct DemoState {
    current: &'static str,
    playing: bool,
}

/// `track-list-demo.tsx`.
fn demo(window: &mut Window, cx: &mut App) -> AnyElement {
    let base = id("track-list", "default");
    let state = window.use_keyed_state(format!("{base}/state"), cx, |_, _| DemoState {
        current: "low-tide",
        playing: true,
    });
    let (current, playing) = {
        let s = state.read(cx);
        (s.current, s.playing)
    };

    let mut list = TrackList::new(base.clone())
        .max_w(px(448.0))
        .variant(TrackListVariant::Outline);
    for (index, track) in TRACKS.iter().enumerate() {
        let state = state.clone();
        let track_id = track.id;
        list = list.child(
            TrackListItem::new(format!("{base}/{}", track.id))
                .active(track.id == current)
                .disabled(track.id == "locked")
                .on_select(move |_, cx| {
                    state.update(cx, |s, cx| {
                        s.playing = if track_id == s.current {
                            !s.playing
                        } else {
                            true
                        };
                        s.current = track_id;
                        cx.notify();
                    });
                })
                .playing(track.id == current && playing)
                .child(TrackListItemIndex::new().child((index + 1).to_string()))
                .child(
                    TrackListItemContent::new()
                        .child(TrackListItemTitle::new().child(track.title))
                        .child(TrackListItemDescription::new().child(track.artist)),
                )
                .child(
                    TrackListItemDuration::new()
                        .child(format_time(track.duration, Default::default())),
                )
                .child(
                    TrackListItemActions::new().child(
                        Button::new(format!("{base}/{}/more", track.id))
                            .aria_label(format!("More options for {}", track.title))
                            .size(ButtonSize::IconXs)
                            .variant(ButtonVariant::Ghost)
                            .icon(IconName::DotsThree),
                    ),
                ),
        );
    }
    list.into_any_element()
}
