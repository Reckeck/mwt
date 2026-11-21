//! Properties of the calendar and the workday rules.

use chrono::{Datelike, NaiveDate};
use proptest::prelude::*;

use crate::calendar::workdays::{WorkdayRules, parse_date};
use crate::calendar::{Calendar, first_of_month, month_index, next_month};
use crate::props::{day, zone};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn date_of_start_of_is_the_day(zone in zone(), date in day()) {
        let calendar = Calendar::new(zone);
        prop_assert_eq!(calendar.date_of(calendar.start_of(date)), date);
    }

    #[test]
    fn month_bounds_span_the_local_month(zone in zone(), date in day()) {
        let calendar = Calendar::new(zone);
        let first = first_of_month(date);
        let (start, end) = calendar.month_bounds(date);
        prop_assert_eq!(start, calendar.start_of(first));
        prop_assert_eq!(end, calendar.start_of(next_month(first)));
        prop_assert!(start < end);
    }

    #[test]
    fn date_of_any_second_lands_in_its_zone_day(zone in zone(), secs in 0..7_258_118_400i64) {
        let calendar = Calendar::new(zone);
        let date = calendar.date_of(secs);
        prop_assert!(calendar.start_of(date) <= secs);
    }
}

proptest! {
    #[test]
    fn first_of_month_is_idempotent(date in day()) {
        let first = first_of_month(date);
        prop_assert_eq!(first.day(), 1);
        prop_assert_eq!(first_of_month(first), first);
    }

    #[test]
    fn month_index_agrees_with_a_linear_search(
        months in prop::collection::vec(day(), 0..12),
        date in day(),
    ) {
        let firsts: Vec<NaiveDate> = months.iter().map(|month| first_of_month(*month)).collect();
        let mut expected = None;
        for (index, month) in firsts.iter().enumerate() {
            if month.year() == date.year() && month.month() == date.month() {
                expected = Some(index);
                break;
            }
        }
        prop_assert_eq!(month_index(&firsts, date), expected);
    }

    #[test]
    fn workday_neighbours_bracket_the_day(date in day()) {
        let rules = WorkdayRules::ru_default();
        let (previous, next) = (rules.previous_workday(date), rules.next_workday(date));
        prop_assert!(rules.classify(previous).workday);
        prop_assert!(rules.classify(next).workday);
        // A working day is its own neighbour; a day off has both strictly around it.
        if rules.classify(date).workday {
            prop_assert_eq!((previous, next), (date, date));
        } else {
            prop_assert!(previous < date && date < next);
        }
    }

    #[test]
    fn parse_date_never_panics(raw in "\\PC{0,40}", zone in zone()) {
        let _ = parse_date(&raw, &Calendar::new(zone));
    }
}
