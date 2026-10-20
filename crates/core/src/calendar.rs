//! Local calendar arithmetic (plan §7.6). Everything is stored in UTC; days,
//! weeks and months that budgets and schedules talk about are cut at the
//! user's local midnight. The time zone is an IANA name the app layer passes
//! in (the OS zone unless the user picked one); an empty or unknown name
//! means UTC.

pub mod workdays;

use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Hour of the local day an automation fires (plan §7.2).
pub const AUTOMATION_HOUR: u32 = 9;

#[derive(Debug, Clone, Copy)]
pub struct Calendar {
    tz: Tz,
}

impl Default for Calendar {
    fn default() -> Self {
        Self { tz: Tz::UTC }
    }
}

impl Calendar {
    pub fn new(zone: &str) -> Self {
        Self {
            tz: zone.trim().parse().unwrap_or(Tz::UTC),
        }
    }

    /// Whether `zone` is a known IANA name.
    pub fn is_valid_zone(zone: &str) -> bool {
        zone.trim().parse::<Tz>().is_ok()
    }

    pub fn zone_name(&self) -> &'static str {
        self.tz.name()
    }

    /// Local date of a moment.
    pub fn date_of(&self, secs: i64) -> NaiveDate {
        Utc.timestamp_opt(secs, 0)
            .single()
            .unwrap_or_default()
            .with_timezone(&self.tz)
            .date_naive()
    }

    /// Local wall-clock date and time of a moment.
    pub fn local(&self, secs: i64) -> NaiveDateTime {
        Utc.timestamp_opt(secs, 0)
            .single()
            .unwrap_or_default()
            .with_timezone(&self.tz)
            .naive_local()
    }

    /// Local date of an RFC3339 moment.
    pub fn date_of_iso(&self, iso: &str) -> Option<NaiveDate> {
        crate::pricing::history::unix_secs(iso).map(|secs| self.date_of(secs))
    }

    /// The moment a local wall-clock time happens. A time skipped by a DST
    /// jump resolves to the first valid moment after it; an ambiguous one to
    /// the earlier of the two.
    pub fn at_local(&self, date: NaiveDate, hour: u32) -> i64 {
        let wall: NaiveDateTime = date.and_hms_opt(hour, 0, 0).unwrap_or_default();
        for shift in 0..4 {
            let probe = wall + Duration::hours(shift);
            match self.tz.from_local_datetime(&probe) {
                LocalResult::Single(moment) => return moment.timestamp(),
                LocalResult::Ambiguous(early, _) => return early.timestamp(),
                LocalResult::None => continue,
            }
        }
        wall.and_utc().timestamp()
    }

    /// `date` at `hour:minute` local time, as Unix seconds.
    pub fn at_local_hm(&self, date: NaiveDate, hour: u32, minute: u32) -> i64 {
        self.at_local(date, hour) + i64::from(minute.min(59)) * 60
    }

    /// Local midnight that starts `date`.
    pub fn start_of(&self, date: NaiveDate) -> i64 {
        self.at_local(date, 0)
    }

    /// `[start, end)` of the local calendar month containing `date`.
    pub fn month_bounds(&self, date: NaiveDate) -> (i64, i64) {
        let first = first_of_month(date);
        (self.start_of(first), self.start_of(next_month(first)))
    }
}

pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

pub fn next_month(date: NaiveDate) -> NaiveDate {
    let first = first_of_month(date);
    let (year, month) = if first.month() == 12 {
        (first.year() + 1, 1)
    } else {
        (first.year(), first.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(first)
}

pub fn days_in_month(date: NaiveDate) -> u32 {
    let first = first_of_month(date);
    (next_month(first) - first).num_days() as u32
}

/// `day` of the month of `date`, clamped to the month's last day.
pub fn clamp_day(date: NaiveDate, day: u8) -> NaiveDate {
    let day = u32::from(day.max(1)).min(days_in_month(date));
    first_of_month(date).with_day(day).unwrap_or(date)
}

/// `YYYY-MM` of a date.
pub fn month_key(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

/// The first day of a `YYYY-MM` month.
pub fn parse_month(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{}-01", key.trim()), "%Y-%m-%d").ok()
}

/// RFC3339 UTC of a moment.
pub fn iso(secs: i64) -> String {
    Utc.timestamp_opt(secs, 0)
        .single()
        .map(|moment| moment.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

pub fn now_secs() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn the_31st_of_february_is_its_last_day() {
        assert_eq!(clamp_day(date("2026-02-10"), 31), date("2026-02-28"));
        assert_eq!(clamp_day(date("2028-02-10"), 31), date("2028-02-29"));
        assert_eq!(days_in_month(date("2026-12-05")), 31);
    }

    #[test]
    fn a_day_is_cut_at_local_midnight() {
        let bangkok = Calendar::new("Asia/Bangkok");
        // 2026-03-01T20:00Z is 03:00 on the 2nd in Bangkok (UTC+7).
        let secs = crate::pricing::history::unix_secs("2026-03-01T20:00:00Z").unwrap();
        assert_eq!(bangkok.date_of(secs), date("2026-03-02"));
        assert_eq!(Calendar::new("").date_of(secs), date("2026-03-01"));
        assert_eq!(
            iso(bangkok.start_of(date("2026-03-02"))),
            "2026-03-01T17:00:00Z"
        );
    }

    #[test]
    fn unknown_zones_fall_back_to_utc() {
        assert_eq!(Calendar::new("Mars/Olympus").zone_name(), "UTC");
        assert!(!Calendar::is_valid_zone("Mars/Olympus"));
        assert!(Calendar::is_valid_zone("Europe/Moscow"));
    }
}
