//! Port of `lib/audio/zones.ts`.

use crate::audio::types::{MeterZone, MeterZoneName};

/// Green below −20 dBFS, amber from −20, red from −9: the thresholds streamers know.
pub const DEFAULT_ZONES: [MeterZone; 3] = [
    MeterZone {
        from_db: f32::NEG_INFINITY,
        zone: MeterZoneName::Ok,
    },
    MeterZone {
        from_db: -20.0,
        zone: MeterZoneName::Warn,
    },
    MeterZone {
        from_db: -9.0,
        zone: MeterZoneName::Clip,
    },
];

/// A level at or above this counts as clipping for a clip light.
pub const CLIP_THRESHOLD_DB: f32 = -1.0;
/// How long a clip light stays on after the last clip, in ms.
pub const CLIP_HOLD_MS: f64 = 1500.0;

/// The zone a level falls in: the last zone whose `from_db` it reaches.
/// Pass `&DEFAULT_ZONES` for the TS default.
pub fn zone_for_db(db: f32, zones: &[MeterZone]) -> MeterZoneName {
    let mut current = MeterZoneName::Ok;
    for zone in zones {
        if db >= zone.from_db {
            current = zone.zone;
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_default_thresholds() {
        assert_eq!(zone_for_db(-30.0, &DEFAULT_ZONES), MeterZoneName::Ok);
        assert_eq!(zone_for_db(-20.0, &DEFAULT_ZONES), MeterZoneName::Warn);
        assert_eq!(zone_for_db(-9.0, &DEFAULT_ZONES), MeterZoneName::Clip);
        assert_eq!(
            zone_for_db(f32::NEG_INFINITY, &DEFAULT_ZONES),
            MeterZoneName::Ok
        );
    }

    #[test]
    fn accepts_custom_zones() {
        let zones = [
            DEFAULT_ZONES[0],
            MeterZone {
                from_db: -3.0,
                zone: MeterZoneName::Clip,
            },
        ];
        assert_eq!(zone_for_db(-9.0, &zones), MeterZoneName::Ok);
        assert_eq!(zone_for_db(-2.0, &zones), MeterZoneName::Clip);
    }
}
