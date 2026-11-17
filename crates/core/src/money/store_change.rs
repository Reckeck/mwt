//! Merging one account into another: what happens to the corrections.
//!
//! A correction stores the real balance `after` of its own account. Once two
//! accounts become one, "really 80" must mean 80 *plus what the other account
//! held at that moment*, or the other account's money is lost. Every moved
//! correction keeps its delta `after - before`, and the merged balance per
//! asset is the sum of both accounts.
//!
//! Pure: no vault, no clock of its own, no I/O. The document must be rebased
//! (every mutation ends in `recompute_holdings`), so each correction's leg is
//! its real delta.

use std::collections::HashMap;

use crate::amount::Amount;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::pricing::history::secs_or_now;

/// Creation order of a record, built by the caller from the sync stamp
/// (money must not depend on sync). Only its order matters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CreatedAt(pub (u64, u32, String));

pub struct MergeInput<'a> {
    pub transactions: &'a [Transaction],
    pub source_id: &'a str,
    pub target_id: &'a str,
    pub source_name: &'a str,
    pub target_name: &'a str,
    /// Transaction id -> creation order; missing means unstamped (newest).
    pub created: &'a HashMap<String, CreatedAt>,
}

/// One moved correction whose counted balance is raised by what the other
/// account held at that moment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CorrectionShift {
    pub tx_id: String,
    pub timestamp: String,
    pub from_store_id: String,
    /// Whose correction it is.
    pub from_store_name: String,
    pub asset: AssetRef,
    pub after_old: Amount,
    pub after_new: Amount,
    /// `after_new - after_old`.
    pub other_balance: Amount,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergePlan {
    pub moved_transactions: u32,
    pub shifts: Vec<CorrectionShift>,
}

/// Rank of a correction in the order `rebase_adjustments` uses: second,
/// unstamped last, creation stamp, list index.
struct Ranked<'a> {
    secs: i64,
    unstamped: bool,
    created: Option<&'a CreatedAt>,
    index: usize,
}

impl Ranked<'_> {
    fn key(&self) -> (i64, bool, Option<&CreatedAt>, usize) {
        (self.secs, self.unstamped, self.created, self.index)
    }
}

pub fn merge_plan(input: &MergeInput<'_>) -> MergePlan {
    let ranks: HashMap<usize, Ranked<'_>> = input
        .transactions
        .iter()
        .enumerate()
        .filter(|(_, tx)| matches!(tx, Transaction::Adjustment(_)))
        .map(|(index, tx)| {
            let created = input.created.get(&tx.meta().id);
            let rank = Ranked {
                secs: secs_or_now(&tx.meta().timestamp),
                unstamped: created.is_none(),
                created,
                index,
            };
            (index, rank)
        })
        .collect();

    let mut plan = MergePlan {
        moved_transactions: input
            .transactions
            .iter()
            .filter(|tx| tx.touches(input.source_id))
            .count() as u32,
        shifts: Vec::new(),
    };
    for (index, tx) in input.transactions.iter().enumerate() {
        let Transaction::Adjustment(inner) = tx else {
            continue;
        };
        let (own_name, other_id) = if inner.store_id == input.source_id {
            (input.source_name, input.target_id)
        } else if inner.store_id == input.target_id {
            (input.target_name, input.source_id)
        } else {
            continue;
        };
        let Some(rank) = ranks.get(&index) else {
            continue;
        };
        let held = other_balances(input.transactions, &ranks, other_id, rank);
        for line in &inner.lines {
            let shift = held.get(&line.asset.key()).copied().unwrap_or_default();
            if shift.is_zero() {
                continue;
            }
            plan.shifts.push(CorrectionShift {
                tx_id: tx.meta().id.clone(),
                timestamp: tx.meta().timestamp.clone(),
                from_store_id: inner.store_id.clone(),
                from_store_name: own_name.to_string(),
                asset: line.asset.clone(),
                after_old: line.after,
                after_new: line.after + shift,
                other_balance: shift,
            });
        }
    }
    plan
}

/// What `store_id` held, per asset key, at the correction `at`: its legs up to
/// that second, minus the corrections of the same second ranking at or after it.
fn other_balances(
    transactions: &[Transaction],
    ranks: &HashMap<usize, Ranked<'_>>,
    store_id: &str,
    at: &Ranked<'_>,
) -> HashMap<String, Amount> {
    let mut totals: HashMap<String, Amount> = HashMap::new();
    for (index, tx) in transactions.iter().enumerate() {
        if secs_or_now(&tx.meta().timestamp) > at.secs {
            continue;
        }
        if ranks
            .get(&index)
            .is_some_and(|other| other.key() >= at.key())
        {
            continue;
        }
        for leg in tx.legs() {
            if leg.store_id == store_id && !leg.asset.is_derivative() {
                *totals.entry(leg.asset.key()).or_default() += leg.amount;
            }
        }
    }
    totals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::transaction::{
        AdjustmentLine, AdjustmentTx, KindDetails, PriceStamp, TopupTx, TxMeta,
    };

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn money(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn meta(id: &str, timestamp: &str) -> TxMeta {
        let mut meta = TxMeta::new("");
        meta.id = id.into();
        meta.timestamp = timestamp.into();
        meta
    }

    fn topup(id: &str, store: &str, raw: &str, timestamp: &str) -> Transaction {
        Transaction::Topup(TopupTx {
            meta: meta(id, timestamp),
            store_id: store.into(),
            asset: rub(),
            amount: money(raw),
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        })
    }

    fn correction(
        id: &str,
        store: &str,
        before: &str,
        after: &str,
        timestamp: &str,
    ) -> Transaction {
        Transaction::Adjustment(AdjustmentTx {
            meta: meta(id, timestamp),
            store_id: store.into(),
            lines: vec![AdjustmentLine {
                asset: rub(),
                before: money(before),
                after: money(after),
                price: PriceStamp::default(),
            }],
            fee: None,
        })
    }

    fn plan_of(transactions: &[Transaction], created: &HashMap<String, CreatedAt>) -> MergePlan {
        merge_plan(&MergeInput {
            transactions,
            source_id: "a",
            target_id: "b",
            source_name: "Card 1",
            target_name: "Card 2",
            created,
        })
    }

    /// Card 1: +300 (1 Jan), really 80 (1 Mar), +50 (10 Mar); Card 2: +1 000 (15 Jan).
    fn review() -> Vec<Transaction> {
        vec![
            topup("t1", "a", "300", "2026-01-01T00:00:00Z"),
            topup("t2", "b", "1000", "2026-01-15T00:00:00Z"),
            correction("c1", "a", "300", "80", "2026-03-01T00:00:00Z"),
            topup("t3", "a", "50", "2026-03-10T00:00:00Z"),
        ]
    }

    #[test]
    fn a_correction_of_the_source_is_shifted_by_the_targets_balance() {
        let plan = plan_of(&review(), &HashMap::new());
        assert_eq!(plan.moved_transactions, 3);
        assert_eq!(plan.shifts.len(), 1);
        let shift = &plan.shifts[0];
        assert_eq!(shift.from_store_name, "Card 1");
        assert_eq!(shift.after_old, money("80"));
        assert_eq!(shift.after_new, money("1080"));
        assert_eq!(shift.other_balance, money("1000"));
    }

    #[test]
    fn a_correction_of_the_target_is_shifted_by_the_sources_balance() {
        let mut transactions = review();
        transactions.push(correction(
            "c2",
            "b",
            "1000",
            "1200",
            "2026-04-01T00:00:00Z",
        ));
        let plan = plan_of(&transactions, &HashMap::new());
        assert_eq!(plan.shifts.len(), 2);
        let second = &plan.shifts[1];
        assert_eq!(second.tx_id, "c2");
        assert_eq!(second.after_old, money("1200"));
        assert_eq!(second.after_new, money("1330"));
        assert_eq!(second.other_balance, money("130"));
    }

    #[test]
    fn a_zero_shift_is_not_listed() {
        // The target holds nothing yet when the source is corrected.
        let transactions = vec![
            topup("t1", "a", "300", "2026-01-01T00:00:00Z"),
            correction("c1", "a", "300", "80", "2026-03-01T00:00:00Z"),
            topup("t2", "b", "1000", "2026-04-01T00:00:00Z"),
        ];
        assert!(plan_of(&transactions, &HashMap::new()).shifts.is_empty());
    }

    #[test]
    fn same_second_corrections_follow_the_rebase_order() {
        // Both correct at one second; "ca" was created first, so it counts
        // into the shift of "cb" and not the reverse.
        let transactions = vec![
            topup("t1", "a", "100", "2026-01-01T00:00:00Z"),
            topup("t2", "b", "200", "2026-01-01T00:00:00Z"),
            correction("cb", "b", "200", "250", "2026-02-01T00:00:00Z"),
            correction("ca", "a", "100", "70", "2026-02-01T00:00:00Z"),
        ];
        let created = HashMap::from([
            ("ca".to_string(), CreatedAt((1, 0, "d".into()))),
            ("cb".to_string(), CreatedAt((2, 0, "d".into()))),
        ]);
        let plan = plan_of(&transactions, &created);
        let of = |id: &str| plan.shifts.iter().find(|shift| shift.tx_id == id).unwrap();
        // ca sees only b's 200 (cb ranks after it); cb sees a's 70 after ca.
        assert_eq!(of("ca").other_balance, money("200"));
        assert_eq!(of("ca").after_new, money("270"));
        assert_eq!(of("cb").other_balance, money("70"));
        assert_eq!(of("cb").after_new, money("320"));
        // deltas preserved: merged 100 + 200 -> 270 (-30) -> 320 (+50).
    }
}
