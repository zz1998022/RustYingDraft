/// One second in microseconds (JianYing's internal time unit).
pub const SEC: u64 = 1_000_000;

/// Default duration for photo/still image materials (3 hours).
pub const PHOTO_DURATION_US: u64 = 10_800_000_000;

/// A time interval on the timeline, stored in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TimeRange {
    pub start: u64,
    pub duration: u64,
}

impl TimeRange {
    pub fn new(start: u64, duration: u64) -> Self {
        Self { start, duration }
    }

    pub fn end(&self) -> u64 {
        self.start + self.duration
    }

    pub fn overlaps(&self, other: &TimeRange) -> bool {
        !(self.end() <= other.start || other.end() <= self.start)
    }

    /// Create from seconds (floating point).
    pub fn from_secs(start: f64, duration: f64) -> Self {
        Self {
            start: (start * SEC as f64) as u64,
            duration: (duration * SEC as f64) as u64,
        }
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.3}s, {:.3}s)",
            self.start as f64 / SEC as f64,
            self.end() as f64 / SEC as f64
        )
    }
}

/// Parse a time string like "1h52m3s", "0.15s", "2s" into microseconds.
/// Supports h/m/s units, with optional decimals and negative signs.
pub fn parse_time_str(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty time string".into());
    }

    let negative = s.starts_with('-');
    let s = if negative { &s[1..] } else { s };

    let mut total_us: i64 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        match ch {
            '0'..='9' | '.' => num_buf.push(ch),
            'h' | 'm' | 's' => {
                let val: f64 = num_buf
                    .parse()
                    .map_err(|_| format!("invalid number in time string: {s}"))?;
                num_buf.clear();
                total_us += match ch {
                    'h' => (val * 3600.0 * SEC as f64) as i64,
                    'm' => (val * 60.0 * SEC as f64) as i64,
                    's' => (val * SEC as f64) as i64,
                    _ => unreachable!(),
                };
            }
            _ => return Err(format!("unexpected character '{ch}' in time string: {s}")),
        }
    }

    if !num_buf.is_empty() {
        return Err(format!("trailing number without unit in time string: {s}"));
    }

    if negative {
        total_us = -total_us;
    }

    if total_us < 0 {
        return Err(format!("negative time not supported: {s}"));
    }

    Ok(total_us as u64)
}

/// Convenience: parse a time value from either a string or raw microseconds.
pub fn tim(value: &str) -> u64 {
    value
        .parse::<u64>()
        .unwrap_or_else(|_| parse_time_str(value).unwrap_or(0))
}

/// Convenience: create a TimeRange from string time values.
pub fn trange(start: &str, duration: &str) -> TimeRange {
    TimeRange::new(tim(start), tim(duration))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_constant() {
        assert_eq!(SEC, 1_000_000);
    }

    #[test]
    fn test_timerange_overlaps() {
        let a = TimeRange::new(0, SEC);
        let b = TimeRange::new(SEC, SEC); // starts right where a ends
        assert!(!a.overlaps(&b));
        assert!(!b.overlaps(&a));

        let c = TimeRange::new(500_000, SEC);
        assert!(a.overlaps(&c));
    }

    #[test]
    fn test_parse_time_str() {
        assert_eq!(parse_time_str("1s").unwrap(), SEC);
        assert_eq!(parse_time_str("0.5s").unwrap(), 500_000);
        assert_eq!(parse_time_str("1m").unwrap(), 60 * SEC);
        assert_eq!(parse_time_str("1h").unwrap(), 3600 * SEC);
        assert_eq!(parse_time_str("1h30m").unwrap(), 90 * 60 * SEC);
        assert_eq!(
            parse_time_str("1h52m3s").unwrap(),
            (3600 + 52 * 60 + 3) * SEC
        );
    }

    #[test]
    fn test_from_secs() {
        let tr = TimeRange::from_secs(0.0, 5.0);
        assert_eq!(tr.start, 0);
        assert_eq!(tr.duration, 5 * SEC);
    }
}
