//! Properties of the dashboard period arithmetic.

use chrono::{Datelike, NaiveDate};
use proptest::prelude::*;

use super::period::{months_back, period_window};
use mwt_base::calendar::days_in_month;
use mwt_model::DashPeriod;
use mwt_testkit::props::day;

fn period() -> impl Strategy<Value = DashPeriod> {
    prop_oneof![
        Just(DashPeriod::Month),
        Just(DashPeriod::Quarter),
        Just(DashPeriod::Year)
    ]
}

proptest! {
    #[test]
    fn months_back_never_moves_forward(date in day(), months in 0..600u32) {
        prop_assert!(months_back(date, months) <= date);
    }

    #[test]
    fn months_back_clamps_the_day_to_the_target_month(date in day(), months in 0..600u32) {
        let moved = months_back(date, months);
        let first = NaiveDate::from_ymd_opt(moved.year(), moved.month(), 1).unwrap();
        prop_assert_eq!(moved.day(), date.day().min(days_in_month(first)));
    }

    #[test]
    fn months_back_by_zero_is_identity(date in day()) {
        prop_assert_eq!(months_back(date, 0), date);
    }

    #[test]
    fn months_back_is_monotone(date in day(), near in 0..300u32, extra in 0..300u32) {
        prop_assert!(months_back(date, near + extra) <= months_back(date, near));
    }

    #[test]
    fn months_back_keeps_the_order_of_dates(
        left in day(),
        right in day(),
        months in 0..300u32,
    ) {
        let (early, late) = if left <= right { (left, right) } else { (right, left) };
        prop_assert!(months_back(early, months) <= months_back(late, months));
    }

    #[test]
    fn period_window_starts_on_the_first_day_of_the_period(
        kind in period(),
        today in day(),
    ) {
        let window = period_window(kind, today);
        prop_assert!(window.start <= window.end);
        prop_assert_eq!(window.end, today);
        prop_assert!(window.prev_start <= window.prev_end);
        prop_assert_eq!(window.start.day(), 1);
        match kind {
            DashPeriod::Month => prop_assert_eq!(window.start.month(), today.month()),
            DashPeriod::Quarter => prop_assert_eq!(window.start.month0() % 3, 0),
            DashPeriod::Year => prop_assert_eq!(window.start.month(), 1),
        }
        prop_assert_eq!(window.start.year(), today.year());
    }
}
