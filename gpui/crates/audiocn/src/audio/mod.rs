//! Pure audio maths, ported from `lib/audio/*` (and `lib/electric.ts`).
//! Each module mirrors one TypeScript file; functions keep their names in snake_case.
pub mod ballistics;
pub mod bands;
pub mod bar_levels;
pub mod click;
pub mod decibels;
pub mod electric;
pub mod frame_loop;
pub mod frame_source;
pub mod history;
pub mod history_playback;
pub mod taper;
pub mod time;
pub mod types;
pub mod wave_line;
pub mod zones;

pub use frame_source::*;
pub use types::*;
