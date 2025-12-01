//! Date/time handling for Amiga format.

/// Amiga date representation.
///
/// Amiga stores dates as days since January 1, 1978,
/// minutes since midnight, and ticks (1/50 second).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AmigaDate {
    /// Days since January 1, 1978.
    pub days: i32,
    /// Minutes since midnight.
    pub mins: i32,
    /// Ticks (1/50 second).
    pub ticks: i32,
}

impl AmigaDate {
    /// Create a new Amiga date from raw values.
    #[inline]
    pub const fn new(days: i32, mins: i32, ticks: i32) -> Self {
        Self { days, mins, ticks }
    }

    /// Convert to a more usable date format.
    #[inline]
    pub fn to_date_time(self) -> DateTime {
        let (year, month, day) = days_to_date(self.days);
        let hour = (self.mins / 60) as u8;
        let minute = (self.mins % 60) as u8;
        let second = (self.ticks / 50) as u8;

        DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Convert to Unix timestamp (seconds since 1970-01-01 00:00:00 UTC).
    ///
    /// This matches GRUB's `aftime2ctime()` behavior:
    /// `days * 86400 + min * 60 + hz / 50 + epoch_offset`
    ///
    /// The Amiga epoch is January 1, 1978, which is 8 years (2922 days)
    /// after the Unix epoch.
    #[inline]
    pub const fn to_unix_timestamp(self) -> i64 {
        const SECONDS_PER_DAY: i64 = 86400;
        const SECONDS_PER_MINUTE: i64 = 60;
        const TICKS_PER_SECOND: i64 = 50;
        // 8 years from 1970 to 1978 (including leap years 1972, 1976)
        // = 365 * 8 + 2 = 2922 days
        const EPOCH_OFFSET: i64 = 2922 * SECONDS_PER_DAY;

        (self.days as i64) * SECONDS_PER_DAY
            + (self.mins as i64) * SECONDS_PER_MINUTE
            + (self.ticks as i64) / TICKS_PER_SECOND
            + EPOCH_OFFSET
    }
}

/// Decoded date and time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateTime {
    /// Year (e.g., 1978-2100).
    pub year: u16,
    /// Month (1-12).
    pub month: u8,
    /// Day of month (1-31).
    pub day: u8,
    /// Hour (0-23).
    pub hour: u8,
    /// Minute (0-59).
    pub minute: u8,
    /// Second (0-59).
    pub second: u8,
}

/// Convert days since 1978-01-01 to (year, month, day).
fn days_to_date(mut days: i32) -> (u16, u8, u8) {
    const DAYS_IN_MONTH: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut year = 1978u16;

    // Find year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    // Find month
    let mut month = 1u8;
    let leap = is_leap_year(year);
    for (i, &days_in_month) in DAYS_IN_MONTH.iter().enumerate() {
        let dim = if i == 1 && leap { 29 } else { days_in_month };
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }

    (year, month, (days + 1) as u8)
}

/// Check if a year is a leap year.
#[inline]
const fn is_leap_year(year: u16) -> bool {
    if year.is_multiple_of(100) {
        year.is_multiple_of(400)
    } else {
        year.is_multiple_of(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch() {
        let date = AmigaDate::new(0, 0, 0);
        let dt = date.to_date_time();
        assert_eq!(dt.year, 1978);
        assert_eq!(dt.month, 1);
        assert_eq!(dt.day, 1);
        assert_eq!(dt.hour, 0);
        assert_eq!(dt.minute, 0);
        assert_eq!(dt.second, 0);
    }

    #[test]
    fn test_known_date() {
        // 1997-02-18 is day 6988
        let date = AmigaDate::new(6988, 0, 0);
        let dt = date.to_date_time();
        assert_eq!(dt.year, 1997);
        assert_eq!(dt.month, 2);
        assert_eq!(dt.day, 18);
    }

    #[test]
    fn test_time() {
        let date = AmigaDate::new(0, 754, 150); // 12:34:03
        let dt = date.to_date_time();
        assert_eq!(dt.hour, 12);
        assert_eq!(dt.minute, 34);
        assert_eq!(dt.second, 3);
    }

    #[test]
    fn test_leap_year() {
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(1984));
        assert!(!is_leap_year(1983));
    }
}
