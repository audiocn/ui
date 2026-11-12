//! One file per docs page. Each exposes a `DEMO` describing its variants; a
//! variant is one example from the docs page (`components/examples/<slug>-<variant>.tsx`,
//! with `<slug>-demo.tsx` as `default`, the hero preview).

use gpui::{AnyElement, App, SharedString, Window};

mod audio_device_select;
mod audio_player;
mod bar_visualizer;
mod channel_strip;
mod channel_toggle;
mod clip_indicator;
mod db_readout;
mod db_scale;
mod electric_bar_visualizer;
mod electric_waveform;
mod fader;
mod frame_source;
mod knob;
mod lcd_display;
mod lcd_meter;
mod level_meter;
mod live_waveform;
mod mic_setup;
mod mixer;
mod music_player;
mod pan_control;
mod parameter_slider;
mod quick_audio_popover;
mod smooth_waveform;
mod sound_pad;
mod soundboard;
mod spectrum;
mod system_audio_mixer;
mod system_audio_settings;
mod tempo_control;
mod track_list;
mod volume_control;
mod waveform;

pub struct Demo {
    /// The docs slug, e.g. `level-meter`.
    pub slug: &'static str,
    pub title: &'static str,
    /// The docs page description.
    pub description: &'static str,
    /// Variant names; the first is the hero preview and is always `default`.
    pub variants: &'static [&'static str],
    pub render: fn(&str, &mut Window, &mut App) -> Option<AnyElement>,
}

/// Element id for one demo instance.
pub fn id(slug: &str, variant: &str) -> SharedString {
    format!("{slug}/{variant}").into()
}

/// Every page, in the docs sidebar order.
pub const ALL: &[Demo] = &[
    frame_source::DEMO,
    level_meter::DEMO,
    lcd_meter::DEMO,
    db_scale::DEMO,
    db_readout::DEMO,
    clip_indicator::DEMO,
    bar_visualizer::DEMO,
    electric_bar_visualizer::DEMO,
    electric_waveform::DEMO,
    smooth_waveform::DEMO,
    live_waveform::DEMO,
    waveform::DEMO,
    spectrum::DEMO,
    fader::DEMO,
    parameter_slider::DEMO,
    tempo_control::DEMO,
    knob::DEMO,
    lcd_display::DEMO,
    pan_control::DEMO,
    channel_toggle::DEMO,
    volume_control::DEMO,
    audio_device_select::DEMO,
    channel_strip::DEMO,
    mixer::DEMO,
    audio_player::DEMO,
    track_list::DEMO,
    sound_pad::DEMO,
    system_audio_mixer::DEMO,
    mic_setup::DEMO,
    system_audio_settings::DEMO,
    quick_audio_popover::DEMO,
    soundboard::DEMO,
    music_player::DEMO,
];

/// The docs sidebar group a page belongs to (`content/docs/**/meta.json`).
pub fn group(slug: &str) -> &'static str {
    match slug {
        "frame-source" => "Concepts",
        "level-meter"
        | "lcd-meter"
        | "db-scale"
        | "db-readout"
        | "clip-indicator"
        | "bar-visualizer"
        | "electric-bar-visualizer"
        | "electric-waveform"
        | "smooth-waveform"
        | "live-waveform"
        | "waveform"
        | "spectrum" => "Meters and visualizers",
        "fader"
        | "parameter-slider"
        | "tempo-control"
        | "knob"
        | "lcd-display"
        | "pan-control"
        | "channel-toggle"
        | "volume-control"
        | "audio-device-select" => "Controls",
        "channel-strip" | "mixer" => "Mixer",
        "audio-player" | "track-list" | "sound-pad" => "Sounds and music",
        "system-audio-mixer"
        | "mic-setup"
        | "system-audio-settings"
        | "quick-audio-popover"
        | "soundboard"
        | "music-player" => "Blocks",
        _ => "Components",
    }
}

pub fn find(slug: &str) -> Option<&'static Demo> {
    ALL.iter().find(|demo| demo.slug == slug)
}
