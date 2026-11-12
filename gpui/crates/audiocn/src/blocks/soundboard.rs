//! Port of `components/blocks/soundboard/soundboard.tsx`.
//!
//! # Audio seam
//!
//! React's board calls `useSound` and `useGainNode` and plays through the
//! speakers (or an `output` node). This one never opens a device. Each pad is
//! either:
//!
//! - **Simulated** (the default). A [`SimulatedPlayer`] of [`SoundboardSound::duration`]
//!   seconds — or the docs-demo length when `id` is `airhorn`, `applause`, …
//!   — drives `playing` and the progress bar. Trigger, stop, loop and "Stop
//!   all" go through that player. There is no audio output.
//! - **Host-owned.** Set [`SoundboardSound::voice`] and play the clip yourself
//!   from [`Soundboard::on_play`] / [`Soundboard::on_stop`]. The pad shows
//!   `voice.playing`, `voice.loading` and `voice.progress`.
//!
//! [`Soundboard::on_play`] still fires for simulated pads, so a host can start
//! a real clip in parallel with the progress bar. [`Soundboard::on_output_gain`]
//! is the master bus (`muted ? 0 : volume²`, the React `useGainNode`). Dropped
//! files are not decoded: they become pads with a 1.5 s simulated clip whose
//! `src` is the path. Wire [`Soundboard::on_play`] to play that path.
//!
//! ```ignore
//! Soundboard::new("board")
//!     .default_sounds(sounds)
//!     .on_play(|sound, _, _| play_file(&sound.src))
//!     .on_stop(|id, _, _| stop_file(id))
//!     .on_output_gain(|gain, _, _| bus.set_gain(gain))
//! ```

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    PathPromptOptions, Refineable, RenderOnce, SharedString, StyleRefinement, Styled, Task, Window,
    div, prelude::FluentBuilder, px,
};

use crate::audio::FrameSource;
use crate::components::{
    AudioPlayerController, HotkeyScope, SimulatedPlayer, SoundPad, SoundPadGrid, SoundPadLabel,
    SoundPadMode, SoundPadProgress, SoundPadShortcut, SoundPadVariant, UseAudioPlayerOptions,
    VolumeControl, VolumeControlMute, VolumeControlSlider, use_simulated_player,
};
use crate::theme::ActiveTheme;
use crate::ui::icons::{Icon, IconName};
use crate::ui::styles::{TextSize, ring_shadow};
use crate::ui::{
    Alert, AlertDescription, Button, ButtonSize, ButtonVariant, ContextMenu, ContextMenuContent,
    ContextMenuGroup, ContextMenuItem, ContextMenuItemVariant, ContextMenuLabel,
    ContextMenuRadioGroup, ContextMenuRadioItem, ContextMenuSeparator, Empty, EmptyDescription,
    EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle, Label, Switch, SwitchSize,
};

/// React `--font-heading` (Outfit). Bundled with the gallery.
const HEADING_FONT: &str = "Outfit";
/// Stand-in length for a dropped file. The board does not decode audio.
const DROPPED_CLIP_SECONDS: f64 = 1.5;

const MODES: [(SoundPadMode, &str); 4] = [
    (SoundPadMode::OneShot, "One shot"),
    (SoundPadMode::Toggle, "Toggle"),
    (SoundPadMode::Hold, "Hold"),
    (SoundPadMode::Loop, "Loop"),
];

const VOLUMES: [f32; 4] = [1.0, 0.75, 0.5, 0.25];

const AUDIO_EXTENSIONS: &[&str] = &[
    "aac", "aif", "aiff", "caf", "flac", "m4a", "mp3", "oga", "ogg", "opus", "wav", "wave", "weba",
    "webm", "wma",
];

type SoundsHandler = Rc<dyn Fn(Vec<SoundboardSound>, &mut Window, &mut App) + 'static>;
type PlayHandler = Rc<dyn Fn(&SoundboardSound, &mut Window, &mut App) + 'static>;
type StopHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type GainHandler = Rc<dyn Fn(f32, &mut Window, &mut App) + 'static>;

/// Playback a host drives for one pad (`useSound`'s `isPlaying` / `isLoaded` / `progress`).
///
/// ```ignore
/// SoundboardVoice { playing: true, loading: false, progress: Some(progress) }
/// ```
#[derive(Clone)]
pub struct SoundboardVoice {
    /// The pad is playing (`playing`).
    pub playing: bool,
    /// The clip is not decoded yet (`loading`). The pad is inert while this is set.
    pub loading: bool,
    /// 0..1 progress (`SoundPadProgress`'s source).
    pub progress: Option<FrameSource<f32>>,
}

/// One pad: the React `SoundboardSound` fields, plus the GPUI playback seam.
///
/// ```ignore
/// SoundboardSound::new("airhorn", "Airhorn", "airhorn")
///     .hotkey("1")
///     .accent(oklch(0.7, 0.2, 30.0))
///     .duration(1.4)
/// ```
#[derive(Clone)]
pub struct SoundboardSound {
    pub id: SharedString,
    pub label: SharedString,
    /// Clip id or filesystem path. The board does not decode it.
    pub src: SharedString,
    pub hotkey: Option<SharedString>,
    /// Default `OneShot`.
    pub mode: Option<SoundPadMode>,
    /// 0..1. Default 1.
    pub volume: Option<f32>,
    pub accent: Option<gpui::Hsla>,
    /// Simulator length in seconds. `None` uses the docs-demo length when
    /// `id` (or `src`) is a known demo sound, otherwise the pad stays loading.
    pub duration: Option<f64>,
    /// Host playback. When set, the built-in simulator is not used.
    pub voice: Option<SoundboardVoice>,
}

impl SoundboardSound {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        src: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            src: src.into(),
            hotkey: None,
            mode: None,
            volume: None,
            accent: None,
            duration: None,
            voice: None,
        }
    }

    pub fn hotkey(mut self, hotkey: impl Into<SharedString>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn mode(mut self, mode: SoundPadMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// 0..1. Default 1.
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = Some(volume);
        self
    }

    pub fn accent(mut self, accent: gpui::Hsla) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Simulator length in seconds.
    pub fn duration(mut self, seconds: f64) -> Self {
        self.duration = Some(seconds);
        self
    }

    /// Host-owned playback. The board shows this instead of simulating.
    pub fn voice(mut self, voice: SoundboardVoice) -> Self {
        self.voice = Some(voice);
        self
    }
}

#[derive(Clone)]
struct RemovedSound {
    sound: SoundboardSound,
    index: usize,
}

struct BoardState {
    sounds: Vec<SoundboardSound>,
    volume: f32,
    muted: bool,
    hotkeys: bool,
    feedback: SharedString,
    removed: Option<RemovedSound>,
    /// Paths this board added (React's object-URL set). Nothing is revoked:
    /// a drop does not copy the file.
    owned_srcs: HashSet<SharedString>,
    players: Rc<RefCell<HashMap<SharedString, Entity<SimulatedPlayer>>>>,
    picker: Option<Task<()>>,
}

impl BoardState {
    fn new(sounds: Vec<SoundboardSound>) -> Self {
        Self {
            sounds,
            volume: 1.0,
            muted: false,
            hotkeys: true,
            feedback: SharedString::default(),
            removed: None,
            owned_srcs: HashSet::new(),
            players: Rc::new(RefCell::new(HashMap::new())),
            picker: None,
        }
    }
}

fn child_id(id: &ElementId, suffix: &str) -> ElementId {
    ElementId::Name(format!("{id}/{suffix}").into())
}

/// Docs-demo clip length (`SOUNDS` in `lib/docs/demo-audio.ts`), in seconds.
pub fn demo_sound_duration(id: &str) -> Option<f64> {
    Some(match id {
        "airhorn" => 1.4,
        "applause" => 2.4,
        "ding" => 1.7,
        "laser" => 0.5,
        "success" => 1.3,
        "fail" => 2.0,
        "whoosh" => 1.2,
        "drumroll" => 2.8,
        _ => return None,
    })
}

fn clip_duration(sound: &SoundboardSound) -> Option<f64> {
    if sound.voice.is_some() {
        return None;
    }
    sound
        .duration
        .or_else(|| demo_sound_duration(sound.id.as_ref()))
        .or_else(|| demo_sound_duration(sound.src.as_ref()))
}

fn mode_of(sound: &SoundboardSound) -> SoundPadMode {
    sound.mode.unwrap_or_default()
}

fn mode_token(mode: SoundPadMode) -> &'static str {
    match mode {
        SoundPadMode::OneShot => "one-shot",
        SoundPadMode::Toggle => "toggle",
        SoundPadMode::Hold => "hold",
        SoundPadMode::Loop => "loop",
    }
}

fn parse_mode(value: &str) -> SoundPadMode {
    match value {
        "toggle" => SoundPadMode::Toggle,
        "hold" => SoundPadMode::Hold,
        "loop" => SoundPadMode::Loop,
        _ => SoundPadMode::OneShot,
    }
}

/// `String(volume)` for the volume radio values (`"1"`, `"0.75"`, `"0.5"`, `"0.25"`).
fn volume_token(volume: f32) -> SharedString {
    for value in VOLUMES {
        if (volume - value).abs() < 0.001 {
            return match value {
                v if (v - 1.0).abs() < 0.001 => "1".into(),
                v if (v - 0.75).abs() < 0.001 => "0.75".into(),
                v if (v - 0.5).abs() < 0.001 => "0.5".into(),
                _ => "0.25".into(),
            };
        }
    }
    format!("{volume}").into()
}

fn bus_gain(volume: f32, muted: bool) -> f32 {
    if muted { 0.0 } else { volume * volume }
}

fn emit_gain(
    handler: &Option<GainHandler>,
    volume: f32,
    muted: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = handler {
        handler(bus_gain(volume, muted), window, cx);
    }
}

fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

fn file_stamp(path: &Path) -> u128 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// `fileToSound`: id `name-lastModified`, label without the extension,
/// hotkey `1`–`9` for the first nine positions.
fn file_to_sound(path: &Path, index: usize) -> SoundboardSound {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("sound");
    let label = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(name);
    let mut sound = SoundboardSound::new(
        format!("{name}-{}", file_stamp(path)),
        label,
        path.display().to_string(),
    )
    .duration(DROPPED_CLIP_SECONDS);
    if index < 9 {
        sound = sound.hotkey((index + 1).to_string());
    }
    sound
}

/// Audio paths that are not already on the board, in drop order.
fn collect_added(sounds: &[SoundboardSound], paths: &[PathBuf]) -> Vec<SoundboardSound> {
    let mut existing: HashSet<SharedString> = sounds.iter().map(|sound| sound.id.clone()).collect();
    let mut added = Vec::new();
    for path in paths {
        if !is_audio_path(path) {
            continue;
        }
        let sound = file_to_sound(path, sounds.len() + added.len());
        if !existing.insert(sound.id.clone()) {
            continue;
        }
        added.push(sound);
    }
    added
}

fn added_feedback(count: usize) -> SharedString {
    if count == 0 {
        "No new sounds added. Choose audio files that aren't already on the board.".into()
    } else if count == 1 {
        "Added 1 sound.".into()
    } else {
        format!("Added {count} sounds.").into()
    }
}

fn commit(
    state: &Entity<BoardState>,
    on_change: &Option<SoundsHandler>,
    next: Vec<SoundboardSound>,
    feedback: Option<SharedString>,
    removed: Option<Option<RemovedSound>>,
    window: &mut Window,
    cx: &mut App,
) {
    let owned = next
        .iter()
        .map(|sound| sound.src.clone())
        .filter(|src| state.read(cx).owned_srcs.contains(src))
        .collect::<Vec<_>>();
    state.update(cx, |board, cx| {
        if let Some(removed) = &removed {
            if let Some(previous) = board.removed.take() {
                let in_use = next.iter().any(|item| item.src == previous.sound.src);
                if !in_use {
                    board.owned_srcs.remove(&previous.sound.src);
                }
            }
            board.removed = removed.clone();
        }
        for src in &owned {
            board.owned_srcs.insert(src.clone());
        }
        for sound in &next {
            if sound.duration == Some(DROPPED_CLIP_SECONDS) {
                board.owned_srcs.insert(sound.src.clone());
            }
        }
        board.sounds = next.clone();
        if let Some(feedback) = feedback.clone() {
            board.feedback = feedback;
        }
        cx.notify();
    });
    if let Some(handler) = on_change {
        handler(next, window, cx);
    }
}

fn map_sound(
    sounds: &[SoundboardSound],
    id: &SharedString,
    mut edit: impl FnMut(&mut SoundboardSound),
) -> Vec<SoundboardSound> {
    sounds
        .iter()
        .map(|sound| {
            if &sound.id == id {
                let mut next = sound.clone();
                edit(&mut next);
                next
            } else {
                sound.clone()
            }
        })
        .collect()
}

fn release_player(
    players: &RefCell<HashMap<SharedString, Entity<SimulatedPlayer>>>,
    id: &SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    let player = players.borrow().get(id).cloned();
    if let Some(player) = player {
        player.pause(window, cx);
        player.seek(0.0, window, cx);
    }
}

fn add_paths(
    state: &Entity<BoardState>,
    sounds: &[SoundboardSound],
    paths: &[PathBuf],
    on_change: &Option<SoundsHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let added = collect_added(sounds, paths);
    let feedback = added_feedback(added.len());
    if added.is_empty() {
        state.update(cx, |board, cx| {
            board.feedback = feedback;
            cx.notify();
        });
        return;
    }
    let mut next = sounds.to_vec();
    next.extend(added);
    commit(state, on_change, next, Some(feedback), None, window, cx);
}

fn begin_pick(
    state: &Entity<BoardState>,
    sounds: Vec<SoundboardSound>,
    on_change: Option<SoundsHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some("Add audio files".into()),
    });
    let state_for_pick = state.clone();
    let task = window.spawn(cx, async move |cx| {
        let Ok(Ok(Some(paths))) = rx.await else {
            return;
        };
        cx.update(|window, cx| {
            add_paths(&state_for_pick, &sounds, &paths, &on_change, window, cx);
        })
        .ok();
    });
    state.update(cx, |board, _| board.picker = Some(task));
}

/// Sound pads with hotkeys, modes, a master volume, stop-all and file drop.
///
/// Controlled with [`Soundboard::sounds`], seeded with [`Soundboard::default_sounds`].
/// Right-click a pad for mode, volume and remove. Removing a pad shows an
/// inline Undo that puts it back. Drop audio files onto the board, or use Add.
///
/// ```ignore
/// Soundboard::new("fx")
///     .columns(4)
///     .default_sounds(vec![
///         SoundboardSound::new("airhorn", "Airhorn", "airhorn").hotkey("1").duration(1.4),
///     ])
/// ```
#[derive(IntoElement)]
pub struct Soundboard {
    id: ElementId,
    sounds: Option<Vec<SoundboardSound>>,
    default_sounds: Vec<SoundboardSound>,
    on_sounds_change: Option<SoundsHandler>,
    on_play: Option<PlayHandler>,
    on_stop: Option<StopHandler>,
    on_output_gain: Option<GainHandler>,
    columns: usize,
    style: StyleRefinement,
}

impl Soundboard {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            sounds: None,
            default_sounds: Vec::new(),
            on_sounds_change: None,
            on_play: None,
            on_stop: None,
            on_output_gain: None,
            columns: 4,
            style: StyleRefinement::default(),
        }
    }

    /// Controlled pad list.
    pub fn sounds(mut self, sounds: Vec<SoundboardSound>) -> Self {
        self.sounds = Some(sounds);
        self
    }

    /// Uncontrolled seed. Default empty (the dashed empty state).
    pub fn default_sounds(mut self, sounds: Vec<SoundboardSound>) -> Self {
        self.default_sounds = sounds;
        self
    }

    /// Pads were added, edited or removed (`onSoundsChange`).
    pub fn on_sounds_change(
        mut self,
        f: impl Fn(Vec<SoundboardSound>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sounds_change = Some(Rc::new(f));
        self
    }

    /// A pad asked to play (`useSound().play`). Also fired for simulated pads.
    pub fn on_play(
        mut self,
        f: impl Fn(&SoundboardSound, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_play = Some(Rc::new(f));
        self
    }

    /// A pad asked to stop, including each pad on "Stop all".
    pub fn on_stop(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(f));
        self
    }

    /// Master bus gain, `0` while muted and `volume²` otherwise (`useGainNode`).
    /// Fires when the slider or mute button changes. The initial gain is 1.
    pub fn on_output_gain(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_output_gain = Some(Rc::new(f));
        self
    }

    /// The most columns. Default 4. The grid drops columns to keep pads at
    /// least 88 px wide.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns;
        self
    }
}

impl Styled for Soundboard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Soundboard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default_sounds = self.default_sounds.clone();
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, move |_, _| {
            BoardState::new(default_sounds)
        });
        let (volume, muted, hotkeys, feedback, removed, players) = {
            let board = state.read(cx);
            (
                board.volume,
                board.muted,
                board.hotkeys,
                board.feedback.clone(),
                board.removed.clone(),
                board.players.clone(),
            )
        };
        let sounds = self
            .sounds
            .clone()
            .unwrap_or_else(|| state.read(cx).sounds.clone());
        players.borrow_mut().clear();

        let border = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let radius = cx.theme().radius_xl();
        let ring = cx.theme().ring;

        let on_change = &self.on_sounds_change;
        let mut any_playing = false;
        let mut pads = Vec::new();
        for sound in &sounds {
            let (element, playing) = pad_element(
                &self.id,
                sound,
                &sounds,
                &state,
                &players,
                on_change,
                &self.on_play,
                &self.on_stop,
                window,
                cx,
            );
            any_playing |= playing;
            pads.push(element);
        }
        if any_playing {
            window.request_animation_frame();
        }

        let sounds_for_drop = sounds.clone();
        let sounds_for_pick = sounds.clone();
        let sounds_for_stop = sounds.clone();
        let (state_drop, state_pick, state_stop, state_undo) =
            (state.clone(), state.clone(), state.clone(), state.clone());
        let (change_drop, change_pick, change_undo) = (
            self.on_sounds_change.clone(),
            self.on_sounds_change.clone(),
            self.on_sounds_change.clone(),
        );
        let players_stop = players.clone();
        let on_stop = self.on_stop.clone();
        let on_gain = self.on_output_gain.clone();
        let (state_volume, state_mute, state_keys) = (state.clone(), state.clone(), state.clone());
        let (gain_volume, gain_mute) = (on_gain.clone(), on_gain.clone());

        let undo = removed.clone().map(|removed| {
            let sounds_now = sounds.clone();
            Button::new(child_id(&self.id, "undo"))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, window, cx| {
                    if !sounds_now.iter().any(|item| item.id == removed.sound.id) {
                        let mut next = sounds_now.clone();
                        let index = removed.index.min(next.len());
                        next.insert(index, removed.sound.clone());
                        commit(
                            &state_undo,
                            &change_undo,
                            next,
                            None,
                            Some(None),
                            window,
                            cx,
                        );
                    } else {
                        state_undo.update(cx, |board, cx| {
                            board.removed = None;
                            cx.notify();
                        });
                    }
                    let label = removed.sound.label.clone();
                    state_undo.update(cx, |board, cx| {
                        board.feedback = format!("Restored {label}.").into();
                        cx.notify();
                    });
                })
                .child("Undo removal")
        });

        let pick = {
            let state = state_pick;
            let sounds = sounds_for_pick;
            let on_change = change_pick;
            move |window: &mut Window, cx: &mut App| {
                begin_pick(&state, sounds.clone(), on_change.clone(), window, cx);
            }
        };

        let mut alert = Alert::new().child(AlertDescription::new(feedback.clone()));
        if let Some(undo) = undo {
            alert = alert.child(div().mt(px(8.0)).child(undo));
        }

        let body = if sounds.is_empty() {
            empty_state(&self.id, border, radius, pick).into_any_element()
        } else {
            let mut grid = SoundPadGrid::new(child_id(&self.id, "pads"))
                .w_full()
                .columns(self.columns)
                .hotkeys(hotkeys)
                .hotkey_scope(HotkeyScope::Global);
            for pad in pads {
                grid = grid.child(pad);
            }
            grid = grid.child(add_pad(&self.id, muted_fg, border, radius, pick));
            div()
                .id(child_id(&self.id, "grid"))
                .relative()
                .w_full()
                .drag_over::<gpui::ExternalPaths>(move |style, _, _, _| {
                    style.shadow(vec![ring_shadow(2.0, ring)])
                })
                .child(grid)
                .into_any_element()
        };

        let mut root = div()
            .id(self.id.clone())
            .w_full()
            .font_family(cx.theme().font_sans.clone())
            .can_drop(|value, _, _| value.is::<gpui::ExternalPaths>())
            .on_drop({
                let state = state_drop;
                let sounds = sounds_for_drop;
                let on_change = change_drop;
                move |paths: &gpui::ExternalPaths, window, cx| {
                    add_paths(&state, &sounds, paths.paths(), &on_change, window, cx);
                }
            })
            .child(toolbar(
                &self.id,
                volume,
                muted,
                hotkeys,
                state_volume,
                state_mute,
                state_keys,
                gain_volume,
                gain_mute,
                state_stop,
                players_stop,
                on_stop,
                sounds_for_stop,
            ))
            .when(!feedback.is_empty(), |el| {
                el.child(div().mb(px(12.0)).child(alert))
            })
            .child(body);
        root.style().refine(&self.style);
        root
    }
}

#[allow(clippy::too_many_arguments)]
fn toolbar(
    id: &ElementId,
    volume: f32,
    muted: bool,
    hotkeys: bool,
    state_volume: Entity<BoardState>,
    state_mute: Entity<BoardState>,
    state_keys: Entity<BoardState>,
    gain_volume: Option<GainHandler>,
    gain_mute: Option<GainHandler>,
    state_stop: Entity<BoardState>,
    players: Rc<RefCell<HashMap<SharedString, Entity<SimulatedPlayer>>>>,
    on_stop: Option<StopHandler>,
    sounds: Vec<SoundboardSound>,
) -> AnyElement {
    div()
        .mb(px(12.0))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .mr_auto()
                .font_family(HEADING_FONT)
                .font_weight(gpui::FontWeight::MEDIUM)
                .child("Soundboard"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    Switch::new(child_id(id, "hotkeys"))
                        .size(SwitchSize::Sm)
                        .checked(hotkeys)
                        .on_checked_change(move |on, _, cx| {
                            state_keys.update(cx, |board, cx| {
                                board.hotkeys = on;
                                cx.notify();
                            });
                        }),
                )
                .child(Label::new("Hotkeys")),
        )
        .child(
            VolumeControl::new(child_id(id, "volume"))
                .w(px(144.0))
                .value(volume)
                .muted(muted)
                .on_value_change(move |volume, window, cx| {
                    state_volume.update(cx, |board, cx| {
                        board.volume = volume;
                        cx.notify();
                    });
                    let muted = state_volume.read(cx).muted;
                    emit_gain(&gain_volume, volume, muted, window, cx);
                })
                .on_muted_change(move |muted, window, cx| {
                    state_mute.update(cx, |board, cx| {
                        board.muted = muted;
                        cx.notify();
                    });
                    let volume = state_mute.read(cx).volume;
                    emit_gain(&gain_mute, volume, muted, window, cx);
                })
                .child(VolumeControlMute::new().child(Icon::new(IconName::SpeakerHigh)))
                .child(VolumeControlSlider::new()),
        )
        .child(
            Button::new(child_id(id, "stop"))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .icon_start(IconName::Stop)
                .on_click(move |_, window, cx| {
                    let players: Vec<_> = players.borrow().values().cloned().collect();
                    for player in players {
                        player.pause(window, cx);
                        player.seek(0.0, window, cx);
                    }
                    if let Some(handler) = &on_stop {
                        for sound in &sounds {
                            handler(&sound.id, window, cx);
                        }
                    }
                    state_stop.update(cx, |board, cx| {
                        board.feedback = "Stopped all sounds.".into();
                        cx.notify();
                    });
                })
                .child("Stop all"),
        )
        .into_any_element()
}

fn empty_state(
    id: &ElementId,
    border: gpui::Hsla,
    radius: gpui::Pixels,
    pick: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::Div {
    div()
        .rounded(radius)
        .border_1()
        .border_dashed()
        .border_color(border)
        .child(
            Empty::new()
                .child(
                    EmptyHeader::new()
                        .child(
                            EmptyMedia::new()
                                .variant(EmptyMediaVariant::Icon)
                                .icon(IconName::Waveform),
                        )
                        .child(EmptyTitle::new("No sounds yet").font_family(HEADING_FONT))
                        .child(EmptyDescription::new(
                            "Drop audio files here, or add them from your computer.",
                        )),
                )
                .child(
                    Button::new(child_id(id, "add-empty"))
                        .size(ButtonSize::Sm)
                        .variant(ButtonVariant::Outline)
                        .icon_start(IconName::Plus)
                        .on_click(move |_, window, cx| pick(window, cx))
                        .child("Add sounds"),
                ),
        )
}

fn add_pad(
    id: &ElementId,
    muted_fg: gpui::Hsla,
    border: gpui::Hsla,
    radius: gpui::Pixels,
    pick: impl Fn(&mut Window, &mut App) + 'static,
) -> SoundPad {
    // Ghost plus a dashed border: `SoundPad`'s outline border lives on an
    // inner element the root style cannot restyle. The centred stack is a
    // full-size child for the same reason (`items-center justify-center`).
    SoundPad::new(child_id(id, "add"))
        .variant(SoundPadVariant::Ghost)
        .text_color(muted_fg)
        .border_1()
        .border_dashed()
        .border_color(border)
        .rounded(radius)
        .on_trigger(pick)
        .child(
            div()
                .w_full()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .text_color(muted_fg)
                .child(Icon::new(IconName::Plus).size(px(20.0)))
                // `justify-center` does nothing here: `SoundPadLabel`'s
                // `mt-auto` takes the free space and pushes it to the bottom.
                // (Taffy would apply both, so the stack is not centred.)
                .child(
                    div()
                        .mt_auto()
                        .text_sm_()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("Add"),
                ),
        )
}

/// The pad plus its context menu. Split from [`render_pad`] so the menu
/// closures each own their copy of the sound list.
#[allow(clippy::too_many_arguments)]
fn pad_element(
    board_id: &ElementId,
    sound: &SoundboardSound,
    sounds: &[SoundboardSound],
    state: &Entity<BoardState>,
    players: &Rc<RefCell<HashMap<SharedString, Entity<SimulatedPlayer>>>>,
    on_change: &Option<SoundsHandler>,
    on_play: &Option<PlayHandler>,
    on_stop: &Option<StopHandler>,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, bool) {
    let mode = mode_of(sound);
    let (playing, loading, progress, simulated) = if let Some(voice) = &sound.voice {
        (voice.playing, voice.loading, voice.progress.clone(), false)
    } else if let Some(duration) = clip_duration(sound) {
        let mut options = UseAudioPlayerOptions::src(sound.src.clone());
        options.r#loop = mode == SoundPadMode::Loop;
        options.volume = sound.volume.unwrap_or(1.0);
        let player = use_simulated_player(
            child_id(board_id, &format!("player-{}", sound.id)),
            duration,
            options,
            window,
            cx,
        );
        players
            .borrow_mut()
            .insert(sound.id.clone(), player.clone());
        (
            player.state(cx).playing,
            false,
            Some(player.read(cx).progress_source()),
            true,
        )
    } else {
        (false, true, None, false)
    };

    let mut pad = SoundPad::new(child_id(board_id, &format!("pad-{}", sound.id)))
        .mode(mode)
        .playing(playing)
        .loading(loading);
    if let Some(hotkey) = &sound.hotkey {
        pad = pad.hotkey(hotkey.clone());
    }
    if let Some(accent) = sound.accent {
        pad = pad.accent(accent);
    }

    let sound_play = sound.clone();
    let id_stop = sound.id.clone();
    let on_play = on_play.clone();
    let on_stop_cb = on_stop.clone();
    let state_play = state.clone();
    let state_stop = state.clone();
    if simulated {
        let player = players
            .borrow()
            .get(&sound.id)
            .cloned()
            .expect("simulated pad registers its player");
        let player_stop = player.clone();
        let on_stop_for_play = on_stop_cb.clone();
        let id_for_play_stop = id_stop.clone();
        pad = pad
            .on_trigger(move |window, cx| {
                player.seek(0.0, window, cx);
                player.play(window, cx);
                if let Some(handler) = &on_play {
                    handler(&sound_play, window, cx);
                }
                state_play.update(cx, |_, cx| cx.notify());
            })
            .on_stop(move |window, cx| {
                player_stop.pause(window, cx);
                player_stop.seek(0.0, window, cx);
                if let Some(handler) = &on_stop_for_play {
                    handler(&id_for_play_stop, window, cx);
                }
                state_stop.update(cx, |_, cx| cx.notify());
            });
    } else {
        pad = pad
            .on_trigger(move |window, cx| {
                if let Some(handler) = &on_play {
                    handler(&sound_play, window, cx);
                }
                state_play.update(cx, |_, cx| cx.notify());
            })
            .on_stop(move |window, cx| {
                if let Some(handler) = &on_stop_cb {
                    handler(&id_stop, window, cx);
                }
                state_stop.update(cx, |_, cx| cx.notify());
            });
    }

    pad = pad
        .child(SoundPadLabel::new().child(sound.label.clone()))
        .child(SoundPadShortcut::new());
    pad = match progress {
        Some(source) => pad.child(SoundPadProgress::new().source(source)),
        None => pad.child(SoundPadProgress::new()),
    };

    let element = context_menu(board_id, sound, sounds, state, players, on_change, pad);
    (element, playing)
}

#[allow(clippy::too_many_arguments)]
fn context_menu(
    board_id: &ElementId,
    sound: &SoundboardSound,
    sounds: &[SoundboardSound],
    state: &Entity<BoardState>,
    players: &Rc<RefCell<HashMap<SharedString, Entity<SimulatedPlayer>>>>,
    on_change: &Option<SoundsHandler>,
    pad: SoundPad,
) -> AnyElement {
    let sounds_mode = sounds.to_vec();
    let sounds_volume = sounds.to_vec();
    let sounds_remove = sounds.to_vec();
    let id_mode = sound.id.clone();
    let id_volume = sound.id.clone();
    let id_remove = sound.id.clone();
    let label = sound.label.clone();
    let (state_mode, state_volume, state_remove) = (state.clone(), state.clone(), state.clone());
    let (change_mode, change_volume, change_remove) =
        (on_change.clone(), on_change.clone(), on_change.clone());
    let players = players.clone();
    let mode = mode_token(mode_of(sound));
    let volume = volume_token(sound.volume.unwrap_or(1.0));

    ContextMenu::new(child_id(board_id, &format!("menu-{}", sound.id)))
        .trigger(pad)
        .content(
            ContextMenuContent::new()
                .w(px(192.0))
                .child(
                    ContextMenuGroup::new()
                        .child(ContextMenuLabel::new("Mode"))
                        .child(
                            ContextMenuRadioGroup::new()
                                .value(mode)
                                .on_value_change(move |value, window, cx| {
                                    let mode = parse_mode(value.as_ref());
                                    let next = map_sound(&sounds_mode, &id_mode, |sound| {
                                        sound.mode = Some(mode)
                                    });
                                    commit(&state_mode, &change_mode, next, None, None, window, cx);
                                })
                                .children(MODES.into_iter().map(|(mode, label)| {
                                    ContextMenuRadioItem::new(mode_token(mode), label)
                                })),
                        ),
                )
                .child(ContextMenuSeparator::new())
                .child(
                    ContextMenuGroup::new()
                        .child(ContextMenuLabel::new("Volume"))
                        .child(
                            ContextMenuRadioGroup::new()
                                .value(volume)
                                .on_value_change(move |value, window, cx| {
                                    let Ok(volume) = value.parse::<f32>() else {
                                        return;
                                    };
                                    let next = map_sound(&sounds_volume, &id_volume, |sound| {
                                        sound.volume = Some(volume);
                                    });
                                    commit(
                                        &state_volume,
                                        &change_volume,
                                        next,
                                        None,
                                        None,
                                        window,
                                        cx,
                                    );
                                })
                                .children(VOLUMES.into_iter().map(|volume| {
                                    ContextMenuRadioItem::new(
                                        volume_token(volume),
                                        format!("{}%", (volume * 100.0).round() as i32),
                                    )
                                })),
                        ),
                )
                .child(ContextMenuSeparator::new())
                .child(
                    ContextMenuGroup::new().child(
                        ContextMenuItem::new("Remove")
                            .variant(ContextMenuItemVariant::Destructive)
                            .on_select(move |window, cx| {
                                release_player(&players, &id_remove, window, cx);
                                let index = sounds_remove
                                    .iter()
                                    .position(|item| item.id == id_remove)
                                    .unwrap_or(0);
                                let removed_sound = sounds_remove
                                    .iter()
                                    .find(|item| item.id == id_remove)
                                    .cloned();
                                let next = sounds_remove
                                    .iter()
                                    .filter(|item| item.id != id_remove)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                let removed =
                                    removed_sound.map(|sound| RemovedSound { sound, index });
                                commit(
                                    &state_remove,
                                    &change_remove,
                                    next,
                                    Some(
                                        format!("Removed {label}. You can undo this removal.")
                                            .into(),
                                    ),
                                    Some(removed),
                                    window,
                                    cx,
                                );
                            }),
                    ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_audio_is_added_with_hotkeys_and_duplicates_are_skipped() {
        let existing = vec![file_to_sound(Path::new("held.wav"), 0)];
        let added = collect_added(
            &existing,
            &[
                PathBuf::from("notes.txt"),
                PathBuf::from("held.wav"),
                PathBuf::from("clap.wav"),
                PathBuf::from("roll.mp3"),
            ],
        );
        assert_eq!(added.len(), 2);
        assert_eq!(added[0].label.as_ref(), "clap");
        assert_eq!(added[0].hotkey.as_deref(), Some("2"));
        assert_eq!(added[1].hotkey.as_deref(), Some("3"));
        assert!((added[0].duration.unwrap() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn the_tenth_pad_has_no_hotkey() {
        let existing: Vec<_> = (0..9)
            .map(|index| {
                SoundboardSound::new(
                    format!("s{index}"),
                    format!("S{index}"),
                    format!("s{index}"),
                )
            })
            .collect();
        let added = collect_added(&existing, &[PathBuf::from("extra.flac")]);
        assert!(added[0].hotkey.is_none());
    }

    #[test]
    fn empty_drop_feedback_matches_react() {
        assert_eq!(
            added_feedback(0).as_ref(),
            "No new sounds added. Choose audio files that aren't already on the board."
        );
        assert_eq!(added_feedback(1).as_ref(), "Added 1 sound.");
        assert_eq!(added_feedback(2).as_ref(), "Added 2 sounds.");
    }

    #[test]
    fn volume_tokens_match_javascript_string() {
        assert_eq!(volume_token(1.0).as_ref(), "1");
        assert_eq!(volume_token(0.75).as_ref(), "0.75");
        assert_eq!(volume_token(0.5).as_ref(), "0.5");
        assert_eq!(volume_token(0.25).as_ref(), "0.25");
    }

    #[test]
    fn bus_gain_is_squared_and_mute_is_silent() {
        assert!((bus_gain(0.5, false) - 0.25).abs() < f32::EPSILON);
        assert_eq!(bus_gain(0.5, true), 0.0);
    }
}
