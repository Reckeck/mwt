//! Dashboard figures with their comparison (plan H1/H2): one call per widget,
//! the period and the same span of the previous period computed here.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use super::FlowClass;
use super::UnlockedVault;
use crate::Amount;
use crate::calendar::period::{PeriodWindow, period_window};
use crate::calendar::{self, Calendar};
use crate::ipc_contract::{BaseKind, Comparison};
use crate::model::vault::DashPeriod;

/// One day of the «Траты» week strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SpendingDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Everyday spending of the day.
    pub daily: Amount,
    pub over: bool,
}

/// The «Траты» widget: the period, the week and a series for the sparkline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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

impl UnlockedVault {
    fn spending_totals(
        &self,
        cal: &Calendar,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> (Totals, Vec<(NaiveDate, Amount)>) {
        let flows = self.flows_between(
            cal,
            currency,
            cal.start_of(from),
            cal.start_of(to + Duration::days(1)),
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
            days.push((flow.date, flow.value));
        }
        (totals, days)
    }

    /// Spending for the dashboard's «Траты» (plan H2).
    pub fn spending_overview(&self, period: DashPeriod, now: i64, zone: &str) -> SpendingOverview {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let currency = self.budget_currency();
        let PeriodWindow {
            start,
            end,
            prev_start,
            prev_end,
        } = period_window(period, today);
        let (current, days) = self.spending_totals(&cal, &currency, start, end);
        let (previous, _) = self.spending_totals(&cal, &currency, prev_start, prev_end);
        let has_history = self.doc.transactions.iter().any(|tx| {
            crate::pricing::history::unix_secs(&tx.meta().timestamp)
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
        let (week_now, _) = self.spending_totals(&cal, &currency, week_start, today);
        let (week_before, _) = self.spending_totals(
            &cal,
            &currency,
            week_start - Duration::days(7),
            week_start - Duration::days(1),
        );
        let months = [
            calendar::first_of_month(week_start),
            calendar::first_of_month(today),
        ];
        let mut by_day = std::collections::HashMap::new();
        let mut allowance = None;
        for first in months.iter().collect::<std::collections::BTreeSet<_>>() {
            if let Ok(month) = self.calendar_month(&calendar::month_key(*first), now, zone) {
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
                    over: day.is_some_and(|day| day.status == super::reports::DayStatus::Over),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::Transaction;
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;

    fn spend(store: &str, sum: &str, category: &str, ts: &str) -> Transaction {
        let mut tx = payment(store, sum, "spend");
        tx.meta_mut().categories = vec![category.into()];
        at(&mut tx, ts);
        tx
    }

    fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        (dir, vault, card)
    }

    #[test]
    fn spending_compares_to_the_same_date_of_last_month() {
        let (_dir, mut vault, card) = vault_with_card();
        for (sum, category, ts) in [
            ("40", "groceries", "2026-09-05T10:00:00Z"),
            ("500", "rent_mortgage", "2026-09-10T10:00:00Z"),
            // After the same date last month: not in the base.
            ("70", "groceries", "2026-09-25T10:00:00Z"),
            ("60", "groceries", "2026-10-03T10:00:00Z"),
            ("20", "groceries", "2026-10-20T10:00:00Z"),
        ] {
            vault
                .add_transaction(spend(&card, sum, category, ts))
                .unwrap();
        }
        let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
        let view = vault.spending_overview(DashPeriod::Month, now, "UTC");
        assert_eq!(view.start, "2026-10-01");
        assert_eq!(view.spent.value, amount("80"));
        assert_eq!(view.spent.base, Some(amount("540")));
        assert_eq!(view.daily.base, Some(amount("40")));
        assert_eq!(view.daily.delta_pct(), Some(amount("100")));
        assert_eq!(view.series.len(), 22);
        assert_eq!(view.series[2], amount("60"));
        // The week: 16..22 October, 20 on the 20th; the week before has nothing.
        assert_eq!(view.week.len(), 7);
        assert_eq!(view.week[0].date, "2026-10-16");
        assert_eq!(view.week[4].daily, amount("20"));
        assert_eq!(view.week_total.value, amount("20"));
        assert_eq!(view.week_total.base, Some(Amount::ZERO));
    }

    #[test]
    fn a_new_vault_has_nothing_to_compare_with() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(spend(&card, "15", "groceries", "2026-10-21T10:00:00Z"))
            .unwrap();
        let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
        let view = vault.spending_overview(DashPeriod::Year, now, "UTC");
        assert_eq!(view.start, "2026-01-01");
        assert_eq!(view.spent.base, None);
        assert_eq!(view.series.len(), 10);
        assert_eq!(view.series[9], amount("15"));
    }
}
