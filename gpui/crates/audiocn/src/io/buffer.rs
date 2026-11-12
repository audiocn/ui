//! Decoded audio in memory (`AudioBuffer`), decoding with symphonia (wav,
//! mp3, ogg/vorbis, flac) and `src` resolution.
//!
//! React loads sources by URL. Natively a `src` is a file path, a `file://`
//! URL, or a site path like `/sounds/peon-work-work.wav`, which resolves
//! against the [asset root](set_asset_root) (the React app's `public/`).
//! Remote `http(s)` URLs are not supported.
//!
//! ```ignore
//! set_asset_root("public");
//! let buffer = AudioBuffer::load("/sounds/peon-work-work.wav")?;
//! ```

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{AudioIoError, AudioIoErrorKind, Result};

struct BufferData {
    channels: Vec<Vec<f32>>,
    sample_rate: u32,
}

/// Decoded, planar `f32` audio (`AudioBuffer`). Cheap to clone.
#[derive(Clone)]
pub struct AudioBuffer {
    data: Arc<BufferData>,
}

impl PartialEq for AudioBuffer {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

impl std::fmt::Debug for AudioBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioBuffer")
            .field("channels", &self.number_of_channels())
            .field("length", &self.length())
            .field("sample_rate", &self.sample_rate())
            .finish()
    }
}

impl AudioBuffer {
    /// A buffer from planar channels (all the same length).
    pub fn from_channels(channels: Vec<Vec<f32>>, sample_rate: u32) -> Self {
        Self {
            data: Arc::new(BufferData {
                channels,
                sample_rate: sample_rate.max(1),
            }),
        }
    }

    /// A mono buffer (e.g. a synthesised [`ClickSound`](crate::audio::click::ClickSound)).
    pub fn mono(samples: impl Into<Vec<f32>>, sample_rate: u32) -> Self {
        Self::from_channels(vec![samples.into()], sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.data.sample_rate
    }

    pub fn number_of_channels(&self) -> usize {
        self.data.channels.len()
    }

    /// Frames per channel.
    pub fn length(&self) -> usize {
        self.data.channels.first().map_or(0, Vec::len)
    }

    /// Seconds.
    pub fn duration(&self) -> f64 {
        self.length() as f64 / self.data.sample_rate as f64
    }

    /// `getChannelData`.
    pub fn channel(&self, index: usize) -> &[f32] {
        self.data.channels.get(index).map_or(&[], Vec::as_slice)
    }

    pub fn channels(&self) -> &[Vec<f32>] {
        &self.data.channels
    }

    /// Stereo frame at `index` (mono duplicated), 0 past the end.
    #[inline]
    pub(crate) fn frame(&self, index: usize) -> [f32; 2] {
        let channels = &self.data.channels;
        match channels.len() {
            0 => [0.0; 2],
            1 => {
                let sample = channels[0].get(index).copied().unwrap_or(0.0);
                [sample, sample]
            }
            _ => [
                channels[0].get(index).copied().unwrap_or(0.0),
                channels[1].get(index).copied().unwrap_or(0.0),
            ],
        }
    }

    /// Decodes an encoded file in memory. `extension` (e.g. `"mp3"`) helps
    /// format detection.
    pub fn decode(bytes: Vec<u8>, extension: Option<&str>) -> Result<AudioBuffer> {
        decode_source(Box::new(Cursor::new(bytes)), extension)
    }

    /// Reads and decodes a `src` (see the module docs).
    pub fn load(src: &str) -> Result<AudioBuffer> {
        let path = resolve_src(src)?;
        let file = std::fs::File::open(&path).map_err(|error| {
            AudioIoError::new(
                AudioIoErrorKind::Unavailable,
                format!("could not open {}: {error}", path.display()),
            )
        })?;
        let extension = path.extension().and_then(|e| e.to_str()).map(str::to_owned);
        decode_source(Box::new(file), extension.as_deref())
    }

    /// [`load`](Self::load), decoded once per `src` (`loadAudioBuffer`).
    /// Failures are not cached.
    pub fn load_cached(src: &str) -> Result<AudioBuffer> {
        if let Some(buffer) = BUFFER_CACHE.lock().get(src) {
            return Ok(buffer.clone());
        }
        let buffer = Self::load(src)?;
        BUFFER_CACHE.lock().insert(src.to_string(), buffer.clone());
        Ok(buffer)
    }
}

static BUFFER_CACHE: std::sync::LazyLock<Mutex<HashMap<String, AudioBuffer>>> =
    std::sync::LazyLock::new(Default::default);

static ASSET_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Where site paths like `/sounds/x.wav` resolve (the React app's `public/`).
/// Without one, they resolve against `./public` if it exists, else the
/// working directory.
pub fn set_asset_root(path: impl Into<PathBuf>) {
    *ASSET_ROOT.lock() = Some(path.into());
}

/// Turns a `src` into a file path.
pub fn resolve_src(src: &str) -> Result<PathBuf> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Err(AudioIoError::new(
            AudioIoErrorKind::Unsupported,
            format!("remote sources are not supported natively: {src}"),
        ));
    }
    if let Some(path) = src.strip_prefix("file://") {
        return Ok(PathBuf::from(path));
    }
    let literal = Path::new(src);
    if literal.exists() {
        return Ok(literal.to_path_buf());
    }
    if let Some(relative) = src.strip_prefix('/') {
        let root = ASSET_ROOT.lock().clone().unwrap_or_else(|| {
            let public = PathBuf::from("public");
            if public.is_dir() {
                public
            } else {
                PathBuf::from(".")
            }
        });
        return Ok(root.join(relative));
    }
    Ok(literal.to_path_buf())
}

fn decode_error(error: impl std::fmt::Display) -> AudioIoError {
    AudioIoError::new(
        AudioIoErrorKind::Decode,
        format!("could not decode: {error}"),
    )
}

fn decode_source(source: Box<dyn MediaSource>, extension: Option<&str>) -> Result<AudioBuffer> {
    let stream = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(decode_error)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| decode_error("no audio track"))?;
    let track_id = track.id;
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(decode_error)?;

    let mut channels: Vec<Vec<f32>> = Vec::new();
    let mut samples: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(error) => return Err(decode_error(error)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A corrupt packet: skip it, like browsers do.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(decode_error(error)),
        };
        let spec = *decoded.spec();
        sample_rate = spec.rate;
        let count = spec.channels.count().max(1);
        if channels.len() != count {
            channels.resize_with(count, Vec::new);
        }
        if samples
            .as_ref()
            .is_none_or(|buffer| buffer.capacity() < decoded.capacity() * count)
        {
            samples = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
        }
        let Some(buffer) = samples.as_mut() else {
            continue;
        };
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(count) {
            for (channel, sample) in channels.iter_mut().zip(frame) {
                channel.push(*sample);
            }
        }
    }
    if channels.is_empty() || sample_rate == 0 {
        return Err(decode_error("no audio"));
    }
    Ok(AudioBuffer::from_channels(channels, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit PCM wav file.
    fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn decodes_stereo_wav() {
        let samples: Vec<i16> = (0..2000).flat_map(|_| [16384i16, -16384]).collect();
        let buffer = AudioBuffer::decode(wav(&samples, 2, 22_050), Some("wav")).unwrap();
        assert_eq!(buffer.number_of_channels(), 2);
        assert_eq!(buffer.sample_rate(), 22_050);
        assert_eq!(buffer.length(), 2000);
        assert!((buffer.channel(0)[10] - 0.5).abs() < 1e-3);
        assert!((buffer.channel(1)[10] + 0.5).abs() < 1e-3);
        assert!((buffer.duration() - 2000.0 / 22_050.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_garbage_and_remote_sources() {
        let error = AudioBuffer::decode(vec![1, 2, 3, 4], None).unwrap_err();
        assert_eq!(error.kind, AudioIoErrorKind::Decode);
        assert_eq!(
            resolve_src("https://example.com/a.mp3").unwrap_err().kind,
            AudioIoErrorKind::Unsupported
        );
    }

    #[test]
    fn site_paths_resolve_against_the_asset_root() {
        assert_eq!(
            resolve_src("file:///tmp/a.wav").unwrap(),
            PathBuf::from("/tmp/a.wav")
        );
        assert!(
            resolve_src("/sounds/none.wav")
                .unwrap()
                .ends_with("sounds/none.wav")
        );
    }

    #[test]
    fn decodes_the_demo_sound() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../public/sounds/peon-work-work.wav"
        );
        if !Path::new(path).exists() {
            return;
        }
        let buffer = AudioBuffer::load(path).unwrap();
        assert!(buffer.duration() > 0.1);
    }
}
