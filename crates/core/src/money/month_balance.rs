//! The month's balance (redesign «Money · month's balance»): what each month
//! left — income less fixed and daily spending — and, when the user carries the
//! unspent over, what has piled up since they turned it on. One formula for the
//! dashboard, Spending, the calendar and the balance tab.

use std::collections::BTreeMap;

use crate::Amount;
use chrono::{Duration, NaiveDate};

use crate::calendar::{self, first_of_month, next_month};
use crate::error::{Error, Result};

use crate::kernel::books::Books;

/// One month of the balance.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthBalanceRow {
    /// `YYYY-MM`.
    pub month: String,
    pub income: Amount,
    /// Fixed and daily spending together.
    pub spent: Amount,
    /// `income − spent`; negative when the month went into the red.
    pub left: Amount,
    /// What the months before brought in: the sum of their `left` since
    /// `carry_since`; zero when nothing is carried.
    pub carried_in: Amount,
    /// `left + carried_in`.
    pub total: Amount,
    /// The month is not over yet.
    pub current: bool,
}

/// One day of the period's last month: what was there at its end.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthBalanceDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// The month's balance at the end of the day; `None` for days ahead.
    pub balance: Option<Amount>,
    /// Where the balance goes spending the day's allowance; from today on.
    pub forecast: Option<Amount>,
}

/// The biggest income or spending of the last month: what the chart labels
/// ("+1 575 $ Salary", "−36 $ Groceries").
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthBalanceMark {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Positive, in the report currency.
    pub amount: Amount,
    /// The operation's first category key; empty without one.
    pub category: String,
}

/// The months of a period with what each left and what carried over.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MonthBalance {
    pub currency: String,
    /// `YYYY-MM` the unspent money is carried from; `None` = not carried.
    pub carry_since: Option<String>,
    /// Oldest first.
    pub rows: Vec<MonthBalanceRow>,
    /// Average `spent` of the finished months among `rows`.
    pub average_spent: Amount,
    /// The whole period: income, spent and left summed over `rows`; `carried_in`
    /// and `total` are those of its last month (what is there now).
    pub total: MonthBalanceRow,
    /// The last month day by day, ending at its `total`.
    pub days: Vec<MonthBalanceDay>,
    /// The day's allowance the forecast spends (the calendar's); `None` without one.
    pub per_day: Option<Amount>,
    /// The month's biggest income, for the chart's label.
    #[serde(default)]
    pub income_mark: Option<MonthBalanceMark>,
    /// The month's biggest spending, for the chart's label.
    #[serde(default)]
    pub spent_mark: Option<MonthBalanceMark>,
}

/// One month of the cash flow, as the report counts it.
#[derive(Debug, Clone, PartialEq)]
pub struct CashMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub income: Amount,
    pub fixed: Amount,
    pub daily: Amount,
}

/// Whether a flow of the last month earns or spends; the other classes do not
/// move the balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    Income,
    Spending,
}

/// One operation of the last month, already in the report currency.
#[derive(Debug, Clone, PartialEq)]
pub struct DayFlow {
    pub date: NaiveDate,
    pub value: Amount,
    pub kind: FlowKind,
    /// The operation's first category key; empty without one.
    pub category: String,
}

/// The reports area's answer for a span of months (OD6): the cash flow of each
/// month and the flows of the last one.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportFlows {
    pub currency: String,
    pub today: NaiveDate,
    pub months: Vec<NaiveDate>,
    /// One per month, in the order of `months`.
    pub cash: Vec<CashMonth>,
    /// The flows of the last of `months`.
    pub last_month: Vec<DayFlow>,
}

/// What the month's balance reads from the reports area, which has not moved
/// yet (OD6). The vault implements it in the scenario, holding the filter, the
/// moment and the zone; a test may fake it.
pub trait ReportSource {
    /// The months of the period the filter shows, oldest first.
    fn shown_months(&self) -> Result<Vec<NaiveDate>>;
    /// The flows of the months `from` to `to` (`YYYY-MM`), the filter's other terms kept.
    fn report(&self, from: &str, to: &str) -> Result<ReportFlows>;
    /// The calendar's day allowance of `month`, when it is in `currency`.
    fn allowance(&self, month: &str, currency: &str) -> Result<Option<Amount>>;
}

type BalanceDays = (
    Vec<MonthBalanceDay>,
    Option<Amount>,
    Option<MonthBalanceMark>,
    Option<MonthBalanceMark>,
);

/// The balance of the filter's months (the last twelve by default). The
/// carry starts at `settings.carry_since`, even before the period does.
pub fn month_balance(books: &Books, source: &dyn ReportSource) -> Result<MonthBalance> {
    let shown = source.shown_months()?;
    let first = *shown
        .first()
        .ok_or_else(|| Error::InvalidInput("an empty period".into()))?;
    let since = books
        .doc()
        .settings
        .carry_since
        .as_deref()
        .and_then(calendar::parse_month);
    // Months before the period still feed the carry: count from its start.
    let start = since.map_or(first, |month| month.min(first));
    let last_shown = shown.last().copied().unwrap_or(first);
    let report = source.report(
        &calendar::month_key(start),
        &calendar::month_key(last_shown),
    )?;
    let this_month = first_of_month(report.today);

    let mut carried = Amount::ZERO;
    let mut rows = Vec::new();
    for (row, month) in report.cash.iter().zip(&report.months) {
        let spent = row.fixed + row.daily;
        let left = row.income - spent;
        let carries = since.is_some_and(|since| *month >= since);
        let carried_in = if carries { carried } else { Amount::ZERO };
        if carries {
            carried += left;
        }
        if *month >= first {
            rows.push(MonthBalanceRow {
                month: row.month.clone(),
                income: row.income,
                spent,
                left,
                carried_in,
                total: left + carried_in,
                current: *month == this_month,
            });
        }
    }
    let finished: Vec<Amount> = rows
        .iter()
        .filter(|row| !row.current)
        .map(|row| row.spent)
        .collect();
    let average_spent = if finished.is_empty() {
        Amount::ZERO
    } else {
        (finished
            .iter()
            .fold(Amount::ZERO, |sum, value| sum + *value)
            / Amount::from(finished.len() as u32))
        .round(2)
    };
    let mut total = MonthBalanceRow::default();
    for row in &rows {
        total.income += row.income;
        total.spent += row.spent;
        total.left += row.left;
    }
    if let Some(last) = rows.last() {
        total.month = last.month.clone();
        total.carried_in = last.carried_in;
        total.total = last.total;
        total.current = last.current;
    }
    let (days, per_day, income_mark, spent_mark) = match rows.last() {
        Some(last) => balance_days(source, &report, last)?,
        None => (Vec::new(), None, None, None),
    };
    Ok(MonthBalance {
        currency: report.currency,
        carry_since: since.map(calendar::month_key),
        rows,
        average_spent,
        total,
        days,
        per_day,
        income_mark,
        spent_mark,
    })
}

/// The month's balance day by day from the same flows as its row, counted back
/// from its `total` so the last real day ends exactly there. Ahead, the
/// calendar's day allowance is spent each day (when it is in the same currency).
fn balance_days(
    source: &dyn ReportSource,
    report: &ReportFlows,
    row: &MonthBalanceRow,
) -> Result<BalanceDays> {
    let first = calendar::parse_month(&row.month)
        .ok_or_else(|| Error::InvalidInput(format!("month {}: expected YYYY-MM", row.month)))?;
    let mut net: BTreeMap<NaiveDate, Amount> = BTreeMap::new();
    let mut income_mark: Option<MonthBalanceMark> = None;
    let mut spent_mark: Option<MonthBalanceMark> = None;
    for flow in &report.last_month {
        let value = flow.value;
        let (sign, mark) = match flow.kind {
            FlowKind::Income => (value, &mut income_mark),
            FlowKind::Spending => (-value, &mut spent_mark),
        };
        if mark.as_ref().is_none_or(|best| value.abs() > best.amount) {
            *mark = Some(MonthBalanceMark {
                date: flow.date.format("%Y-%m-%d").to_string(),
                amount: value.abs(),
                category: flow.category.clone(),
            });
        }
        *net.entry(flow.date).or_default() += sign;
    }
    let per_day = if row.current {
        source.allowance(&row.month, &report.currency)?
    } else {
        None
    };
    let mut dates = Vec::new();
    let mut date = first;
    while date < next_month(first) {
        dates.push(date);
        date += Duration::days(1);
    }
    let mut balances = vec![None; dates.len()];
    let mut after = Amount::ZERO;
    for (index, date) in dates.iter().enumerate().rev() {
        if *date > report.today {
            continue;
        }
        balances[index] = Some(row.total - after);
        after += net.get(date).copied().unwrap_or_default();
    }
    let last_real = balances.iter().rposition(Option::is_some);
    let days = dates
        .iter()
        .enumerate()
        .map(|(index, date)| MonthBalanceDay {
            date: date.format("%Y-%m-%d").to_string(),
            balance: balances[index],
            forecast: match (last_real, per_day) {
                (Some(last), Some(rate)) if index >= last => {
                    Some(row.total - rate * Amount::from((index - last) as u32))
                }
                _ => None,
            },
        })
        .collect();
    Ok((days, per_day, income_mark, spent_mark))
}

/// The month a carry starts from, as stored (`YYYY-MM`); blank or `None` stops
/// the carry.
pub fn parse_carry_since(month: Option<&str>) -> Result<Option<String>> {
    match month.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) => calendar::parse_month(text)
            .map(calendar::month_key)
            .map(Some)
            .ok_or_else(|| Error::InvalidInput(format!("month {text}: expected YYYY-MM"))),
    }
}
