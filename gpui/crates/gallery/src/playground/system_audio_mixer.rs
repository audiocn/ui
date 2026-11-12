//! Playground page for `SystemAudioMixer`: layout and strips, track / sound
//! data sets, a page-owned `MixerStore` whose gains, mutes, solos and
//! monitors sync with the panel both ways (and are logged), the meter seam,
//! devices and permission, the microphone / system / music / pad transports
//! (block-owned or host-driven), and every callback.

use audiocn::motion::Instant;
use audiocn::signal::mixer::{MixerOptions, MixerStore, use_mixer};
use audiocn::theme::oklch;
use audiocn::{
    FrameEmitter, MixerActivity, MixerSound, MixerSoundPlayback, MixerSourceId, MixerTrack,
    Orientation, SystemAudioMixer, system_audio_mixer_channels,
};
use gpui::{AnyElement, App, Entity, IntoElement, SharedString, Styled, Window, px};

use super::mic_setup::{
    DEVICE_IDS, DEVICE_SETS, MIC_ACQUIRING, MIC_ACTIVE, MIC_IDLE, MIC_STATUSES, PERMISSIONS,
    REAL_AUDIO, device_id, device_index, device_set, devices_detail, later, mic_status, permission,
    set_choice,
};
use super::music_player::{TRACK_SETS, songs};
use super::soundboard::{SOUND_SETS, clips};
use super::system_audio_settings::{
    SYSTEM_ACTIVE, SYSTEM_IDLE, SYSTEM_PROMPTING, SYSTEM_STATUSES, system_status,
};
use super::{Control, Ctx, Props, SourceUse, Spec, Value};

pub const SPEC: Spec = Spec {
    slug: "system-audio-mixer",
    component: "SystemAudioMixer",
    source: SourceUse::Parts,
    controls,
    render,
};

/// The four channels in strip order, with their panel key prefix and title.
const CHANNELS: [(&str, &str); 4] = [
    ("microphone", "mic"),
    ("system", "system"),
    ("music", "music"),
    ("sounds", "sounds"),
];

const SOURCES: &[(&str, &str)] = &[
    ("all four (unset)", ""),
    (
        "microphone + system",
        "[MixerSourceId::Microphone, MixerSourceId::System]",
    ),
    (
        "music + sounds",
        "[MixerSourceId::Music, MixerSourceId::Sounds]",
    ),
    ("microphone only", "[MixerSourceId::Microphone]"),
    ("none (empty state)", "[]"),
];

const METERS: &[(&str, &str)] = &[
    ("block demo signals", ""),
    (
        "playground: every strip + master",
        ".meter(MixerSourceId::Microphone, meter.clone())\n    .meter(MixerSourceId::System, meter.clone())\n    .meter(MixerSourceId::Music, meter.clone())\n    .meter(MixerSourceId::Sounds, meter.clone())\n    .master_meter(meter)",
    ),
    (
        "playground: microphone only",
        ".meter(MixerSourceId::Microphone, meter)",
    ),
    ("playground: master only", ".master_meter(meter)"),
];

const PLAYBACK: &[(&str, &str)] = &[
    ("unset (simulated pads)", ""),
    ("host-driven (page plays them)", "playback"),
    ("host: every pad loading", "playback"),
];

/// Panel keys per channel: gain, muted, solo, monitor.
fn keys(prefix: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match prefix {
        "mic" => ("mic_gain", "mic_muted", "mic_solo", "mic_monitor"),
        "system" => (
            "system_gain",
            "system_muted",
            "system_solo",
            "system_monitor",
        ),
        "music" => ("music_gain", "music_muted", "music_solo", "music_monitor"),
        _ => (
            "sounds_gain",
            "sounds_muted",
            "sounds_solo",
            "sounds_monitor",
        ),
    }
}

fn channel_controls(
    prefix: &'static str,
    group: &'static str,
    gain: f32,
    monitor: bool,
) -> [Control; 4] {
    let (g, m, s, mon) = keys(prefix);
    [
        Control::num(g, "Gain (dB)", gain, -60.0, 12.0, 0.5)
            .group(group)
            .no_code(),
        Control::bool(m, "Muted", false).group(group).no_code(),
        Control::bool(s, "Solo", false).group(group).no_code(),
        Control::bool(mon, "Monitor", monitor)
            .group(group)
            .no_code(),
    ]
}

fn controls() -> Vec<Control> {
    let mut controls = vec![
        // Layout
        Control::choice(
            "default_orientation",
            "Default orientation",
            0,
            &[
                ("rows (horizontal)", "Orientation::Horizontal"),
                ("console (vertical)", "Orientation::Vertical"),
            ],
        )
        .group("Layout")
        .remount(),
        Control::choice("sources", "Sources", 0, SOURCES).group("Layout"),
        Control::px("height", "Height (px, 0 = auto)", 0.0, 0.0, 900.0, 10.0)
            .group("Layout")
            .method("h")
            .help("The console example uses 480."),
        // Data
        Control::choice("tracks", "Tracks", 0, TRACK_SETS).group("Data"),
        Control::choice("sounds", "Sounds", 0, SOUND_SETS).group("Data"),
        Control::text("persist_key", "Persist key", "")
            .group("Data")
            .help("Accepted for parity; the block does not write it."),
        // Meters
        Control::choice("meters", "Meters", 0, METERS)
            .group("Meters")
            .code("{}")
            .help("Playground: .meter(..) / .master_meter(..) from the Source panel."),
        // Mixer
        Control::bool("own_mixer", "Page-owned mixer", true)
            .group("Mixer")
            .no_code()
            .remount()
            .help("On: .mixer(store), synced with the sliders below and logged."),
        Control::num("master_gain", "Master gain (dB)", 0.0, -60.0, 6.0, 0.5)
            .group("Mixer")
            .no_code(),
        Control::bool("master_muted", "Master muted", false)
            .group("Mixer")
            .no_code(),
    ];
    controls.extend(channel_controls("mic", "Mixer: microphone", 0.0, false));
    controls.extend(channel_controls("system", "Mixer: system", -6.0, false));
    controls.extend(channel_controls("music", "Mixer: music", -12.0, true));
    controls.extend(channel_controls("sounds", "Mixer: sounds", -6.0, true));
    controls.extend([
        // Devices
        Control::choice("devices", "Devices", 1, DEVICE_SETS).group("Devices"),
        Control::bool("devices_loading", "Loading", false).group("Devices"),
        Control::choice("permission", "Permission", 1, PERMISSIONS).group("Devices"),
        Control::bool("device_controlled", "Controlled device", false)
            .group("Devices")
            .no_code()
            .remount(),
        Control::choice("device_id", "Device id", 0, DEVICE_IDS).group("Devices"),
        // Transport
        Control::bool("simulate_host", "Simulate host", true)
            .group("Transport")
            .no_code()
            .help("Controlled statuses / music follow their callbacks like a real host."),
        Control::choice("microphone_status", "Microphone status", 0, MIC_STATUSES)
            .group("Transport"),
        Control::choice("system_status", "System status", 0, SYSTEM_STATUSES).group("Transport"),
        Control::bool("system_supported", "System supported", true).group("Transport"),
        Control::bool("music_controlled", "Controlled music", false)
            .group("Transport")
            .no_code()
            .remount()
            .help("On: music_playing / track_index; the page is the player."),
        Control::bool("music_playing", "Music playing", false).group("Transport"),
        Control::num("track_index", "Track index", 0.0, 0.0, 7.0, 1.0).group("Transport"),
        Control::choice("sound_playback", "Sound playback", 0, PLAYBACK).group("Transport"),
    ]);
    if REAL_AUDIO {
        controls.insert(
            0,
            Control::bool("real_audio", "Real audio", false)
                .group("Layout")
                .no_code()
                .remount()
                .help("Real microphone + system capture on those strips (audiocn::io)."),
        );
    }
    controls
}

fn tracks(label: &str) -> Vec<MixerTrack> {
    songs(label)
        .into_iter()
        .map(|(id, title, artist, seconds)| {
            MixerTrack::new(id, title)
                .artist(artist)
                .src(id)
                .duration(seconds)
        })
        .collect()
}

fn sounds(label: &str) -> Vec<MixerSound> {
    clips(label)
        .into_iter()
        .map(|(id, label, hotkey, (l, c, h), seconds)| {
            MixerSound::new(id, label)
                .src(id)
                .hotkey(hotkey)
                .accent(oklch(l, c, h))
                .duration(seconds)
        })
        .collect()
}

// ----- mixer sync -------------------------------------------------------------

/// gain, muted, solo, monitor per channel, then master gain and mute.
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    channels: [(f32, bool, bool, bool); 4],
    master: (f32, bool),
}

fn panel_snapshot(p: &Props) -> Snapshot {
    let mut channels = [(0.0, false, false, false); 4];
    for (slot, (_, prefix)) in channels.iter_mut().zip(CHANNELS) {
        let (g, m, s, mon) = keys(prefix);
        *slot = (p.num(g), p.bool(m), p.bool(s), p.bool(mon));
    }
    Snapshot {
        channels,
        master: (p.num("master_gain"), p.bool("master_muted")),
    }
}

fn store_snapshot(store: &MixerStore) -> Snapshot {
    let mut channels = [(0.0, false, false, false); 4];
    for (slot, (id, _)) in channels.iter_mut().zip(CHANNELS) {
        if let Some(c) = store.channel(id) {
            *slot = (c.gain_db, c.muted, c.solo, c.monitor);
        }
    }
    Snapshot {
        channels,
        master: (store.master.gain_db, store.master.muted),
    }
}

fn apply_to_store(store: &mut MixerStore, snap: &Snapshot) {
    for ((id, _), (gain, muted, solo, monitor)) in CHANNELS.iter().zip(snap.channels) {
        store.set_gain(id, gain);
        store.set_muted(id, muted);
        store.set_solo(id, solo, false);
        store.set_monitor(id, monitor);
    }
    store.set_master_gain(snap.master.0);
    store.set_master_muted(snap.master.1);
}

/// What changed between two snapshots, as log entries.
fn diff(old: &Snapshot, new: &Snapshot) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ((id, _), (a, b)) in CHANNELS.iter().zip(old.channels.iter().zip(&new.channels)) {
        if a.0 != b.0 {
            out.push((format!("{id}: gain"), format!("{:.1} dB", b.0)));
        }
        if a.1 != b.1 {
            out.push((format!("{id}: muted"), b.1.to_string()));
        }
        if a.2 != b.2 {
            out.push((format!("{id}: solo"), b.2.to_string()));
        }
        if a.3 != b.3 {
            out.push((format!("{id}: monitor"), b.3.to_string()));
        }
    }
    if old.master.0 != new.master.0 {
        out.push(("master: gain".into(), format!("{:.1} dB", new.master.0)));
    }
    if old.master.1 != new.master.1 {
        out.push(("master: muted".into(), new.master.1.to_string()));
    }
    out
}

fn write_panel(ctx: &Ctx, snap: &Snapshot, cx: &mut App) {
    for ((_, prefix), (gain, muted, solo, monitor)) in CHANNELS.iter().zip(snap.channels) {
        let (g, m, s, mon) = keys(prefix);
        ctx.set(g, Value::Num(gain), cx);
        ctx.set(m, Value::Bool(muted), cx);
        ctx.set(s, Value::Bool(solo), cx);
        ctx.set(mon, Value::Bool(monitor), cx);
    }
    ctx.set_num("master_gain", snap.master.0, cx);
    ctx.set_bool("master_muted", snap.master.1, cx);
}

struct Synced {
    panel: Snapshot,
    store: Snapshot,
}

/// Two-way sync: a panel edit is pushed into the store; a strip edit (the
/// store moved on its own) is written back to the panel and logged.
fn sync_mixer(p: &Props, ctx: &Ctx, mixer: &Entity<MixerStore>, window: &mut Window, cx: &mut App) {
    let synced = window.use_keyed_state(ctx.id("mixer-sync"), cx, |_, _| None::<Synced>);
    let panel = panel_snapshot(p);
    let store = store_snapshot(mixer.read(cx));
    let previous = synced.update(cx, |s, _| s.take());
    match previous {
        Some(prev) if prev.panel != panel => {
            mixer.update(cx, |m, cx| {
                apply_to_store(m, &panel);
                cx.notify();
            });
            let store = store_snapshot(mixer.read(cx));
            synced.update(cx, |s, _| *s = Some(Synced { panel, store }));
        }
        Some(prev) if prev.store != store => {
            let changes = diff(&prev.store, &store);
            let ctx = ctx.clone();
            let snap = store.clone();
            window.defer(cx, move |_, cx| {
                write_panel(&ctx, &snap, cx);
                for (event, detail) in changes {
                    ctx.log(format!("mixer {event}"), detail, cx);
                }
            });
            synced.update(cx, |s, _| {
                *s = Some(Synced {
                    panel: store.clone(),
                    store,
                })
            });
        }
        Some(prev) => synced.update(cx, |s, _| *s = Some(prev)),
        None => {
            mixer.update(cx, |m, _| apply_to_store(m, &panel));
            let store = store_snapshot(mixer.read(cx));
            synced.update(cx, |s, _| *s = Some(Synced { panel, store }));
        }
    }
}

// ----- host-driven pads -------------------------------------------------------

struct Pads {
    playing: Option<(SharedString, Instant, f64)>,
    progress: FrameEmitter<f32>,
}

fn pad_playback(
    p: &Props,
    ctx: &Ctx,
    sounds: &[MixerSound],
    window: &mut Window,
    cx: &mut App,
) -> (Entity<Pads>, Vec<(SharedString, MixerSoundPlayback)>) {
    let pads = window.use_keyed_state(ctx.id("pads"), cx, |_, _| Pads {
        playing: None,
        progress: FrameEmitter::new(),
    });
    let loading = p.choice("sound_playback") == "host: every pad loading";
    let (playing, progress) = {
        let state = pads.read(cx);
        (state.playing.clone(), state.progress.clone())
    };
    let playing_id = match playing {
        Some((id, started, seconds)) => {
            let t = started.elapsed().as_secs_f64();
            if t >= seconds {
                pads.update(cx, |s, _| s.playing = None);
                progress.emit(0.0);
                None
            } else {
                progress.emit((t / seconds) as f32);
                window.request_animation_frame();
                Some(id)
            }
        }
        None => None,
    };
    let list = sounds
        .iter()
        .map(|sound| {
            let on = playing_id.as_ref() == Some(&sound.id);
            (
                sound.id.clone(),
                MixerSoundPlayback {
                    playing: on,
                    loading,
                    progress: on.then(|| progress.source()),
                },
            )
        })
        .collect();
    (pads, list)
}

// ----- render -----------------------------------------------------------------

fn activity_detail(a: &MixerActivity) -> String {
    format!(
        "mic {} · system {} · device {} · music {} · sounds {}",
        a.microphone_open,
        a.system_open,
        devices_detail(&a.device_id),
        a.music_src.as_deref().unwrap_or("none"),
        a.sounds_playing
    )
}

fn render(p: &Props, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let track_list = tracks(p.choice("tracks"));
    let sound_list = sounds(p.choice("sounds"));
    let simulate = p.bool("simulate_host");
    let real = REAL_AUDIO && p.bool("real_audio");

    let mut block = SystemAudioMixer::new(ctx.id("mixer"))
        .default_orientation(if p.index("default_orientation") == 1 {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        })
        .tracks(track_list.clone())
        .sounds(sound_list.clone())
        .devices(device_set(p.choice("devices")))
        .devices_loading(p.bool("devices_loading"))
        .permission(permission(p.choice("permission")))
        .system_supported(p.bool("system_supported"));
    block = match p.choice("sources") {
        "microphone + system" => block.sources([MixerSourceId::Microphone, MixerSourceId::System]),
        "music + sounds" => block.sources([MixerSourceId::Music, MixerSourceId::Sounds]),
        "microphone only" => block.sources([MixerSourceId::Microphone]),
        "none (empty state)" => block.sources([]),
        _ => block,
    };
    let persist = p.text("persist_key");
    if !persist.is_empty() {
        block = block.persist_key(persist);
    }

    // Mixer state.
    let mixer = (p.bool("own_mixer") || real).then(|| {
        use_mixer(
            ctx.id("store"),
            MixerOptions {
                channels: system_audio_mixer_channels(),
                ..Default::default()
            },
            window,
            cx,
        )
    });
    if let Some(mixer) = &mixer {
        if p.bool("own_mixer") {
            sync_mixer(p, ctx, mixer, window, cx);
        }
        block = block.mixer(mixer.clone());
    }

    // Meters.
    if let Some(meter) = ctx.meter() {
        block = match p.choice("meters") {
            "playground: every strip + master" => {
                let mut b = block;
                for id in MixerSourceId::ALL {
                    b = b.meter(id, meter.clone());
                }
                b.master_meter(meter)
            }
            "playground: microphone only" => block.meter(MixerSourceId::Microphone, meter),
            "playground: master only" => block.master_meter(meter),
            _ => block,
        };
    }

    // Devices.
    let device_controlled = p.bool("device_controlled");
    if device_controlled {
        block = block.device_id(device_id(p.choice("device_id")));
    }
    let (c_device, c_perm) = (ctx.clone(), ctx.clone());
    block = block
        .on_device_change(move |id, _, cx| {
            if device_controlled {
                set_choice(&c_device, "device_id", device_index(&id), cx);
            }
            c_device.log("on_device_change", devices_detail(&id), cx);
        })
        .on_request_permission(move |_, cx| {
            c_perm.log("on_request_permission", "-> granted", cx);
            set_choice(&c_perm, "permission", 0, cx);
        });

    // Microphone.
    let mic = mic_status(p.choice("microphone_status"));
    if let Some(status) = mic {
        block = block.microphone_status(status);
    }
    let sim_mic = simulate && mic.is_some();
    let (c_start, c_stop) = (ctx.clone(), ctx.clone());
    block = block
        .on_microphone_start(move |_, cx| {
            c_start.log("on_microphone_start", "open the microphone", cx);
            if sim_mic {
                set_choice(&c_start, "microphone_status", MIC_ACQUIRING, cx);
                let c = c_start.clone();
                later(cx, 700, move |cx| {
                    set_choice(&c, "microphone_status", MIC_ACTIVE, cx)
                });
            }
        })
        .on_microphone_stop(move |_, cx| {
            c_stop.log("on_microphone_stop", "close the microphone", cx);
            if sim_mic {
                set_choice(&c_stop, "microphone_status", MIC_IDLE, cx);
            }
        });

    // System audio.
    let system = system_status(p.choice("system_status"));
    if let Some(status) = system {
        block = block.system_status(status);
    }
    let sim_system = simulate && system.is_some();
    let c_system = ctx.clone();
    block = block.on_system_change(move |on, _, cx| {
        c_system.log("on_system_change", on.to_string(), cx);
        if sim_system {
            if on {
                set_choice(&c_system, "system_status", SYSTEM_PROMPTING, cx);
                let c = c_system.clone();
                later(cx, 900, move |cx| {
                    set_choice(&c, "system_status", SYSTEM_ACTIVE, cx)
                });
            } else {
                set_choice(&c_system, "system_status", SYSTEM_IDLE, cx);
            }
        }
    });

    // Music.
    let music_controlled = p.bool("music_controlled");
    let track_count = track_list.len();
    if music_controlled {
        block = block
            .music_playing(p.bool("music_playing"))
            .track_index(p.num("track_index") as usize);
    }
    let (c_toggle, c_next) = (ctx.clone(), ctx.clone());
    let playing_now = p.bool("music_playing");
    let index_now = p.num("track_index") as usize;
    block = block
        .on_music_toggle(move |_, cx| {
            c_toggle.log("on_music_toggle", "", cx);
            if music_controlled && simulate {
                c_toggle.set_bool("music_playing", !playing_now, cx);
            }
        })
        .on_next_track(move |_, cx| {
            let next = if track_count == 0 {
                0
            } else {
                (index_now + 1) % track_count
            };
            c_next.log("on_next_track", format!("-> {next}"), cx);
            if music_controlled && simulate {
                c_next.set_num("track_index", next as f32, cx);
            }
        });

    // Pads.
    let c_trigger = ctx.clone();
    if p.index("sound_playback") > 0 {
        let (pads, playback) = pad_playback(p, ctx, &sound_list, window, cx);
        let durations: Vec<(SharedString, f64)> = sound_list
            .iter()
            .map(|s| (s.id.clone(), s.duration))
            .collect();
        block = block
            .sound_playback(playback)
            .on_sound_trigger(move |id, _, cx| {
                c_trigger.log("on_sound_trigger", format!("{id} (host plays it)"), cx);
                let seconds = durations
                    .iter()
                    .find(|(sid, _)| *sid == id)
                    .map_or(1.0, |(_, d)| *d);
                pads.update(cx, |s, cx| {
                    s.playing = Some((id, Instant::now(), seconds));
                    cx.notify();
                });
            });
    } else {
        block = block.on_sound_trigger(move |id, _, cx| {
            c_trigger.log("on_sound_trigger", id.to_string(), cx)
        });
    }

    let c_output = ctx.clone();
    block = block.on_output_change(move |activity, _, cx| {
        c_output.log("on_output_change", activity_detail(activity), cx);
    });

    if real && let Some(mixer) = mixer {
        block = real::wire(block, p, ctx, mixer, window, cx);
    }

    let mut el = block.w_full();
    if p.num("height") > 0.0 {
        el = el.h(px(p.num("height")));
    }
    el.into_any_element()
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
mod real {
    use audiocn::SystemAudioMixer;
    use audiocn::signal::mixer::MixerStore;
    use gpui::{App, Entity, Window};

    use super::super::{Ctx, Props};

    pub fn wire(
        block: SystemAudioMixer,
        _p: &Props,
        _ctx: &Ctx,
        _mixer: Entity<MixerStore>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> SystemAudioMixer {
        block
    }
}

#[cfg(all(feature = "device", not(target_family = "wasm")))]
mod real {
    use audiocn::io::system_audio::is_supported;
    use audiocn::io::{
        AnalyserChannels, AnalyserTapOptions, MicrophoneHandle, MicrophoneOptions,
        SystemAudioOptions, use_audio_devices, use_microphone, use_system_audio,
    };
    use audiocn::signal::mixer::MixerStore;
    use audiocn::{AudioDeviceKind, MixerSourceId, SystemAudioMixer};
    use gpui::{App, Entity, SharedString, Window};

    use super::super::{Ctx, Props};
    use super::{device_id, device_index, devices_detail, set_choice};

    struct Seen {
        device_id: Option<SharedString>,
        system_supported: bool,
    }

    /// The strip's fader and mute on the input, so the meter reads after them.
    fn follow_strip(handle: Option<MicrophoneHandle>, mixer: &MixerStore, channel: &str) {
        let (Some(handle), Some(strip)) = (handle, mixer.channel(channel)) else {
            return;
        };
        handle.set_gain_db(strip.gain_db);
        handle.set_muted(!mixer.is_audible(channel));
    }

    /// `useAudioDevices`, `useMicrophone({ deviceId })` and `useSystemAudio`
    /// on the microphone and system strips, like the gallery demo. Every
    /// callback still logs.
    pub fn wire(
        block: SystemAudioMixer,
        p: &Props,
        ctx: &Ctx,
        mixer: Entity<MixerStore>,
        window: &mut Window,
        cx: &mut App,
    ) -> SystemAudioMixer {
        let seen = window.use_keyed_state(ctx.id("real-seen"), cx, |_, _| Seen {
            device_id: None,
            system_supported: is_supported(),
        });
        let controlled = p.bool("device_controlled");
        let (device, system_supported) = {
            let s = seen.read(cx);
            (
                if controlled {
                    device_id(p.choice("device_id"))
                } else {
                    s.device_id.clone()
                },
                s.system_supported,
            )
        };
        let devices = use_audio_devices(
            ctx.id("real-devices"),
            AudioDeviceKind::AudioInput,
            window,
            cx,
        );
        let stereo = AnalyserTapOptions {
            channels: AnalyserChannels::Stereo,
            ..Default::default()
        };
        let microphone = use_microphone(
            ctx.id("real-microphone"),
            MicrophoneOptions {
                device_id: device.as_ref().map(ToString::to_string),
                analyser: stereo,
                ..Default::default()
            },
            window,
            cx,
        );
        let system = use_system_audio(
            ctx.id("real-system"),
            SystemAudioOptions {
                analyser: stereo,
                ..Default::default()
            },
            window,
            cx,
        );
        {
            let store = mixer.read(cx);
            follow_strip(microphone.handle.clone(), store, "microphone");
            follow_strip(system.stream(), store, "system");
        }
        let refresh = devices.clone();
        let (start, stop) = (microphone.clone(), microphone.clone());
        let capture = system.clone();
        let (c_perm, c_device, c_start, c_stop, c_system) = (
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
            ctx.clone(),
        );
        block
            .devices(devices.devices.clone())
            .devices_loading(devices.is_loading)
            .permission(devices.permission)
            .on_request_permission(move |_, cx| {
                let granted = refresh.request_permission();
                c_perm.log("on_request_permission", format!("real: {granted}"), cx);
            })
            .device_id(device)
            .on_device_change(move |id, _, cx| {
                if controlled {
                    set_choice(&c_device, "device_id", device_index(&id), cx);
                }
                c_device.log("on_device_change", devices_detail(&id), cx);
                seen.update(cx, |s, cx| {
                    s.device_id = id;
                    cx.notify();
                });
            })
            .microphone_status(microphone.status.into())
            .on_microphone_start(move |_, cx| {
                c_start.log("on_microphone_start", "real", cx);
                start.start(cx);
            })
            .on_microphone_stop(move |_, cx| {
                c_stop.log("on_microphone_stop", "real", cx);
                stop.stop(cx);
            })
            .meter(MixerSourceId::Microphone, microphone.meter.clone())
            .system_supported(system_supported)
            .system_status(system.status().into())
            .on_system_change(move |on, window, cx| {
                c_system.log("on_system_change", format!("real: {on}"), cx);
                if on {
                    capture.start();
                } else {
                    capture.stop();
                }
                window.refresh();
            })
            .meter(MixerSourceId::System, system.meter_source())
    }
}
