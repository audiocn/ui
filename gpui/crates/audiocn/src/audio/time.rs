//! Port of `lib/audio/time.ts`.

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 3600;
const MINUS_SIGN: &str = "\u{2212}";

/// Options for [`format_time`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatTimeOptions {
    /// Prefix with "−", for time remaining.
    pub remaining: bool,
    /// Always show hours.
    pub hours: bool,
}

/// Formats seconds as `m:ss`, or `h:mm:ss` from one hour. NaN, infinite and
/// negative input show as `0:00`.
pub fn format_time(seconds: f64, options: FormatTimeOptions) -> String {
    let total = if seconds.is_finite() {
        seconds.floor().max(0.0) as u64
    } else {
        0
    };
    let hours = total / SECONDS_PER_HOUR;
    let minutes = (total % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let secs = total % SECONDS_PER_MINUTE;
    let prefix = if options.remaining { MINUS_SIGN } else { "" };

    if hours > 0 || options.hours {
        return format!("{prefix}{hours}:{minutes:02}:{secs:02}");
    }
    format!("{prefix}{minutes}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(s: f64) -> String {
        format_time(s, FormatTimeOptions::default())
    }

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(fmt(0.0), "0:00");
        assert_eq!(fmt(84.9), "1:24");
    }

    #[test]
    fn switches_to_hours() {
        assert_eq!(fmt(3725.0), "1:02:05");
        let hours = FormatTimeOptions {
            hours: true,
            ..Default::default()
        };
        assert_eq!(format_time(65.0, hours), "0:01:05");
    }

    #[test]
    fn prefixes_remaining_and_guards_bad_input() {
        let remaining = FormatTimeOptions {
            remaining: true,
            ..Default::default()
        };
        assert_eq!(format_time(30.0, remaining), "−0:30");
        assert_eq!(fmt(f64::NAN), "0:00");
        assert_eq!(fmt(-5.0), "0:00");
    }
}
