//! Properties of the budget periods and of the month identities.

use chrono::{Datelike, Duration, Weekday};
use proptest::prelude::*;

use super::budget::{MonthSummary, period_dates, settle_month};
use crate::Amount;
use crate::model::planning::BudgetPeriod;
use crate::testkit::props::{amount, day, money};

fn budget_period() -> impl Strategy<Value = BudgetPeriod> {
    prop_oneof![
        Just(BudgetPeriod::Day),
        Just(BudgetPeriod::Week),
        Just(BudgetPeriod::Month)
    ]
}

proptest! {
    #[test]
    fn period_dates_contain_today(kind in budget_period(), today in day()) {
        let (start, end) = period_dates(kind, today);
        prop_assert!(start <= today);
        prop_assert!(today < end);
    }

    #[test]
    fn a_week_starts_on_monday_and_spans_seven_days(today in day()) {
        let (start, end) = period_dates(BudgetPeriod::Week, today);
        prop_assert_eq!(start.weekday(), Weekday::Mon);
        prop_assert_eq!(end - start, Duration::days(7));
    }

    #[test]
    fn a_month_runs_from_its_first_day_to_the_next_first(today in day()) {
        let (start, end) = period_dates(BudgetPeriod::Month, today);
        prop_assert_eq!(start.day(), 1);
        prop_assert_eq!(end.day(), 1);
        prop_assert_eq!(start.month(), today.month());
    }

    #[test]
    fn settle_month_keeps_its_identities(
        income in money(),
        fixed in money(),
        daily in money(),
        rate in amount(0..=100, 2),
    ) {
        let mut summary = MonthSummary { income, fixed, daily, ..MonthSummary::default() };
        settle_month(&mut summary, rate);
        prop_assert_eq!(summary.savings_plan, income * rate);
        prop_assert_eq!(summary.savings_fact, income - fixed - daily);
        prop_assert_eq!(summary.deviation, summary.savings_fact - summary.savings_plan);
        if income.is_positive() {
            let expected = (summary.savings_fact / income * Amount::from(100)).round(1);
            prop_assert_eq!(summary.saved_pct, expected);
            prop_assert!(summary.saved_pct <= Amount::from(100));
        } else {
            prop_assert_eq!(summary.saved_pct, Amount::ZERO);
        }
    }
}
