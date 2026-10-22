//! Figures for the existing dashboard widgets (plan H3): the month's pace for
//! «Бюджет дня» and «Итоги месяца», the capital's trend for «Чистый капитал».

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::FlowClass;
use super::UnlockedVault;
use crate::Amount;
use crate::calendar::period::{PeriodWindow, months_back, period_window};
use crate::calendar::{self, Calendar};
use crate::ipc_contract::{BaseKind, Comparison};
use crate::model::vault::DashPeriod;

/// How many growing categories «Итоги месяца» names.
const TOP_GROWTH: usize = 3;

/// A spending category that grew against the same date of the last month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CategoryGrowth {
    /// `Category::key`.
    pub category: String,
    pub value: Amount,
    pub base: Amount,
}

/// The month so far against the previous month to the same date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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

impl UnlockedVault {
    fn pace_totals(
        &self,
        cal: &Calendar,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> PaceTotals {
        let mut totals = PaceTotals::default();
        for flow in self.flows_between(
            cal,
            currency,
            cal.start_of(from),
            cal.start_of(to + Duration::days(1)),
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

    /// The current month's pace (plan H3: «Бюджет дня» and «Итоги месяца»).
    pub fn month_pace(&self, now: i64, zone: &str) -> MonthPace {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let currency = self.budget_currency();
        let first = calendar::first_of_month(today);
        let prev_first = months_back(first, 1);
        let prev_today = months_back(today, 1);
        let current = self.pace_totals(&cal, &currency, first, today);
        let previous = self.pace_totals(&cal, &currency, prev_first, prev_today);
        let has_history = self.doc.transactions.iter().any(|tx| {
            crate::pricing::history::unix_secs(&tx.meta().timestamp)
                .is_some_and(|secs| cal.date_of(secs) <= prev_today)
        });
        let base = |value: Amount| has_history.then_some(value);

        let day = today.day();
        let days = calendar::days_in_month(today);
        let ratio = Amount::from(day) / Amount::from(days);
        let savings_plan = current.income * self.doc.settings.savings_rate;
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

    /// The holdings' value over a period (plan H3: «Чистый капитал»).
    pub fn net_worth_trend(&self, period: DashPeriod, now: i64, zone: &str) -> NetWorthTrend {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let (currency, _) = self.base_and_fx();
        let PeriodWindow { start, .. } = period_window(period, today);
        let step = |date: NaiveDate| -> usize {
            match period {
                DashPeriod::Month => (date - start).num_days() as usize,
                DashPeriod::Quarter => ((date - start).num_days() / 7) as usize,
                DashPeriod::Year => date.month0() as usize,
            }
        };
        let slots = step(today) + 1;
        let history = self.history_series(&currency);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::Transaction;
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;

    fn dated(mut tx: Transaction, category: &str, ts: &str) -> Transaction {
        tx.meta_mut().categories = vec![category.into()];
        at(&mut tx, ts);
        tx
    }

    #[test]
    fn the_month_pace_compares_with_the_same_date_and_names_growth() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        for tx in [
            dated(
                income(&card, "1000", "pay"),
                "salary",
                "2026-09-05T10:00:00Z",
            ),
            dated(
                payment(&card, "100", "food"),
                "groceries",
                "2026-09-06T10:00:00Z",
            ),
            dated(payment(&card, "50", "taxi"), "taxi", "2026-09-07T10:00:00Z"),
            // After the same date last month: not in the base.
            dated(
                payment(&card, "300", "late"),
                "groceries",
                "2026-09-20T10:00:00Z",
            ),
            dated(
                income(&card, "1000", "pay"),
                "salary",
                "2026-10-05T10:00:00Z",
            ),
            dated(
                payment(&card, "180", "food"),
                "groceries",
                "2026-10-06T10:00:00Z",
            ),
            dated(payment(&card, "60", "taxi"), "taxi", "2026-10-07T10:00:00Z"),
            dated(
                payment(&card, "20", "cinema"),
                "entertainment",
                "2026-10-08T10:00:00Z",
            ),
        ] {
            vault.add_transaction(tx).unwrap();
        }
        let now = unix_secs("2026-10-10T12:00:00Z").unwrap();
        let pace = vault.month_pace(now, "UTC");
        assert_eq!(pace.month, "2026-10");
        assert_eq!((pace.day, pace.days), (10, 31));
        assert_eq!(pace.income.base, Some(amount("1000")));
        assert_eq!(pace.spent.value, amount("260"));
        assert_eq!(pace.spent.base, Some(amount("150")));
        assert_eq!(pace.saved.value, amount("740"));
        // 260 over 10 of 31 days.
        assert_eq!(pace.forecast_spent, amount("806"));
        let keys: Vec<&str> = pace
            .growth
            .iter()
            .map(|row| row.category.as_str())
            .collect();
        assert_eq!(keys, ["groceries", "entertainment", "taxi"]);
        assert!(pace.spend_plan_to_date.is_some());
    }

    #[test]
    fn a_new_vault_has_no_base_and_no_growth() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault
            .add_transaction(dated(
                payment(&card, "40", "food"),
                "groceries",
                "2026-10-03T10:00:00Z",
            ))
            .unwrap();
        let pace = vault.month_pace(unix_secs("2026-10-10T12:00:00Z").unwrap(), "UTC");
        assert_eq!(pace.spent.base, None);
        assert!(pace.growth.is_empty());
        assert_eq!(pace.spend_plan_to_date, None);
    }

    #[test]
    fn the_capital_trend_starts_from_the_value_before_the_period() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        for (sum, ts) in [
            ("500", "2026-09-20T10:00:00Z"),
            ("200", "2026-10-04T10:00:00Z"),
        ] {
            vault
                .add_transaction(dated(income(&card, sum, "pay"), "salary", ts))
                .unwrap();
        }
        let trend = vault.net_worth_trend(
            DashPeriod::Month,
            unix_secs("2026-10-10T12:00:00Z").unwrap(),
            "UTC",
        );
        assert_eq!(trend.start, "2026-10-01");
        assert_eq!(trend.series.len(), 10);
        assert_eq!(trend.change.base, Some(amount("500")));
        assert_eq!(trend.change.value, amount("700"));
        assert_eq!(trend.series[0], amount("500"));
        assert_eq!(trend.series[3], amount("700"));
    }
}

#[cfg(test)]
mod rate_change_tests {
    use crate::model::vault::{FxPoint, PricePoint};
    use crate::service::test_support::*;

    #[test]
    fn rates_and_prices_carry_their_change() {
        let (_dir, mut vault) = setup();
        vault.set_base_currency("usd");
        for (per_usd, ts) in [
            ("100", "2026-10-20T10:00:00Z"),
            ("80", "2026-10-21T12:00:00Z"),
        ] {
            vault.doc.fx_cache.push(FxPoint {
                currency: "rub".into(),
                per_usd: amount(per_usd),
                ts: ts.into(),
                source: "test".into(),
            });
        }
        let table = vault.fx_table(&[]);
        let rub = table.rows.iter().find(|row| row.currency == "rub").unwrap();
        // One rouble went from 0.01 to 0.0125 dollars.
        assert_eq!(rub.change_pct, Some(amount("25")));

        let key = btc().key();
        vault.doc.price_cache.push(PricePoint {
            asset_key: key.clone(),
            value_usd: amount("50000"),
            ts: "2026-10-21T12:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: Some(amount("-3.5")),
        });
        let summary = vault.portfolio_summary();
        if let Some(stat) = summary.assets.iter().find(|stat| stat.asset.key() == key) {
            assert_eq!(stat.change_24h_pct, Some(amount("-3.5")));
        }
    }
}
