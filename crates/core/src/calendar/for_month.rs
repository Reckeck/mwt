//! «За какой месяц»: the month an income is for when it is not the month of
//! its date. A salary paid on the 5th is for the month before; an advance
//! paid on the 20th is for the month it comes in.

use chrono::{Months, NaiveDate};

use super::{clamp_day, first_of_month, month_key};

/// Furthest an income is for, in months from its planned month.
pub const MAX_FOR_MONTH_OFFSET: i8 = 12;

/// The month of the schedule's own day, not of the date a weekend moved it to:
/// a «forward» shift of the 31st can land on the 2nd of the next month, a
/// «back» shift of the 1st on the last day of the month before. Of the planned
/// month and its neighbours the one whose `day` is nearest to `planned` wins.
pub fn nominal_month(planned: NaiveDate, day: u8) -> NaiveDate {
    let here = first_of_month(planned);
    let neighbours = [
        here.checked_sub_months(Months::new(1)),
        Some(here),
        here.checked_add_months(Months::new(1)),
    ];
    neighbours
        .into_iter()
        .flatten()
        .min_by_key(|month| (clamp_day(*month, day) - planned).num_days().abs())
        .unwrap_or(here)
}

/// `YYYY-MM` of the month `offset` months from the schedule's month (0 — this
/// one, -1 — the previous): `day` is the schedule's day of month, `planned`
/// the date the run is planned for (after a weekend shift).
pub fn for_month_key(planned: NaiveDate, day: Option<u8>, offset: i8) -> String {
    let offset = offset.clamp(-MAX_FOR_MONTH_OFFSET, MAX_FOR_MONTH_OFFSET);
    let base = day.map_or_else(
        || first_of_month(planned),
        |day| nominal_month(planned, day),
    );
    let shifted = if offset < 0 {
        base.checked_sub_months(Months::new(u32::from(offset.unsigned_abs())))
    } else {
        base.checked_add_months(Months::new(u32::from(offset.unsigned_abs())))
    };
    month_key(shifted.unwrap_or(base))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn the_fifth_is_for_the_previous_month() {
        assert_eq!(for_month_key(date("2026-10-05"), Some(5), -1), "2026-09");
    }

    #[test]
    fn the_twentieth_is_for_this_month() {
        assert_eq!(for_month_key(date("2026-10-20"), Some(20), 0), "2026-10");
    }

    #[test]
    fn the_previous_month_of_january_is_december() {
        assert_eq!(for_month_key(date("2027-01-05"), Some(5), -1), "2026-12");
    }

    #[test]
    fn a_weekend_shift_back_does_not_change_the_month() {
        // The 5th of Sept 2026 is a Saturday: «в пятницу перед» pays on the 4th.
        assert_eq!(for_month_key(date("2026-09-04"), Some(5), -1), "2026-08");
        // Sunday the 1st of Nov 2026 moved back lands on Friday Oct 30th.
        assert_eq!(for_month_key(date("2026-10-30"), Some(1), -1), "2026-10");
        assert_eq!(for_month_key(date("2026-10-30"), Some(1), 0), "2026-11");
    }

    #[test]
    fn a_weekend_shift_forward_does_not_change_the_month() {
        // Saturday the 31st of Jan 2026 moved forward lands on Monday Feb 2nd.
        assert_eq!(for_month_key(date("2026-02-02"), Some(31), 0), "2026-01");
        assert_eq!(for_month_key(date("2026-02-02"), Some(31), -1), "2025-12");
    }

    #[test]
    fn without_a_schedule_day_the_planned_month_counts() {
        assert_eq!(for_month_key(date("2026-10-05"), None, -1), "2026-09");
    }
}
