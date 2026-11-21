//! Money in and out of one wallet month by month, for the figures on its page
//! (a bank shows what came in and went out this month, not a P&L).
//!
//! Pure part of the area: a read over a read context. There is no write. What a
//! leg is worth stays with the budget area until it moves (OD6): the caller
//! hands that valuation in, so this module names nothing of `service`.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar::{Calendar, first_of_month, month_key, next_month};
use crate::kernel::books::Books;
use crate::model::ids::StoreId;
use crate::model::transaction::{Leg, LegRole};
use crate::money::tree::subtree_ids;
use crate::time::secs_or_now;

/// One local calendar month of a wallet, in the base currency.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreMonthRow {
    /// `YYYY-MM`.
    pub month: String,
    pub inflow: Amount,
    pub outflow: Amount,
    pub tx_count: u32,
}

/// The last `months` local months up to the one holding `now`, oldest first.
/// Moves between two stores of the scope are left out: they are neither.
/// `leg_value(leg, currency, secs)` is the leg's worth, `None` without a rate.
pub fn store_months(
    books: &Books<'_>,
    store_id: &StoreId,
    include_children: bool,
    months: u32,
    now: i64,
    zone: &str,
    leg_value: impl Fn(&Leg, &str, i64) -> Option<Amount>,
) -> Vec<StoreMonthRow> {
    let doc = books.doc();
    let cal = Calendar::new(zone);
    let (base, _fx) = books.base_and_fx();
    let mut scope: HashSet<String> = HashSet::from([store_id.to_string()]);
    if include_children {
        scope.extend(subtree_ids(doc, store_id.as_str()));
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
    for tx in &doc.transactions {
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
            let value = leg_value(leg, base, secs).unwrap_or(Amount::ZERO);
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
