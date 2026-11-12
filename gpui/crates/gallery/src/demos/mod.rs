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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Components,
    Blocks,
    Concepts,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Self::Components => "Components",
            Self::Blocks => "Blocks",
            Self::Concepts => "Concepts",
        }
    }
}

pub struct Demo {
    /// The docs slug, e.g. `level-meter`.
    pub slug: &'static str,
    pub title: &'static str,
    /// The docs page description.
    pub description: &'static str,
    pub section: Section,
    /// Variant names; the first is the hero preview and is always `default`.
    pub variants: &'static [&'static str],
    pub render: fn(&str, &mut Window, &mut App) -> Option<AnyElement>,
}

/// Element id for one demo instance.
pub fn id(slug: &str, variant: &str) -> SharedString {
    format!("{slug}/{variant}").into()
}

pub const ALL: &[Demo] = &[
    level_meter::DEMO,
    db_scale::DEMO,
    clip_indicator::DEMO,
    db_readout::DEMO,
    lcd_display::DEMO,
    lcd_meter::DEMO,
    knob::DEMO,
    pan_control::DEMO,
    fader::DEMO,
    parameter_slider::DEMO,
    volume_control::DEMO,
    waveform::DEMO,
    live_waveform::DEMO,
    smooth_waveform::DEMO,
    electric_waveform::DEMO,
    electric_bar_visualizer::DEMO,
    bar_visualizer::DEMO,
    spectrum::DEMO,
    audio_player::DEMO,
    track_list::DEMO,
    sound_pad::DEMO,
    tempo_control::DEMO,
    audio_device_select::DEMO,
    channel_toggle::DEMO,
    channel_strip::DEMO,
    mixer::DEMO,
    mic_setup::DEMO,
    music_player::DEMO,
    quick_audio_popover::DEMO,
    soundboard::DEMO,
    system_audio_mixer::DEMO,
    system_audio_settings::DEMO,
    frame_source::DEMO,
];

pub fn find(slug: &str) -> Option<&'static Demo> {
    ALL.iter().find(|demo| demo.slug == slug)
}
