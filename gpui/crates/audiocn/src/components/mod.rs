//! The audiocn components, one module per `components/ui/*.tsx` file.
mod db_scale;
pub use db_scale::*;
mod clip_indicator;
pub use clip_indicator::*;
mod db_readout;
pub use db_readout::*;
mod level_meter;
pub use level_meter::*;
mod lcd_display;
pub use lcd_display::*;
mod lcd_meter;
pub use lcd_meter::*;
mod knob;
pub use knob::*;
mod pan_control;
pub use pan_control::*;
mod fader;
pub use fader::*;
mod parameter_slider;
pub use parameter_slider::*;
mod volume_control;
pub use volume_control::*;
mod waveform;
pub use waveform::*;
mod live_waveform;
pub use live_waveform::*;
mod smooth_waveform;
pub use smooth_waveform::*;
mod electric_waveform;
pub use electric_waveform::*;
mod electric_bar_visualizer;
pub use electric_bar_visualizer::*;
mod bar_visualizer;
pub use bar_visualizer::*;
mod spectrum;
pub use spectrum::*;
mod audio_player;
pub use audio_player::*;
mod track_list;
pub use track_list::*;
mod sound_pad;
pub use sound_pad::*;
mod tempo_control;
pub use tempo_control::*;
mod audio_device_select;
pub use audio_device_select::*;
mod channel_toggle;
pub use channel_toggle::*;
mod channel_strip;
pub use channel_strip::*;
mod mixer;
pub use mixer::*;

/// Registers key bindings for every component. Each component that needs
/// bindings adds one line here, calling its own `init`.
pub(crate) fn init(_cx: &mut gpui::App) {
    // e.g. knob::init(cx);
}
