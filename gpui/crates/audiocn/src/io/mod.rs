//! Native audio I/O (enabled with the `device` feature): the GPUI counterpart of the
//! Web Audio hooks (`use-microphone`, `use-audio-analyser`, `use-audio-devices`, ...).
//!
//! Built on [`cpal`] (on Linux: the PulseAudio protocol, which PipeWire speaks
//! through `pipewire-pulse`, falling back to ALSA), [`rustfft`] for the
//! analyser and [`symphonia`] for decoding files (wav, mp3, ogg/vorbis, flac).
//!
//! | React hook            | Here                                                    |
//! |-----------------------|---------------------------------------------------------|
//! | `useAudioContext`     | [`context::AudioOutput::shared`], [`context::Bus`]       |
//! | `useAudioAnalyser`    | [`analyser::AnalyserTap`] (and every handle's sources)   |
//! | `useMicrophone`       | [`microphone::Microphone`], [`microphone::use_microphone`] |
//! | `useAudioDevices`     | [`devices::AudioDevices`], [`devices::use_audio_devices`] |
//! | `useAudioPlayer`      | [`player::AudioPlayerEngine`], [`player::use_audio_player`] |
//! | `useSound`            | [`sound::Sound`], [`sound::use_sound`]                   |
//! | `useGainNode`         | [`gain::GainNode`]                                       |
//! | `useWebAudioMixer`    | [`web_audio_mixer::WebAudioMixer`]                       |
//! | `useSystemAudio`      | [`system_audio::SystemAudio`]                            |
//!
//! **Threads.** Audio callbacks run on their own threads and never touch GPUI.
//! Meters and visualizers get their frames through
//! [`FrameEmitter`](crate::audio::frame_source::FrameEmitter)s that a shared
//! analysis thread fills about 60 times a second; components read the
//! [`FrameSource`](crate::audio::frame_source::FrameSource)s while painting.
//! Everything inside the engine is stereo `f32` at the output device's rate.
pub mod analyser;
pub mod buffer;
pub mod context;
pub mod devices;
pub mod gain;
pub mod microphone;
pub mod player;
pub mod sound;
pub mod system_audio;
pub mod web_audio_mixer;

pub use analyser::{AnalyserChannels, AnalyserTap, AnalyserTapOptions, AudioAnalyserStatus};
pub use buffer::{AudioBuffer, set_asset_root};
pub use context::{AudioContextStatus, AudioInput, AudioOutput, AudioSource, Bus, Destination};
pub use devices::{AudioDevices, UseAudioDevicesResult, use_audio_devices};
pub use gain::{GainNode, GainNodeOptions};
pub use microphone::{
    Microphone, MicrophoneHandle, MicrophoneOptions, MicrophoneStatus, UseMicrophone,
    use_microphone,
};
pub use player::{
    AudioPlayerEngine, install_audio_player_backend, use_audio_player, use_audio_player_with,
};
pub use sound::{Sound, SoundOptions, play_click, use_sound};
pub use system_audio::{SystemAudio, SystemAudioOptions, SystemAudioStatus, use_system_audio};
pub use web_audio_mixer::{DuckingOptions, WebAudioMixer, WebAudioMixerOptions};

use std::fmt;

/// What went wrong opening a device, a stream or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioIoErrorKind {
    /// The system refused access (`NotAllowedError`).
    Denied,
    /// No such device, or it went away (`NotFoundError`).
    Unavailable,
    /// The device or format cannot do what was asked.
    Unsupported,
    /// The file could not be read or decoded (`MediaError`).
    Decode,
    Other,
}

/// An error from the native audio layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioIoError {
    pub kind: AudioIoErrorKind,
    pub message: String,
}

impl AudioIoError {
    pub fn new(kind: AudioIoErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for AudioIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AudioIoError {}

impl From<cpal::Error> for AudioIoError {
    fn from(error: cpal::Error) -> Self {
        use cpal::ErrorKind as K;
        let kind = match error.kind() {
            K::PermissionDenied => AudioIoErrorKind::Denied,
            K::DeviceNotAvailable | K::HostUnavailable | K::StreamInvalidated => {
                AudioIoErrorKind::Unavailable
            }
            K::UnsupportedConfig | K::UnsupportedOperation => AudioIoErrorKind::Unsupported,
            _ => AudioIoErrorKind::Other,
        };
        Self::new(kind, error.to_string())
    }
}

/// `Result` with an [`AudioIoError`].
pub type Result<T, E = AudioIoError> = std::result::Result<T, E>;
