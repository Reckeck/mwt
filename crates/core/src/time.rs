//! Time port: a source of "now" that tests can pin, and the plain helpers
//! that turn timestamps and dates into unix seconds.

use chrono::NaiveDate;

/// Unix seconds of an RFC3339 timestamp. Plain string order is not enough:
/// `…:00Z` and `…:00.5Z` sort the wrong way round.
pub fn unix_secs(iso: &str) -> Option<i64> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(iso.trim(), &Rfc3339)
        .ok()
        .map(|moment| moment.unix_timestamp())
}

/// Unix seconds of `iso`, or now when it is empty or unreadable.
pub fn secs_or_now(iso: &str) -> i64 {
    unix_secs(iso).unwrap_or_else(|| time::OffsetDateTime::now_utc().unix_timestamp())
}

/// RFC3339 midnight UTC of `date`, the key daily prices use.
pub fn date_ts(date: NaiveDate) -> String {
    format!("{}T00:00:00Z", date.format("%Y-%m-%d"))
}

/// Unix seconds of midnight UTC of `date`.
pub fn date_secs(date: NaiveDate) -> i64 {
    date.and_hms_opt(0, 0, 0)
        .map_or(0, |moment| moment.and_utc().timestamp())
}

/// A local calendar date; the key an automation occurrence is identified by.
pub type LocalDate = chrono::NaiveDate;

/// Source of the current moment, in unix seconds.
pub trait Clock {
    fn now_unix(&self) -> i64;
}

/// The wall clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        crate::calendar::now_secs()
    }
}

/// A clock stuck at one moment.
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_returns_its_moment() {
        assert_eq!(FixedClock(42).now_unix(), 42);
    }

    #[test]
    fn the_system_clock_is_after_2020() {
        assert!(SystemClock.now_unix() > 1_577_836_800);
    }

    #[test]
    fn moved_time_helpers_keep_their_old_paths() {
        let day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let iso = "2026-03-01T20:00:00Z";
        assert_eq!(crate::pricing::history::unix_secs(iso), unix_secs(iso));
        assert_eq!(crate::pricing::history::secs_or_now(iso), secs_or_now(iso));
        assert_eq!(crate::cryptotrack::prices::date_secs(day), date_secs(day));
        assert_eq!(crate::cryptotrack::prices::date_ts(day), date_ts(day));
        assert_eq!(unix_secs(iso), Some(1_772_395_200));
    }
}
