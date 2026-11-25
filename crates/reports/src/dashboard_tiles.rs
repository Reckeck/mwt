//! Figures for the existing dashboard widgets (plan H3): the month's pace for
//! "Daily budget" and "This month", the capital's trend for "Net worth".

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::comparison::{BaseKind, Comparison};
use mwt_base::Amount;
use mwt_base::calendar::{self, Calendar};
use mwt_model::DashPeriod;
use mwt_money::books::Books;
use mwt_money::portfolio::history_series;
use mwt_money::worth::FlowClass;
use mwt_planning::budget::budget_currency;
use mwt_planning::flows::{Basis, flows_for_basis};
use mwt_planning::period::{PeriodWindow, months_back, period_window};

/// How many growing categories "This month" names.
const TOP_GROWTH: usize = 3;

/// A spending category that grew against the same date of the last month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CategoryGrowth {
    /// `Category::key`.
    pub category: String,
    pub value: Amount,
    pub base: Amount,
}

/// The month so far against the previous month to the same date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MonthPace {
    pub currency: String,
    /// `YYYY-MM`.
    pub month: String,
    /// Today's day of the month and the month's length.
    pub day: u32,
    pub days: u32,
    pub income: Comparison,
    /// Mandatory and everyday spending.
    pub spent: Comparison,
    /// Income less spending.
    pub saved: Comparison,
    /// Income times the savings rate.
    pub savings_plan: Amount,
    /// What the plan lets spend by today: income less the savings plan, spread over the days.
    pub spend_plan_to_date: Option<Amount>,
    /// Spending at today's pace by the month's end.
    pub forecast_spent: Amount,
    /// Up to three categories that grew most, largest growth first.
    pub growth: Vec<CategoryGrowth>,
}

/// The capital over a period: now against the period's first day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NetWorthTrend {
    pub currency: String,
    pub period: DashPeriod,
    /// `YYYY-MM-DD`.
    pub start: String,
    /// The value of the holdings today; the base is the value when the period began.
    pub change: Comparison,
    /// Value per step of the period (days of a month, weeks of a quarter, months of a year).
    pub series: Vec<Amount>,
}

#[derive(Default)]
struct PaceTotals {
    income: Amount,
    spent: Amount,
    by_category: BTreeMap<String, Amount>,
}

fn pace_totals(
    books: &Books,
    cal: &Calendar,
    currency: &str,
    from: NaiveDate,
    to: NaiveDate,
    basis: Basis,
) -> PaceTotals {
    let mut totals = PaceTotals::default();
    for flow in flows_for_basis(
        books,
        cal,
        currency,
        cal.start_of(from),
        cal.start_of(to + Duration::days(1)),
        basis,
    ) {
        match flow.class {
            FlowClass::Income => totals.income += flow.value,
            FlowClass::Fixed | FlowClass::Daily => {
                totals.spent += flow.value;
                if let Some(key) = flow.categories.first() {
                    *totals.by_category.entry(key.clone()).or_default() += flow.value;
                }
            }
            _ => {}
        }
    }
    totals
}

/// The current month's pace (plan H3: "Daily budget" and "This month"), by the `period` basis.
pub fn month_pace(books: &Books, now: i64, zone: &str) -> MonthPace {
    month_pace_by(books, Basis::Period, now, zone)
}

/// [`month_pace`] with the basis of the month figures chosen.
pub fn month_pace_by(books: &Books, basis: Basis, now: i64, zone: &str) -> MonthPace {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let currency = budget_currency(books);
    let first = calendar::first_of_month(today);
    let prev_first = months_back(first, 1);
    let prev_today = months_back(today, 1);
    let current = pace_totals(books, &cal, &currency, first, today, basis);
    let previous = pace_totals(books, &cal, &currency, prev_first, prev_today, basis);
    let has_history = books.doc().transactions.iter().any(|tx| {
        mwt_base::time::unix_secs(&tx.meta().timestamp)
            .is_some_and(|secs| cal.date_of(secs) <= prev_today)
    });
    let base = |value: Amount| has_history.then_some(value);

    let day = today.day();
    let days = calendar::days_in_month(today);
    let ratio = Amount::from(day) / Amount::from(days);
    let savings_plan = current.income * books.doc().settings.savings_rate;
    let spend_plan_to_date = current
        .income
        .is_positive()
        .then(|| ((current.income - savings_plan) * ratio).round(2));
    let forecast_spent = (current.spent / ratio).round(2);

    let mut growth: Vec<CategoryGrowth> = current
        .by_category
        .iter()
        .map(|(key, value)| CategoryGrowth {
            category: key.clone(),
            value: *value,
            base: previous
                .by_category
                .get(key)
                .copied()
                .unwrap_or(Amount::ZERO),
        })
        .filter(|row| row.value > row.base)
        .collect();
    if has_history {
        growth.sort_by_key(|row| std::cmp::Reverse(row.value - row.base));
        growth.truncate(TOP_GROWTH);
    } else {
        growth.clear();
    }

    MonthPace {
        currency,
        month: calendar::month_key(first),
        day,
        days,
        income: Comparison::new(current.income, base(previous.income), BaseKind::PrevPeriod),
        spent: Comparison::new(current.spent, base(previous.spent), BaseKind::PrevPeriod),
        saved: Comparison::new(
            current.income - current.spent,
            base(previous.income - previous.spent),
            BaseKind::PrevPeriod,
        ),
        savings_plan: savings_plan.round(2),
        spend_plan_to_date,
        forecast_spent,
        growth,
    }
}

/// The holdings' value over a period (plan H3: "Net worth").
pub fn net_worth_trend(books: &Books, period: DashPeriod, now: i64, zone: &str) -> NetWorthTrend {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let currency = books.base_and_fx().0.to_owned();
    let PeriodWindow { start, .. } = period_window(period, today);
    let step = |date: NaiveDate| -> usize {
        match period {
            DashPeriod::Month => (date - start).num_days() as usize,
            DashPeriod::Quarter => ((date - start).num_days() / 7) as usize,
            DashPeriod::Year => date.month0() as usize,
        }
    };
    let slots = step(today) + 1;
    let history = history_series(books, &currency);
    let mut opening = None;
    let mut values: Vec<Option<Amount>> = vec![None; slots];
    for point in &history {
        let date = cal.date_of(point.ts_ms.div_euclid(1000));
        if date < start {
            opening = Some(point.value_base);
        } else if let Some(slot) = values.get_mut(step(date)) {
            *slot = Some(point.value_base);
        }
    }
    let mut last = opening.unwrap_or(Amount::ZERO);
    let series: Vec<Amount> = values
        .into_iter()
        .map(|value| {
            last = value.unwrap_or(last);
            last.round(2)
        })
        .collect();
    let value = series.last().copied().unwrap_or(Amount::ZERO);
    NetWorthTrend {
        currency,
        period,
        start: start.format("%Y-%m-%d").to_string(),
        change: Comparison::new(
            value,
            opening.map(|open| open.round(2)),
            BaseKind::PrevPeriod,
        ),
        series,
    }
}
