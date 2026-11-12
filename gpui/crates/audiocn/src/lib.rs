//! # audiocn for GPUI
//!
//! Audio UI components for [GPUI](https://gpui.rs), Zed's Rust UI framework,
//! ported one-to-one from [audiocn](https://audiocn.dev) (React, built the
//! shadcn way): level meters, visualizers, faders, knobs, channel strips, a
//! complete mixer, players and sound pads.
//!
//! - **Themed by shadcn.** [`theme::Theme`] carries shadcn's CSS variables
//!   (and audiocn's own `--meter-*` / `--channel-*`) under the same names;
//!   [`theme::Theme::apply_css`] reads any shadcn theme.
//! - **Fast by default.** Meters and visualizers read a [`FrameSource`] while
//!   painting and request the next animation frame only while something moves.
//! - **Bring your own audio.** Components receive levels and values; they never
//!   open a microphone themselves.
//! - **Real units.** Mixer components speak decibels.
//! - **Accessible.** Keyboard control on every fader, knob and strip.
//!
//! Every component is a builder (`RenderOnce`) named after its React
//! counterpart (`<LevelMeter>` → [`LevelMeter`]). Interactive ones keep their
//! state in GPUI's keyed element state, so give each one an id that is unique
//! among its siblings.
//!
//! ```ignore
//! use audiocn::{Fader, LevelMeter};
//!
//! div()
//!     .child(LevelMeter::new("master").source(meter_source.clone()))
//!     .child(Fader::new("gain").value(db).on_value_change(|db, _, _| println!("{db} dB")))
//! ```

pub mod audio;
pub mod blocks;
pub mod components;
pub mod config;
#[cfg(all(feature = "device", not(target_family = "wasm")))]
pub mod io;
pub mod motion;
pub mod signal;
pub mod theme;
pub mod ui;

pub use audio::frame_source::{FrameEmitter, FrameSource, create_frame_emitter};
pub use audio::types::*;
pub use blocks::*;
pub use components::*;
pub use config::{AudioConfig, AudioSize};
pub use theme::{ActiveTheme, Theme};

/// Registers the key bindings the interactive components use. Call once at
/// startup (it is idempotent per app).
pub fn init(cx: &mut gpui::App) {
    ui::init(cx);
    components::init(cx);
}
