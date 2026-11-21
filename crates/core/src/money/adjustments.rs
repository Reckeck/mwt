//! Corrections ("Adjustment"): set the balances of one account to what is
//! really there. The ledger balance before it is worked out at the
//! correction's own moment, so a back-dated one corrects the past, and it is
//! worked out again whenever the history changes: what a correction keeps is
//! the real balance (`after`), the difference follows the ledger.

use std::collections::HashMap;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::AssetRef;
use crate::model::transaction::{AdjustmentLine, Transaction};
use crate::model::vault::VaultDoc;
use crate::sync::stamp::Stamp;
use crate::sync::view::full_key;
use crate::time::secs_or_now;

/// What the correction screen opens with.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AdjustmentDraft {
    pub store_id: String,
    /// RFC3339 moment the balances are for.
    pub at: String,
    /// Every asset the account holds then; `after` starts equal to `before`.
    pub lines: Vec<AdjustmentLine>,
}

/// Balance of every asset of one store at `at` (inclusive), leaving out the
/// transaction `exclude`. Zero balances are dropped.
pub fn balances_at(
    doc: &VaultDoc,
    store_id: &str,
    at: &str,
    exclude: Option<&str>,
) -> Vec<(AssetRef, Amount)> {
    balances_where(doc, store_id, secs_or_now(at), |tx| {
        exclude.is_some_and(|id| tx.meta().id == id)
    })
}

/// Balance of every asset of one store up to `limit` seconds (inclusive),
/// leaving out the transactions `skip` picks. Zero balances are dropped.
fn balances_where(
    doc: &VaultDoc,
    store_id: &str,
    limit: i64,
    skip: impl Fn(&Transaction) -> bool,
) -> Vec<(AssetRef, Amount)> {
    let mut order: Vec<String> = Vec::new();
    let mut assets: HashMap<String, AssetRef> = HashMap::new();
    let mut totals: HashMap<String, Amount> = HashMap::new();
    for tx in &doc.transactions {
        if skip(tx) || secs_or_now(&tx.meta().timestamp) > limit {
            continue;
        }
        for leg in tx.legs() {
            if leg.store_id != store_id || leg.asset.is_derivative() {
                continue;
            }
            let key = leg.asset.key();
            if !assets.contains_key(&key) {
                order.push(key.clone());
                assets.insert(key.clone(), leg.asset.clone());
            }
            *totals.entry(key).or_default() += leg.amount;
        }
    }
    order
        .into_iter()
        .filter_map(|key| {
            let total = totals.get(&key).copied().unwrap_or_default();
            (!total.is_zero()).then(|| (assets.remove(&key).unwrap_or_default(), total))
        })
        .collect()
}

/// Work out again what the ledger held before every correction, oldest
/// first, so each one still sets the balance it was given after an
/// earlier operation was added, edited or deleted. A line whose
/// difference comes to zero stays: it keeps the balance pinned for the
/// next change. Corrections of one account at the same moment see only
/// the ones before them in the list, so the result does not flip-flop.
/// Each correction's new `before` feeds the next one's balance, so the
/// document is rewritten in place, oldest first.
pub fn rebase_adjustment_lines(doc: &mut VaultDoc) {
    // Corrections at one moment go in the order they were created: by the
    // stamp the record got when it was made, which every device holds the
    // same, so the result does not depend on the order of the list after a
    // sync. One not stamped yet was just added and is the newest; without
    // stamps at all the list order decides.
    let mut order: Vec<(i64, bool, Option<Stamp>, usize)> = doc
        .transactions
        .iter()
        .enumerate()
        .filter(|(_, tx)| matches!(tx, Transaction::Adjustment(_)))
        .map(|(index, tx)| {
            let created = doc
                .sync
                .stamps
                .get(&full_key("transactions", &tx.meta().id))
                .map(|stamps| stamps.created.clone());
            (
                secs_or_now(&tx.meta().timestamp),
                created.is_none(),
                created,
                index,
            )
        })
        .collect();
    order.sort_unstable();
    let order: Vec<(i64, usize)> = order
        .into_iter()
        .map(|(secs, _, _, index)| (secs, index))
        .collect();
    for (rank, &(secs, index)) in order.iter().enumerate() {
        let Transaction::Adjustment(inner) = &doc.transactions[index] else {
            continue;
        };
        let id = inner.meta.id.clone();
        let store_id = inner.store_id.clone();
        let later: Vec<&str> = order[rank + 1..]
            .iter()
            .take_while(|(other, _)| *other == secs)
            .map(|&(_, other)| doc.transactions[other].meta().id.as_str())
            .collect();
        let balances: HashMap<String, Amount> = balances_where(doc, &store_id, secs, |tx| {
            tx.meta().id == id || later.contains(&tx.meta().id.as_str())
        })
        .into_iter()
        .map(|(asset, balance)| (asset.key(), balance))
        .collect();
        if let Transaction::Adjustment(inner) = &mut doc.transactions[index] {
            for line in &mut inner.lines {
                line.before = balances.get(&line.asset.key()).copied().unwrap_or_default();
            }
        }
    }
}

/// Fill in what the ledger held before the correction, from the other
/// transactions, and drop the assets whose balance already matches.
pub fn complete_adjustment(doc: &VaultDoc, tx: &mut Transaction) -> Result<()> {
    let Transaction::Adjustment(inner) = tx else {
        return Ok(());
    };
    let balances: HashMap<String, Amount> = balances_at(
        doc,
        &inner.store_id,
        &inner.meta.timestamp,
        Some(&inner.meta.id),
    )
    .into_iter()
    .map(|(asset, balance)| (asset.key(), balance))
    .collect();
    for line in &mut inner.lines {
        line.before = balances.get(&line.asset.key()).copied().unwrap_or_default();
    }
    if inner.lines.is_empty() {
        return Ok(());
    }
    // Editing a correction keeps its own assets even when they match now:
    // such a line keeps pinning the balance against later changes.
    let own: Vec<String> = doc
        .transactions
        .iter()
        .find_map(|tx| match tx {
            Transaction::Adjustment(old) if old.meta.id == inner.meta.id => {
                Some(old.lines.iter().map(|line| line.asset.key()).collect())
            }
            _ => None,
        })
        .unwrap_or_default();
    inner
        .lines
        .retain(|line| !line.delta().is_zero() || own.contains(&line.asset.key()));
    if inner.lines.is_empty() {
        return Err(Error::InvalidInput(
            "nothing to correct: the balances already match".into(),
        ));
    }
    Ok(())
}
