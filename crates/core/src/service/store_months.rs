//! Money in and out of one wallet month by month, for the figures on its page
//! (a bank shows what came in and went out this month, not a P&L).

use std::collections::HashSet;

use crate::Amount;
use crate::calendar::{Calendar, first_of_month, month_key, next_month};
use crate::model::transaction::LegRole;
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

/// One local calendar month of a wallet, in the base currency.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreMonthRow {
    /// `YYYY-MM`.
    pub month: String,
    pub inflow: Amount,
    pub outflow: Amount,
    pub tx_count: u32,
}

impl UnlockedVault {
    /// The last `months` local months up to the one holding `now`, oldest first.
    /// Moves between two stores of the scope are left out: they are neither.
    pub fn store_months(
        &self,
        store_id: &str,
        include_children: bool,
        months: u32,
        now: i64,
        zone: &str,
    ) -> Vec<StoreMonthRow> {
        let cal = Calendar::new(zone);
        let (base, _fx) = self.base_and_fx();
        let book = self.rate_book();
        let mut scope: HashSet<String> = HashSet::from([store_id.to_string()]);
        if include_children {
            scope.extend(self.subtree_ids(store_id));
        }
        let mut first = first_of_month(cal.date_of(now));
        for _step in 1..months.max(1) {
            first = first_of_month(first - chrono::Duration::days(1));
        }
        let mut rows = Vec::new();
        let mut month = first;
        for _step in 0..months.max(1) {
            let (from, to) = cal.month_bounds(month);
            rows.push((
                from,
                to,
                StoreMonthRow {
                    month: month_key(month),
                    ..StoreMonthRow::default()
                },
            ));
            month = next_month(month);
        }
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            let Some((_, _, row)) = rows
                .iter_mut()
                .find(|(from, to, _)| secs >= *from && secs < *to)
            else {
                continue;
            };
            let legs: Vec<_> = tx
                .legs()
                .into_iter()
                .filter(|leg| leg.role == LegRole::Principal)
                .collect();
            let internal = legs.iter().all(|leg| scope.contains(&leg.store_id))
                && legs.iter().any(|leg| leg.amount.is_negative())
                && legs.iter().any(|leg| leg.amount.is_positive())
                && tx.store_ids().len() > 1;
            let mut touched = false;
            for leg in legs.iter().filter(|leg| scope.contains(&leg.store_id)) {
                touched = true;
                if internal {
                    continue;
                }
                let value = Self::leg_value_in(&book, leg, &base, secs).unwrap_or(Amount::ZERO);
                if leg.amount.is_negative() {
                    row.outflow += value;
                } else {
                    row.inflow += value;
                }
            }
            if touched {
                row.tx_count += 1;
            }
        }
        rows.into_iter()
            .map(|(_, _, mut row)| {
                row.inflow = row.inflow.round(2);
                row.outflow = row.outflow.round(2);
                row
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    #[test]
    fn store_months_split_inflow_and_outflow_by_month() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let mut salary = income_of(&bank, usd(), "1000", "1", "salary");
        at(&mut salary, "2026-09-10T12:00:00Z");
        vault.add_transaction(salary).unwrap();
        let mut rent = payment(&bank, "300", "rent");
        at(&mut rent, "2026-10-05T12:00:00Z");
        vault.add_transaction(rent).unwrap();
        let mut food = payment(&bank, "50", "food");
        at(&mut food, "2026-10-06T12:00:00Z");
        vault.add_transaction(food).unwrap();

        let now = crate::pricing::history::secs_or_now("2026-10-19T12:00:00Z");
        let rows = vault.store_months(&bank, true, 2, now, "UTC");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].month, "2026-09");
        assert_eq!(rows[0].inflow, amount("1000"));
        assert_eq!(rows[1].month, "2026-10");
        assert_eq!(rows[1].outflow, amount("350"));
        assert_eq!(rows[1].tx_count, 2);
    }
}
