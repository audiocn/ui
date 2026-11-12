//! Smoke test of native audio I/O (no window):
//!
//! ```sh
//! cargo run -p audiocn --features device --example device_smoke          # list, 1 s of mic, silent playback
//! cargo run -p audiocn --features device --example device_smoke -- --play # playback you can hear
//! ```

#[cfg(all(feature = "device", not(target_family = "wasm")))]
fn main() {
    use std::time::{Duration, Instant};

    use audiocn::AudioDeviceKind;
    use audiocn::io::{
        AudioDevices, AudioOutput, Microphone, MicrophoneOptions, Sound, SoundOptions,
    };

    println!("host: {}", audiocn::io::devices::host_name());
    for kind in [AudioDeviceKind::AudioInput, AudioDeviceKind::AudioOutput] {
        match AudioDevices::list(kind) {
            Ok(devices) => {
                println!("{kind:?}: {} device(s)", devices.len());
                for device in devices {
                    let default = if device.is_default { " (default)" } else { "" };
                    let note = device.description.as_deref().unwrap_or("");
                    println!("  {}{default} {note}\n    {}", device.label, device.id);
                }
            }
            Err(error) => println!("{kind:?}: {error}"),
        }
    }
    match audiocn::io::system_audio::find_monitor_device() {
        Some(device) => println!("system audio monitor: {}", device.label),
        None => println!("system audio monitor: none"),
    }

    {
        use audiocn::io::{SystemAudio, SystemAudioStatus};
        let system = SystemAudio::new(Default::default());
        system.start();
        let started = Instant::now();
        while system.status() == SystemAudioStatus::Prompting
            && started.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(300));
        println!(
            "system audio: {:?}, error {:?}, peak {:?} dBFS",
            system.status(),
            system.error().map(|error| error.message),
            system
                .meter_source()
                .latest()
                .map(|frame| frame.channels[0].peak_db)
        );
        system.stop();
    }

    match Microphone::start(MicrophoneOptions::default()) {
        Ok(mic) => {
            println!(
                "mic: {} @ {} Hz, {} ch",
                mic.device_label(),
                mic.sample_rate(),
                mic.channels()
            );
            let started = Instant::now();
            let mut loudest = f32::NEG_INFINITY;
            let mut frames = 0u64;
            let mut seen = 0;
            while started.elapsed() < Duration::from_secs(1) {
                std::thread::sleep(Duration::from_millis(100));
                let source = mic.meter_source();
                if source.version() == seen {
                    continue;
                }
                seen = source.version();
                if let Some(frame) = source.latest() {
                    let level = frame.channels[0];
                    loudest = loudest.max(level.peak_db);
                    frames += 1;
                    println!(
                        "  peak {:7.1} dBFS  rms {:7.1} dBFS",
                        level.peak_db,
                        level.rms_db.unwrap_or(f32::NEG_INFINITY)
                    );
                }
            }
            let visual = mic.visual_source().latest();
            println!(
                "mic: {} meter versions, loudest peak {loudest:.1} dBFS, {} bands, history {}",
                mic.meter_source().version(),
                visual.as_ref().map_or(0, |v| v.bands.len()),
                visual.as_ref().map_or(0, |v| v.history_length),
            );
            let _ = frames;
            mic.stop();
        }
        Err(error) => println!("mic: {error} ({:?})", error.kind),
    }

    {
        let audible = std::env::args().any(|arg| arg == "--play");
        let output = AudioOutput::shared().expect("output");
        println!("output: {output:?}");
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../public/sounds/peon-work-work.wav"
        );
        let sound = Sound::new(
            path,
            SoundOptions {
                volume: if audible { 1.0 } else { 0.0 },
                ..Default::default()
            },
        );
        while sound.is_loading() {
            std::thread::sleep(Duration::from_millis(10));
        }
        println!(
            "sound: {:.2} s, error {:?}",
            sound.duration(),
            sound.error()
        );
        sound.play();
        std::thread::sleep(Duration::from_millis(50));
        println!("playing: {}", sound.is_playing());
        let mut progress = Vec::new();
        while sound.is_playing() {
            std::thread::sleep(Duration::from_millis(100));
            progress.push(sound.progress().latest().unwrap_or(0.0));
        }
        println!("progress: {progress:.2?}");
        println!(
            "done at {:.2} s on the output clock, progress {:?}",
            output.current_time(),
            sound.progress().latest()
        );
    }
}

#[cfg(not(all(feature = "device", not(target_family = "wasm"))))]
fn main() {
    eprintln!("run with --features device");
}
