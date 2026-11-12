//! Native audio I/O (enabled with the `device` feature): the GPUI counterpart of the
//! Web Audio hooks (`use-microphone`, `use-audio-analyser`, `use-audio-devices`, ...).
pub mod analyser;
pub mod devices;
pub mod gain;
pub mod microphone;
pub mod player;
pub mod sound;
pub mod system_audio;
pub mod web_audio_mixer;
