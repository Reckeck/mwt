//! The month's balance (redesign «Money · month's balance»): what each month
//! left — income less fixed and daily spending — and, when the user carries the
//! unspent over, what has piled up since they turned it on. One formula for the
//! dashboard, Spending, the calendar and the balance tab.

use std::collections::BTreeMap;

use crate::Amount;
use chrono::{Duration, NaiveDate};

use crate::calendar::{self, first_of_month, next_month};
use crate::error::{Error, Result};

use super::UnlockedVault;
use super::reports::{Ctx, ReportFilter};
use super::worth::FlowClass;

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

type BalanceDays = (
    Vec<MonthBalanceDay>,
    Option<Amount>,
    Option<MonthBalanceMark>,
    Option<MonthBalanceMark>,
);

impl UnlockedVault {
    /// The balance of the filter's months (the last twelve by default). The
    /// carry starts at `settings.carry_since`, even before the period does.
    pub fn month_balance(
        &self,
        filter: &ReportFilter,
        now: i64,
        zone: &str,
        inflation: &BTreeMap<String, Amount>,
    ) -> Result<MonthBalance> {
        let shown = self.report_ctx(filter, now, zone, inflation)?;
        let first = *shown
            .months
            .first()
            .ok_or_else(|| Error::InvalidInput("an empty period".into()))?;
        let since = self
            .doc
            .settings
            .carry_since
            .as_deref()
            .and_then(calendar::parse_month);
        // Months before the period still feed the carry: count from its start.
        let start = since.map_or(first, |month| month.min(first));
        let mut wide = filter.clone();
        wide.from = Some(calendar::month_key(start));
        wide.to = shown.months.last().map(|month| calendar::month_key(*month));
        let ctx = self.report_ctx(&wide, now, zone, inflation)?;
        let cash = self.cashflow_report(&ctx);
        let this_month = first_of_month(ctx.today);

        let mut carried = Amount::ZERO;
        let mut rows = Vec::new();
        for (row, month) in cash.rows.iter().zip(&ctx.months) {
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
            Some(last) => self.balance_days(&ctx, last)?,
            None => (Vec::new(), None, None, None),
        };
        Ok(MonthBalance {
            currency: cash.meta.currency,
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
    fn balance_days(&self, ctx: &Ctx, row: &MonthBalanceRow) -> Result<BalanceDays> {
        let first = calendar::parse_month(&row.month)
            .ok_or_else(|| Error::InvalidInput(format!("month {}: expected YYYY-MM", row.month)))?;
        let (from, to) = ctx.cal.month_bounds(first);
        let mut net: BTreeMap<NaiveDate, Amount> = BTreeMap::new();
        let mut income_mark: Option<MonthBalanceMark> = None;
        let mut spent_mark: Option<MonthBalanceMark> = None;
        for flow in self.report_flows(ctx, from, to) {
            let value = ctx.shown(flow.value, first);
            let (sign, mark) = match flow.class {
                FlowClass::Income => (value, &mut income_mark),
                FlowClass::Fixed | FlowClass::Daily => (-value, &mut spent_mark),
                _ => continue,
            };
            if mark.as_ref().is_none_or(|best| value.abs() > best.amount) {
                *mark = Some(MonthBalanceMark {
                    date: flow.date.format("%Y-%m-%d").to_string(),
                    amount: value.abs(),
                    category: flow.categories.first().cloned().unwrap_or_default(),
                });
            }
            *net.entry(flow.date).or_default() += sign;
        }
        let per_day = if row.current {
            let month = self.calendar_month(&row.month, ctx.now, ctx.cal.zone_name())?;
            month.allowance.filter(|_| month.currency == ctx.currency)
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
            if *date > ctx.today {
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

    /// Carry the unspent over from `month` (`YYYY-MM`), or stop carrying (`None`).
    pub fn set_carry_over(&mut self, month: Option<&str>) -> Result<()> {
        let since = match month.map(str::trim).filter(|text| !text.is_empty()) {
            None => None,
            Some(text) => Some(
                calendar::parse_month(text)
                    .map(calendar::month_key)
                    .ok_or_else(|| {
                        Error::InvalidInput(format!("month {text}: expected YYYY-MM"))
                    })?,
            ),
        };
        if self.doc.settings.carry_since == since {
            return Ok(());
        }
        self.doc.settings.carry_since = since;
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::Transaction;
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;

    const NOW: &str = "2026-06-15T12:00:00Z";

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    fn balance(vault: &UnlockedVault, from: &str, to: &str) -> MonthBalance {
        let filter = ReportFilter {
            from: Some(from.into()),
            to: Some(to.into()),
            ..ReportFilter::default()
        };
        vault
            .month_balance(&filter, unix_secs(NOW).unwrap(), "UTC", &BTreeMap::new())
            .unwrap()
    }

    fn vault_with_months() -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        for (month, spent) in [("03", "700"), ("04", "1200"), ("05", "900"), ("06", "100")] {
            vault
                .add_transaction(dated(
                    income(&card, "1000", "salary"),
                    &format!("2026-{month}-05T10:00:00Z"),
                ))
                .unwrap();
            vault
                .add_transaction(dated(
                    payment(&card, spent, "shop"),
                    &format!("2026-{month}-10T10:00:00Z"),
                ))
                .unwrap();
        }
        (dir, vault)
    }

    #[test]
    fn without_carry_each_month_stands_alone() {
        let (_dir, vault) = vault_with_months();
        let report = balance(&vault, "2026-03", "2026-06");
        let left: Vec<Amount> = report.rows.iter().map(|row| row.left).collect();
        assert_eq!(
            left,
            vec![amount("300"), amount("-200"), amount("100"), amount("900")]
        );
        assert!(report.rows.iter().all(|row| row.carried_in == Amount::ZERO));
        assert!(report.rows[3].current);
        // The running month is left out of the average: (700 + 1200 + 900) / 3.
        assert_eq!(report.average_spent, amount("933.33"));
        assert_eq!(report.total.left, amount("1100"));
        assert_eq!(report.total.month, "2026-06");
    }

    #[test]
    fn the_carry_starts_where_it_was_turned_on_even_before_the_period() {
        let (_dir, mut vault) = vault_with_months();
        vault.set_carry_over(Some("2026-04")).unwrap();
        let report = balance(&vault, "2026-05", "2026-06");
        assert_eq!(report.carry_since.as_deref(), Some("2026-04"));
        // April −200 carries into May; May +100 on top makes −100 for June.
        assert_eq!(report.rows[0].carried_in, amount("-200"));
        assert_eq!(report.rows[0].total, amount("-100"));
        assert_eq!(report.rows[1].carried_in, amount("-100"));
        assert_eq!(report.rows[1].total, amount("800"));
        assert_eq!(report.total.total, amount("800"));
        vault.set_carry_over(None).unwrap();
        assert_eq!(
            balance(&vault, "2026-05", "2026-06").rows[1].carried_in,
            Amount::ZERO
        );
    }

    #[test]
    fn the_last_month_goes_day_by_day_and_ends_at_its_total() {
        let (_dir, vault) = vault_with_months();
        let report = balance(&vault, "2026-03", "2026-06");
        assert_eq!(report.days.len(), 30);
        let balance_on = |day: usize| report.days[day - 1].balance;
        // Nothing before the salary on the 5th, +1000 then, −100 on the 10th.
        assert_eq!(balance_on(4), Some(Amount::ZERO));
        assert_eq!(balance_on(5), Some(amount("1000")));
        assert_eq!(balance_on(15), Some(report.total.total));
        assert_eq!(balance_on(16), None);
        // The chart labels the biggest income and the biggest spending.
        let income = report.income_mark.as_ref().expect("an income mark");
        assert_eq!(income.amount, amount("1000"));
        assert_eq!(income.date, "2026-06-05");
        assert_eq!(
            report.spent_mark.as_ref().map(|mark| mark.amount),
            Some(amount("100"))
        );
        let forecast: Vec<Amount> = report.days.iter().filter_map(|day| day.forecast).collect();
        // A forecast exists only with an allowance; it never rises.
        assert!(report.per_day.is_some() || forecast.is_empty());
        assert!(forecast.windows(2).all(|pair| pair[1] <= pair[0]));
    }

    #[test]
    fn a_bad_month_is_refused() {
        let (_dir, mut vault) = vault_with_months();
        assert!(vault.set_carry_over(Some("June")).is_err());
    }
}
