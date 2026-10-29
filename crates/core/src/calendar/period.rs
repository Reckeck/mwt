//! Dashboard periods (plan H1): the window up to today and the same span of
//! the previous period "to the same date".

use chrono::{Datelike, NaiveDate};

use crate::model::vault::DashPeriod;

/// Both windows are inclusive day ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodWindow {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub prev_start: NaiveDate,
    pub prev_end: NaiveDate,
}

/// Months in one period.
pub fn period_months(period: DashPeriod) -> u32 {
    match period {
        DashPeriod::Month => 1,
        DashPeriod::Quarter => 3,
        DashPeriod::Year => 12,
    }
}

/// `date` moved `months` back, the day clamped to the shorter month
/// (31 March one month back is 28/29 February).
pub fn months_back(date: NaiveDate, months: u32) -> NaiveDate {
    let index = date.year() * 12 + date.month0() as i32 - months as i32;
    let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date);
    let day = date.day().min(super::days_in_month(first));
    first.with_day(day).unwrap_or(first)
}

/// The current period from its first day to `today`, and the previous period
/// of the same length up to the same date.
pub fn period_window(period: DashPeriod, today: NaiveDate) -> PeriodWindow {
    let first = super::first_of_month(today);
    let start = match period {
        DashPeriod::Month => first,
        DashPeriod::Quarter => months_back(first, first.month0() % 3),
        DashPeriod::Year => months_back(first, first.month0()),
    };
    let span = period_months(period);
    PeriodWindow {
        start,
        end: today,
        prev_start: months_back(start, span),
        prev_end: months_back(today, span),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn month_compares_to_the_same_date_of_the_previous_month() {
        let window = period_window(DashPeriod::Month, day("2026-10-22"));
        assert_eq!(window.start, day("2026-10-01"));
        assert_eq!(window.end, day("2026-10-22"));
        assert_eq!(window.prev_start, day("2026-09-01"));
        assert_eq!(window.prev_end, day("2026-09-22"));
    }

    #[test]
    fn a_longer_month_clamps_to_the_shorter_one() {
        let window = period_window(DashPeriod::Month, day("2026-03-31"));
        assert_eq!(window.prev_start, day("2026-02-01"));
        assert_eq!(window.prev_end, day("2026-02-28"));
        let leap = period_window(DashPeriod::Month, day("2028-03-30"));
        assert_eq!(leap.prev_end, day("2028-02-29"));
    }

    #[test]
    fn january_looks_back_into_december() {
        let window = period_window(DashPeriod::Month, day("2027-01-15"));
        assert_eq!(window.prev_start, day("2026-12-01"));
        assert_eq!(window.prev_end, day("2026-12-15"));
    }

    #[test]
    fn quarter_starts_on_its_first_month() {
        let window = period_window(DashPeriod::Quarter, day("2026-11-30"));
        assert_eq!(window.start, day("2026-10-01"));
        assert_eq!(window.prev_start, day("2026-07-01"));
        assert_eq!(window.prev_end, day("2026-08-30"));
        let first = period_window(DashPeriod::Quarter, day("2026-02-10"));
        assert_eq!(first.start, day("2026-01-01"));
        assert_eq!(first.prev_start, day("2025-10-01"));
        assert_eq!(first.prev_end, day("2025-11-10"));
    }

    #[test]
    fn year_compares_to_the_same_date_a_year_ago() {
        let window = period_window(DashPeriod::Year, day("2028-02-29"));
        assert_eq!(window.start, day("2028-01-01"));
        assert_eq!(window.prev_start, day("2027-01-01"));
        assert_eq!(window.prev_end, day("2027-02-28"));
    }
}
