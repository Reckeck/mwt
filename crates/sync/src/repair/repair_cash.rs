//! Stage M, rule R1 of `repair_invariants`: one root Cash wallet.
//!
//! Two vaults made apart each own a Cash wallet, and both are locked against
//! delete and archive, so after the first sync the user could not fix it.
//! Winner: the id `cash` (what `ensure_cash_wallet` makes now), else the
//! earliest creation stamp, else the smaller id, so every device picks the
//! same one from the same merged state. A loser's sub-accounts move under the
//! winner (a clashing name gets a free one), every reference to it (operations,
//! links, goals, exchange targets) is pointed at the winner and the loser is
//! dropped, which the change tracker turns into a tombstone. The user-facing
//! `merge_into` still refuses a root Cash; this is a path of the repair only.

use super::repair::{children_of, find_store, free_store_name};
use super::repair_report::RepairReport;
use mwt_model::StoreKind;
use mwt_model::VaultDoc;

/// Merge the root Cash wallets into one (`cash_id` is the id the vault gives its Cash wallet);
/// the ids of the merged-away go to the report.
pub fn repair_cash_wallets(doc: &mut VaultDoc, cash_id: &str, report: &mut RepairReport) {
    let mut roots: Vec<String> = doc
        .stores
        .iter()
        .filter(|node| node.kind == StoreKind::Cash && node.parent_id.is_none())
        .map(|node| node.id.clone())
        .collect();
    if roots.len() < 2 {
        return;
    }
    let created = |id: &str| {
        doc.sync
            .stamps
            .get(&format!("stores:{id}"))
            .map(|known| known.created.clone())
            .unwrap_or_default()
    };
    roots.sort_by_key(|id| (id != cash_id, created(id), id.clone()));
    let winner = roots.remove(0);
    for loser in roots {
        if merge_cash_into(doc, &loser, &winner).is_some() {
            report.merged_cash.push(loser);
        }
    }
}

fn merge_cash_into(doc: &mut VaultDoc, loser: &str, winner: &str) -> Option<()> {
    let mut children: Vec<String> = children_of(doc, loser)
        .into_iter()
        .map(|child| child.id.clone())
        .collect();
    children.sort();
    for child in children {
        let name = find_store(doc, &child)?.name.clone();
        let free = free_store_name(doc, &child, Some(winner), &name);
        let node = doc.stores.iter_mut().find(|node| node.id == child)?;
        node.parent_id = Some(winner.to_string());
        node.name = free;
    }
    doc.stores.retain(|node| node.id != loser);
    doc.exchanges.bybit.retain(|keys| keys.store_id != loser);
    // Sync bookkeeping keeps its own keys; only the document is repointed.
    let sync = std::mem::take(&mut doc.sync);
    let mut value = serde_json::to_value(&*doc).ok()?;
    replace_id(&mut value, loser, winner);
    let mut moved = serde_json::from_value(value).ok()?;
    std::mem::swap(doc, &mut moved);
    doc.sync = sync;
    Some(())
}

/// Replace every string equal to `from` with `to`, anywhere in the document.
/// Ids are UUIDs, so a value equal to one is that id. A copy of
/// `service::store_change::replace_id`, which is private to `service` until the money stream
/// moves it.
fn replace_id(value: &mut serde_json::Value, from: &str, to: &str) {
    match value {
        serde_json::Value::String(text) if text == from => *text = to.to_string(),
        serde_json::Value::Array(items) => {
            for entry in items {
                replace_id(entry, from, to);
            }
        }
        serde_json::Value::Object(map) => {
            for entry in map.values_mut() {
                replace_id(entry, from, to);
            }
        }
        _ => {}
    }
}
