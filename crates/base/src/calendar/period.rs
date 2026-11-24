//! Operation period helpers (spec op-period P2-P4, P6, D2): bounds of a half,
//! quarter and year, the months a period touches, the resolution of a default
//! period rule from a money date, and exact share rounding. Pure, plain dates.

use chrono::{Datelike, Months, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};

use super::{days_in_month, first_of_month, month_key, next_month};
use crate::Amount;

/// Which half of a month: days 1..15 or 16..last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfPart {
    First,
    Second,
}

/// The kind of a period, the base twin of the model's `PeriodKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodUnit {
    Month,
    Half,
    Quarter,
    Year,
    Range,
}

/// A default period rule over plain values, the base twin of the model's `PeriodRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleUnit {
    PrevMonth,
    PrevHalfSecond,
    ThisHalfFirst,
    ThisMonth,
    NextMonth,
    /// `offset` units (months, quarters, years) from the
    /// unit of the money date (half-months for `Half`, so -1 on the first half
    /// is the second half of the previous month). `half` picks the half for
    /// `Half` and then the offset counts months; none steps from the half the
    /// money date is in. `Range` is not a rule unit and resolves
    /// like `Month`.
    Custom {
        unit: PeriodUnit,
        offset: i8,
        half: Option<HalfPart>,
    },
}

fn month_last_day(first: NaiveDate) -> NaiveDate {
    first.with_day(days_in_month(first)).unwrap_or(first)
}

fn shift_months(first: NaiveDate, delta: i32) -> NaiveDate {
    let months = Months::new(delta.unsigned_abs());
    let shifted = if delta >= 0 {
        first.checked_add_months(months)
    } else {
        first.checked_sub_months(months)
    };
    shifted.unwrap_or(first)
}

fn month_period(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first = first_of_month(date);
    (first, month_last_day(first))
}

/// The first (1..15) or second (16..last) half of the month of `date`.
pub fn half_bounds(date: NaiveDate, half: HalfPart) -> (NaiveDate, NaiveDate) {
    let first = first_of_month(date);
    match half {
        HalfPart::First => (first, first.with_day(15).unwrap_or(first)),
        HalfPart::Second => (first.with_day(16).unwrap_or(first), month_last_day(first)),
    }
}

/// The calendar quarter holding `date`.
pub fn quarter_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first_month = (date.month() - 1) / 3 * 3 + 1;
    let start = NaiveDate::from_ymd_opt(date.year(), first_month, 1).unwrap_or(date);
    (start, month_last_day(shift_months(start, 2)))
}

/// The calendar year holding `date`.
pub fn year_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap_or(date);
    let end = NaiveDate::from_ymd_opt(date.year(), 12, 31).unwrap_or(date);
    (start, end)
}

/// Month keys `YYYY-MM` from `start` to `end`, inclusive; empty when `end < start`.
pub fn months_touched(start: NaiveDate, end: NaiveDate) -> Vec<String> {
    let mut keys = Vec::new();
    if end < start {
        return keys;
    }
    let last = first_of_month(end);
    let mut month = first_of_month(start);
    while month <= last {
        keys.push(month_key(month));
        month = next_month(month);
    }
    keys
}

/// Days of `start..=end` inside each month touched, in the order of
/// `months_touched`.
pub fn days_in_month_within(start: NaiveDate, end: NaiveDate) -> Vec<u32> {
    if end < start {
        return Vec::new();
    }
    let mut days = Vec::new();
    let mut month = first_of_month(start);
    while month <= end {
        let first_day = month.max(start);
        let last_day = month_last_day(month).min(end);
        days.push((last_day - first_day).num_days() as u32 + 1);
        month = next_month(month);
    }
    days
}

/// The half-month `delta` halves away from the half `date` is in (spec 5.1).
fn shifted_half_bounds(date: NaiveDate, delta: i32) -> (NaiveDate, NaiveDate) {
    let in_second = i32::from(date.day() > 15);
    let index = in_second + delta;
    let part = if index.rem_euclid(2) == 0 {
        HalfPart::First
    } else {
        HalfPart::Second
    };
    let months = index.div_euclid(2);
    half_bounds(shift_months(first_of_month(date), months), part)
}

/// Resolve a default period rule from the money date (D2): kind and inclusive bounds.
pub fn resolve_period_rule(
    rule: RuleUnit,
    money_date: NaiveDate,
) -> (PeriodUnit, NaiveDate, NaiveDate) {
    let month_of = |delta: i32| shift_months(first_of_month(money_date), delta);
    match rule {
        RuleUnit::PrevMonth => {
            let (start, end) = month_period(month_of(-1));
            (PeriodUnit::Month, start, end)
        }
        RuleUnit::PrevHalfSecond => {
            let (start, end) = half_bounds(month_of(-1), HalfPart::Second);
            (PeriodUnit::Half, start, end)
        }
        RuleUnit::ThisHalfFirst => {
            let (start, end) = half_bounds(money_date, HalfPart::First);
            (PeriodUnit::Half, start, end)
        }
        RuleUnit::ThisMonth => {
            let (start, end) = month_period(money_date);
            (PeriodUnit::Month, start, end)
        }
        RuleUnit::NextMonth => {
            let (start, end) = month_period(month_of(1));
            (PeriodUnit::Month, start, end)
        }
        RuleUnit::Custom { unit, offset, half } => {
            let delta = i32::from(offset);
            match unit {
                PeriodUnit::Month | PeriodUnit::Range => {
                    let (start, end) = month_period(month_of(delta));
                    (PeriodUnit::Month, start, end)
                }
                PeriodUnit::Half => {
                    let (start, end) = match half {
                        Some(part) => half_bounds(month_of(delta), part),
                        None => shifted_half_bounds(money_date, delta),
                    };
                    (PeriodUnit::Half, start, end)
                }
                PeriodUnit::Quarter => {
                    let (start, end) = quarter_bounds(month_of(delta * 3));
                    (PeriodUnit::Quarter, start, end)
                }
                PeriodUnit::Year => {
                    let (start, end) = year_bounds(month_of(delta * 12));
                    (PeriodUnit::Year, start, end)
                }
            }
        }
    }
}

fn round_down(value: Decimal, places: u32) -> Decimal {
    value.round_dp_with_strategy(places, RoundingStrategy::ToZero)
}

/// Weighted shares: `round_down(amount * weight / total, places)` each, the
/// rest of the minor units to the last one (P6, Q5). Empty without weights.
fn weighted_shares(amount: Amount, weights: &[u32], places: u32) -> Vec<Amount> {
    let total: u64 = weights.iter().map(|weight| u64::from(*weight)).sum();
    let Some(last) = weights.len().checked_sub(1) else {
        return Vec::new();
    };
    if total == 0 {
        let mut shares = vec![Amount::new(Decimal::ZERO); weights.len()];
        shares[last] = amount;
        return shares;
    }
    let mut shares: Vec<Amount> = weights[..last]
        .iter()
        .map(|weight| {
            let share = amount.0 * Decimal::from(*weight) / Decimal::from(total);
            Amount::new(round_down(share, places))
        })
        .collect();
    let given = shares
        .iter()
        .fold(Decimal::ZERO, |sum, share| sum + share.0);
    shares.push(Amount::new(amount.0 - given));
    shares
}

/// `even` shares: `round_down(amount / months, places)` each, the rest to the last month.
pub fn even_shares(amount: Amount, months: usize, places: u32) -> Vec<Amount> {
    weighted_shares(amount, &vec![1; months], places)
}

/// `by_days` shares: weights are the days of the period inside each month.
pub fn by_days_shares(amount: Amount, days: &[u32], places: u32) -> Vec<Amount> {
    weighted_shares(amount, days, places)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }

    fn money(raw: &str) -> Amount {
        Amount::parse(raw).expect("amount")
    }

    fn sum(shares: &[Amount]) -> Amount {
        Amount::new(
            shares
                .iter()
                .fold(Decimal::ZERO, |total, share| total + share.0),
        )
    }

    #[test]
    fn p6_even_hundred_over_three_months() {
        let shares = even_shares(money("100.00"), 3, 2);
        assert_eq!(shares, vec![money("33.33"), money("33.33"), money("33.34")]);
    }

    #[test]
    fn p6_by_days_16_oct_to_15_dec_sums_exactly() {
        let days = days_in_month_within(date(2026, 10, 16), date(2026, 12, 15));
        assert_eq!(days, vec![16, 30, 15]);
        let shares = by_days_shares(money("100.00"), &days, 2);
        assert_eq!(shares, vec![money("26.22"), money("49.18"), money("24.60")]);
        assert_eq!(sum(&shares), money("100.00"));
    }

    #[test]
    fn p4_months_touched_crosses_a_year() {
        assert_eq!(
            months_touched(date(2026, 11, 20), date(2027, 1, 2)),
            vec!["2026-11", "2026-12", "2027-01"]
        );
        assert_eq!(
            months_touched(date(2026, 3, 1), date(2026, 3, 15)),
            vec!["2026-03"]
        );
    }

    #[test]
    fn p2_bounds_of_half_quarter_year() {
        assert_eq!(
            half_bounds(date(2026, 2, 9), HalfPart::Second),
            (date(2026, 2, 16), date(2026, 2, 28))
        );
        assert_eq!(
            half_bounds(date(2028, 2, 9), HalfPart::Second),
            (date(2028, 2, 16), date(2028, 2, 29))
        );
        assert_eq!(
            quarter_bounds(date(2026, 11, 5)),
            (date(2026, 10, 1), date(2026, 12, 31))
        );
        assert_eq!(
            quarter_bounds(date(2026, 1, 1)),
            (date(2026, 1, 1), date(2026, 3, 31))
        );
        assert_eq!(
            year_bounds(date(2026, 6, 6)),
            (date(2026, 1, 1), date(2026, 12, 31))
        );
    }

    #[test]
    fn d2_january_resolves_to_previous_december() {
        assert_eq!(
            resolve_period_rule(RuleUnit::PrevMonth, date(2027, 1, 10)),
            (PeriodUnit::Month, date(2026, 12, 1), date(2026, 12, 31))
        );
    }

    #[test]
    fn d2_31_january_prev_month_is_december() {
        assert_eq!(
            resolve_period_rule(RuleUnit::PrevMonth, date(2027, 1, 31)),
            (PeriodUnit::Month, date(2026, 12, 1), date(2026, 12, 31))
        );
    }

    #[test]
    fn d2_march_prev_month_is_february_leap_and_non_leap() {
        assert_eq!(
            resolve_period_rule(RuleUnit::PrevMonth, date(2027, 3, 31)),
            (PeriodUnit::Month, date(2027, 2, 1), date(2027, 2, 28))
        );
        assert_eq!(
            resolve_period_rule(RuleUnit::PrevMonth, date(2028, 3, 31)),
            (PeriodUnit::Month, date(2028, 2, 1), date(2028, 2, 29))
        );
    }

    #[test]
    fn d2_prev_half_second_on_5_nov_is_16_to_31_oct() {
        assert_eq!(
            resolve_period_rule(RuleUnit::PrevHalfSecond, date(2026, 11, 5)),
            (PeriodUnit::Half, date(2026, 10, 16), date(2026, 10, 31))
        );
    }

    #[test]
    fn d2_this_half_first_this_month_next_month() {
        let money_date = date(2026, 12, 20);
        assert_eq!(
            resolve_period_rule(RuleUnit::ThisHalfFirst, money_date),
            (PeriodUnit::Half, date(2026, 12, 1), date(2026, 12, 15))
        );
        assert_eq!(
            resolve_period_rule(RuleUnit::ThisMonth, money_date),
            (PeriodUnit::Month, date(2026, 12, 1), date(2026, 12, 31))
        );
        assert_eq!(
            resolve_period_rule(RuleUnit::NextMonth, money_date),
            (PeriodUnit::Month, date(2027, 1, 1), date(2027, 1, 31))
        );
    }

    #[test]
    fn d2_custom_quarter_and_year_offsets() {
        let custom = |unit, offset| RuleUnit::Custom {
            unit,
            offset,
            half: None,
        };
        assert_eq!(
            resolve_period_rule(custom(PeriodUnit::Quarter, -1), date(2027, 2, 10)),
            (PeriodUnit::Quarter, date(2026, 10, 1), date(2026, 12, 31))
        );
        assert_eq!(
            resolve_period_rule(custom(PeriodUnit::Year, -1), date(2027, 2, 10)),
            (PeriodUnit::Year, date(2026, 1, 1), date(2026, 12, 31))
        );
        assert_eq!(
            resolve_period_rule(
                RuleUnit::Custom {
                    unit: PeriodUnit::Half,
                    offset: -1,
                    half: Some(HalfPart::First)
                },
                date(2027, 1, 20)
            ),
            (PeriodUnit::Half, date(2026, 12, 1), date(2026, 12, 15))
        );
    }

    #[test]
    fn custom_half_offset_steps_in_half_month_units() {
        let half = |offset| RuleUnit::Custom {
            unit: PeriodUnit::Half,
            offset,
            half: None,
        };
        assert_eq!(
            resolve_period_rule(half(-1), date(2026, 11, 20)),
            (PeriodUnit::Half, date(2026, 11, 1), date(2026, 11, 15))
        );
        assert_eq!(
            resolve_period_rule(half(-1), date(2026, 11, 10)),
            (PeriodUnit::Half, date(2026, 10, 16), date(2026, 10, 31))
        );
        assert_eq!(
            resolve_period_rule(half(1), date(2026, 12, 31)),
            (PeriodUnit::Half, date(2027, 1, 1), date(2027, 1, 15))
        );
    }
}
