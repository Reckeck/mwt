//! Dashboard figures with their comparison (plan H1/H2): one call per widget,
//! the period and the same span of the previous period computed here.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::comparison::{BaseKind, Comparison};
use super::overview::{DayStatus, calendar_month};
use mwt_base::Amount;
use mwt_base::calendar::{self, Calendar};
use mwt_model::DashPeriod;
use mwt_money::books::Books;
use mwt_money::worth::FlowClass;
use mwt_planning::budget::budget_currency;
use mwt_planning::flows::{Basis, flows_for_basis};
use mwt_planning::period::{PeriodWindow, period_window};

/// One day of the "Spending" week strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpendingDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Everyday spending of the day.
    pub daily: Amount,
    pub over: bool,
}

/// The "Spending" widget: the period, the week and a series for the sparkline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpendingOverview {
    pub currency: String,
    pub period: DashPeriod,
    /// First day of the period, `YYYY-MM-DD`.
    pub start: String,
    /// Mandatory and everyday spending so far against the previous period to the same date.
    pub spent: Comparison,
    /// Everyday spending alone, the same way.
    pub daily: Comparison,
    /// Spending per step of the period: days of a month, weeks of a quarter, months of a year.
    pub series: Vec<Amount>,
    /// The last seven days, oldest first.
    pub week: Vec<SpendingDay>,
    /// The last seven days against the seven before them.
    pub week_total: Comparison,
    /// Everyday allowance of a day, when the budget has one.
    pub allowance: Option<Amount>,
}

struct Totals {
    spent: Amount,
    daily: Amount,
}

fn spending_totals(
    books: &Books,
    cal: &Calendar,
    currency: &str,
    from: NaiveDate,
    to: NaiveDate,
    basis: Basis,
) -> (Totals, Vec<(NaiveDate, Amount)>) {
    let flows = flows_for_basis(
        books,
        cal,
        currency,
        cal.start_of(from),
        cal.start_of(to + Duration::days(1)),
        basis,
    );
    let mut totals = Totals {
        spent: Amount::ZERO,
        daily: Amount::ZERO,
    };
    let mut days = Vec::new();
    for flow in flows {
        if !matches!(flow.class, FlowClass::Daily | FlowClass::Fixed) {
            continue;
        }
        totals.spent += flow.value;
        if flow.class == FlowClass::Daily {
            totals.daily += flow.value;
        }
        // The series is by day; a period flow paid outside the window has no slot in it.
        if flow.date >= from && flow.date <= to {
            days.push((flow.date, flow.value));
        }
    }
    (totals, days)
}

/// Spending for the dashboard's "Spending" (plan H2), by the `period` basis.
pub fn spending_overview(
    books: &Books,
    period: DashPeriod,
    now: i64,
    zone: &str,
) -> SpendingOverview {
    spending_overview_by(books, period, Basis::Period, now, zone)
}

/// [`spending_overview`] with the basis of the period figures chosen. The
/// last-seven-days strip and its total always read the money date (R3).
pub fn spending_overview_by(
    books: &Books,
    period: DashPeriod,
    basis: Basis,
    now: i64,
    zone: &str,
) -> SpendingOverview {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let currency = budget_currency(books);
    let PeriodWindow {
        start,
        end,
        prev_start,
        prev_end,
    } = period_window(period, today);
    let (current, days) = spending_totals(books, &cal, &currency, start, end, basis);
    let (previous, _) = spending_totals(books, &cal, &currency, prev_start, prev_end, basis);
    let has_history = books.doc().transactions.iter().any(|tx| {
        mwt_base::time::unix_secs(&tx.meta().timestamp)
            .is_some_and(|secs| cal.date_of(secs) <= prev_end)
    });
    let base = |value: Amount| has_history.then_some(value);

    let step = |date: NaiveDate| -> usize {
        match period {
            DashPeriod::Month => (date - start).num_days() as usize,
            DashPeriod::Quarter => ((date - start).num_days() / 7) as usize,
            DashPeriod::Year => date.month0() as usize,
        }
    };
    let mut series = vec![Amount::ZERO; step(end) + 1];
    for (date, value) in &days {
        if let Some(slot) = series.get_mut(step(*date)) {
            *slot += *value;
        }
    }

    let week_start = today - Duration::days(6);
    let (week_now, _) =
        spending_totals(books, &cal, &currency, week_start, today, Basis::MoneyDate);
    let (week_before, _) = spending_totals(
        books,
        &cal,
        &currency,
        week_start - Duration::days(7),
        week_start - Duration::days(1),
        Basis::MoneyDate,
    );
    let months = [
        calendar::first_of_month(week_start),
        calendar::first_of_month(today),
    ];
    let mut by_day = std::collections::HashMap::new();
    let mut allowance = None;
    for first in months.iter().collect::<std::collections::BTreeSet<_>>() {
        if let Ok(month) = calendar_month(books, &calendar::month_key(*first), now, zone) {
            allowance = month.allowance.or(allowance);
            for day in month.days {
                by_day.insert(day.date.clone(), day);
            }
        }
    }
    let week = (0..7)
        .map(|offset| {
            let date = (week_start + Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string();
            let day = by_day.get(&date);
            SpendingDay {
                daily: day.map_or(Amount::ZERO, |day| day.daily),
                over: day.is_some_and(|day| day.status == DayStatus::Over),
                date,
            }
        })
        .collect();

    SpendingOverview {
        currency,
        period,
        start: start.format("%Y-%m-%d").to_string(),
        spent: Comparison::new(current.spent, base(previous.spent), BaseKind::PrevPeriod),
        daily: Comparison::new(current.daily, base(previous.daily), BaseKind::PrevPeriod),
        series,
        week,
        week_total: Comparison::new(
            week_now.spent,
            base(week_before.spent),
            BaseKind::PrevPeriod,
        ),
        allowance,
    }
}
