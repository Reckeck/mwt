//! What an operation is «for» when it is saved (spec op-period D3, D4, P6, P8): the period a
//! rule gives a new operation, and the spread shares kept in step with the amount and the period.
//! Pure; the usecase reads the vault and calls these.

use chrono::NaiveDate;
use mwt_base::Amount;
use mwt_base::calendar::period::{
    HalfPart, PeriodUnit, RuleUnit, by_days_shares, days_in_month_within, even_shares,
    months_touched, resolve_period_rule,
};
use mwt_model::{MonthShare, OpPeriod, PeriodKind, PeriodRule, RuleHalf, Spread, SpreadMode};

fn unit_of(kind: PeriodKind) -> PeriodUnit {
    match kind {
        PeriodKind::Month => PeriodUnit::Month,
        PeriodKind::Half => PeriodUnit::Half,
        PeriodKind::Quarter => PeriodUnit::Quarter,
        PeriodKind::Year => PeriodUnit::Year,
        PeriodKind::Range => PeriodUnit::Range,
    }
}

fn kind_of(unit: PeriodUnit) -> PeriodKind {
    match unit {
        PeriodUnit::Month => PeriodKind::Month,
        PeriodUnit::Half => PeriodKind::Half,
        PeriodUnit::Quarter => PeriodKind::Quarter,
        PeriodUnit::Year => PeriodKind::Year,
        PeriodUnit::Range => PeriodKind::Range,
    }
}

fn rule_unit(rule: PeriodRule) -> RuleUnit {
    match rule {
        PeriodRule::PrevMonth => RuleUnit::PrevMonth,
        PeriodRule::PrevHalfSecond => RuleUnit::PrevHalfSecond,
        PeriodRule::ThisHalfFirst => RuleUnit::ThisHalfFirst,
        PeriodRule::ThisMonth => RuleUnit::ThisMonth,
        PeriodRule::NextMonth => RuleUnit::NextMonth,
        PeriodRule::Custom { kind, offset, half } => RuleUnit::Custom {
            unit: unit_of(kind),
            offset,
            half: half.map(|part| match part {
                RuleHalf::First => HalfPart::First,
                RuleHalf::Second => HalfPart::Second,
            }),
        },
    }
}

/// The period a rule gives an operation of `money_date` (D2).
pub fn period_of_rule(rule: PeriodRule, money_date: NaiveDate) -> OpPeriod {
    let (unit, start, end) = resolve_period_rule(rule_unit(rule), money_date);
    OpPeriod {
        kind: kind_of(unit),
        start: start.format("%Y-%m-%d").to_string(),
        end: end.format("%Y-%m-%d").to_string(),
    }
}

/// The period of a new operation (D3): the explicit input, else the template's rule, else the
/// rule of the main category, else none. `no_period_rule` with no explicit period means
/// «really none». The result is stored; a rule is never read again (D4).
pub fn resolve_period_for_new(
    explicit: Option<&OpPeriod>,
    no_period_rule: bool,
    template_rule: Option<PeriodRule>,
    category_rule: Option<PeriodRule>,
    money_date: NaiveDate,
) -> Option<OpPeriod> {
    if let Some(period) = explicit {
        return Some(period.clone());
    }
    if no_period_rule {
        return None;
    }
    template_rule
        .or(category_rule)
        .map(|rule| period_of_rule(rule, money_date))
}

/// What the stored operation looked like before an edit (P8).
pub struct SpreadBefore<'a> {
    pub amount: Amount,
    pub period: Option<&'a OpPeriod>,
    pub spread: Option<&'a Spread>,
}

fn parse_day(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

fn shares_over(months: Vec<String>, amounts: Vec<Amount>) -> Vec<MonthShare> {
    months
        .into_iter()
        .zip(amounts)
        .map(|(month, amount)| MonthShare { month, amount })
        .collect()
}

/// A manual spread whose amount or period moved loses its shares (P8): the save is then
/// refused by validation until new ones come. Shares that differ from the stored ones are new.
fn settle_manual(
    mut spread: Spread,
    period: &OpPeriod,
    amount: Amount,
    before: Option<&SpreadBefore<'_>>,
) -> Spread {
    let stale = before.is_some_and(|stored| {
        stored.spread.is_some_and(|old| {
            old.mode == SpreadMode::Manual
                && old.shares == spread.shares
                && (stored.amount != amount || stored.period != Some(period))
        })
    });
    if stale {
        spread.shares.clear();
    }
    spread
}

/// The spread to store (P6, P8): a single-month period drops it, `even` and `by_days` are
/// derived afresh in `places` minor digits, a `manual` one is kept unless it went stale.
/// A spread with no readable period is returned as is; validation refuses it.
pub fn settle_spread(
    period: Option<&OpPeriod>,
    spread: Option<Spread>,
    amount: Amount,
    places: u32,
    before: Option<&SpreadBefore<'_>>,
) -> Option<Spread> {
    let spread = spread?;
    let Some(period) = period else {
        return Some(spread);
    };
    let (Some(start), Some(end)) = (parse_day(&period.start), parse_day(&period.end)) else {
        return Some(spread);
    };
    let months = months_touched(start, end);
    if months.len() < 2 {
        return None;
    }
    let shares = match spread.mode {
        SpreadMode::Even => even_shares(amount, months.len(), places),
        SpreadMode::ByDays => by_days_shares(amount, &days_in_month_within(start, end), places),
        SpreadMode::Manual => return Some(settle_manual(spread, period, amount, before)),
    };
    Some(Spread {
        mode: spread.mode,
        shares: shares_over(months, shares),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, date).expect("date")
    }

    fn period(kind: PeriodKind, start: &str, end: &str) -> OpPeriod {
        OpPeriod {
            kind,
            start: start.into(),
            end: end.into(),
        }
    }

    #[test]
    fn d3_precedence_explicit_then_template_then_category_then_none() {
        let today = day(2026, 11, 5);
        let own = period(PeriodKind::Month, "2026-01-01", "2026-01-31");
        let pick = |explicit, none, template, category| {
            resolve_period_for_new(explicit, none, template, category, today)
        };
        let both = (Some(PeriodRule::PrevMonth), Some(PeriodRule::NextMonth));
        assert_eq!(pick(Some(&own), false, both.0, both.1), Some(own.clone()));
        assert_eq!(pick(Some(&own), true, both.0, both.1), Some(own.clone()));
        assert_eq!(
            pick(None, false, both.0, both.1),
            Some(period(PeriodKind::Month, "2026-10-01", "2026-10-31"))
        );
        assert_eq!(
            pick(None, false, None, both.1),
            Some(period(PeriodKind::Month, "2026-12-01", "2026-12-31"))
        );
        assert_eq!(pick(None, true, both.0, both.1), None);
        assert_eq!(pick(None, false, None, None), None);
    }

    #[test]
    fn d2_prev_half_second_in_january_is_december() {
        assert_eq!(
            period_of_rule(PeriodRule::PrevHalfSecond, day(2026, 1, 5)),
            period(PeriodKind::Half, "2025-12-16", "2025-12-31")
        );
    }

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).expect("amount")
    }

    fn spread(mode: SpreadMode, shares: &[(&str, &str)]) -> Spread {
        Spread {
            mode,
            shares: shares
                .iter()
                .map(|(month, raw)| MonthShare {
                    month: (*month).into(),
                    amount: amount(raw),
                })
                .collect(),
        }
    }

    #[test]
    fn p6_even_spread_is_derived_with_the_rest_in_the_last_month() {
        let quarter = period(PeriodKind::Quarter, "2026-10-01", "2026-12-31");
        let settled = settle_spread(
            Some(&quarter),
            Some(spread(SpreadMode::Even, &[])),
            amount("100.00"),
            2,
            None,
        );
        assert_eq!(
            settled,
            Some(spread(
                SpreadMode::Even,
                &[
                    ("2026-10", "33.33"),
                    ("2026-11", "33.33"),
                    ("2026-12", "33.34")
                ]
            ))
        );
    }

    #[test]
    fn p8_single_month_period_drops_the_spread() {
        let month = period(PeriodKind::Month, "2026-10-01", "2026-10-31");
        let settled = settle_spread(
            Some(&month),
            Some(spread(SpreadMode::Even, &[])),
            amount("10"),
            2,
            None,
        );
        assert_eq!(settled, None);
    }
}
