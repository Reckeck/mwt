//! The invariant pass of stage M: after a merge (and when a vault opens) the
//! document is checked against the rules a merge cannot keep by itself, and a
//! violation is fixed by a deterministic edit.
//!
//! - Acts only on a violation, so a consistent document is not stamped or sent.
//! - Deterministic: the fix depends only on the merged state (ids, stamps,
//!   values), so two devices that merged the same records make the same edit.
//! - Stamped as a local edit, so it travels to the other devices, older builds
//!   included; recorded in the batch that caused it, so undoing the batch
//!   undoes the repair; the user is told in a notice.
//! - Never changes money: it moves or drops references and structure only.
//!
//! Rules, in the order they run: R3 (deleted records that money points to are
//! restored, `repair_restore.rs`), R9 (staged imports that are already booked
//! or discarded, `repair_imports.rs`), duplicates by natural key, R1 (one root Cash
//! wallet, `repair_cash.rs`), R2 (account tree), R4 (envelopes of deleted
//! goals), R5 (links to missing asset classes), the shares of the classes
//! (`repair_natural.rs`), the links and money of shared events (R6, R7,
//! `repair_events.rs`), R8 (emergency fund and inflation sources that name a
//! missing record). The order is kept by `usecases::sync::repair`; this file
//! holds R2 and R4 and what every rule shares.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::Value;

use super::repair_report::RepairReport;
use crate::model::notify::{MAX_NOTICE_LOG, NoticeRecord};
use crate::model::store::{Binding, Custody, ExchangeProfile, Store, StoreKind};
use crate::model::vault::VaultDoc;
use crate::sync::{BatchEntry, Stamp};

/// The keys whose record differs between two views of the document.
pub fn changed_keys(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Add the entries of a repair to those a batch already holds: a key the batch
/// knows keeps its `before` and takes the new `after`.
pub fn merge_batch_entries(kept: &mut Vec<BatchEntry>, entries: Vec<BatchEntry>) {
    for entry in entries {
        match kept.iter_mut().find(|known| known.key == entry.key) {
            Some(known) => known.after = entry.after,
            None => kept.push(entry),
        }
    }
}

/// The notice of a repair goes to the (synced) log under a key that the repair
/// itself decides, so every device that makes it shows it once.
pub fn push_notice(doc: &mut VaultDoc, record: NoticeRecord) {
    if doc
        .notification_log
        .iter()
        .any(|known| known.key == record.key)
    {
        return;
    }
    doc.notification_log.push(record);
    let overflow = doc.notification_log.len().saturating_sub(MAX_NOTICE_LOG);
    doc.notification_log.drain(..overflow);
}

/// The store with this id.
pub(super) fn find_store<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc Store> {
    doc.stores.iter().find(|store| store.id == id)
}

/// The stores whose parent is `parent_id`.
pub(super) fn children_of<'doc>(doc: &'doc VaultDoc, parent_id: &str) -> Vec<&'doc Store> {
    doc.stores
        .iter()
        .filter(|store| store.parent_id.as_deref() == Some(parent_id))
        .collect()
}

/// A name free among the wallets (or among the children of `parent`).
pub(super) fn free_store_name(
    doc: &VaultDoc,
    node: &str,
    parent: Option<&str>,
    wanted: &str,
) -> String {
    let free = |name: &str| {
        let lowered = name.trim().to_lowercase();
        !doc.stores.iter().any(|store| {
            store.id != node
                && store.parent_id.as_deref() == parent
                && store.name.trim().to_lowercase() == lowered
        })
    };
    if free(wanted) {
        return wanted.to_string();
    }
    (2..)
        .map(|index| format!("{wanted} ({index})"))
        .find(|name| free(name))
        .unwrap_or_else(|| wanted.to_string())
}

// -- R2: the account tree -------------------------------------------------

/// A parent exists, is a wallet (not an account) of the same kind, and the
/// parent links make no cycle.
///
/// The choice is by the stamp of `parent_id`: of the members of a cycle the
/// one whose link is the oldest is cut (equal stamps: the smaller id), so
/// the more recent decision of the user stands. An account under a missing,
/// nested or foreign-kind parent is cut itself. A cut account becomes an
/// archived wallet of its own kind, as when an account is split off.
pub(crate) fn repair_account_tree(doc: &mut VaultDoc, report: &mut RepairReport) {
    while let Some(cycle) = account_cycle(doc) {
        let victim = cycle
            .into_iter()
            .min_by_key(|id| (parent_link_stamp(doc, id), id.clone()))
            .unwrap_or_default();
        cut_to_wallet(doc, &victim, report);
    }
    let mut broken: Vec<String> = doc
        .stores
        .iter()
        .filter(|node| {
            node.parent_id.as_deref().is_some_and(|parent_id| {
                find_store(doc, parent_id)
                    .is_none_or(|parent| parent.parent_id.is_some() || parent.kind != node.kind)
            })
        })
        .map(|node| node.id.clone())
        .collect();
    broken.sort();
    for id in broken {
        cut_to_wallet(doc, &id, report);
    }
}

/// The members of one cycle of parent links (smallest start id first), if any.
pub fn account_cycle(doc: &VaultDoc) -> Option<Vec<String>> {
    let mut ids: Vec<&str> = doc.stores.iter().map(|node| node.id.as_str()).collect();
    ids.sort_unstable();
    for start in ids {
        let mut path: Vec<&str> = vec![start];
        let mut seen: HashSet<&str> = HashSet::from([start]);
        let mut next = find_store(doc, start).and_then(|node| node.parent_id.as_deref());
        while let Some(id) = next {
            if !seen.insert(id) {
                let from = path.iter().position(|member| *member == id)?;
                return Some(
                    path[from..]
                        .iter()
                        .map(|member| member.to_string())
                        .collect(),
                );
            }
            path.push(id);
            next = find_store(doc, id).and_then(|node| node.parent_id.as_deref());
        }
    }
    None
}

fn parent_link_stamp(doc: &VaultDoc, id: &str) -> Stamp {
    doc.sync
        .stamps
        .get(&format!("stores:{id}"))
        .map(|stamps| stamps.field("parent_id").clone())
        .unwrap_or_default()
}

/// The fields of `detach` (a wallet of its own, no account type or
/// binding), archived, with a name free among the wallets.
fn cut_to_wallet(doc: &mut VaultDoc, id: &str, report: &mut RepairReport) {
    let Some(node) = find_store(doc, id).cloned() else {
        return;
    };
    let name = free_store_name(doc, id, None, &node.name);
    if let Some(target) = doc.stores.iter_mut().find(|store| store.id == id) {
        target.parent_id = None;
        target.sub_type = None;
        target.binding = Binding::Any;
        target.name = name;
        target.archived = true;
        target.custody = (target.kind == StoreKind::Crypto).then_some(Custody::Hot);
        target.exchange = (target.kind == StoreKind::Exchange).then(ExchangeProfile::default);
    }
    report.detached_accounts.push((id.to_string(), node.name));
}

// -- R4: envelopes of deleted goals -----------------------------------------

/// An envelope whose goal is gone is deleted again (the earmark only: the
/// money never left the account). A top-up on another device resurrects the
/// envelope after the goal went; this puts the deletion back.
pub(crate) fn repair_goal_envelopes(doc: &mut VaultDoc, report: &mut RepairReport) {
    let goals: HashSet<&str> = doc.goals.iter().map(|goal| goal.id.as_str()).collect();
    let mut gone: Vec<(String, String)> = doc
        .envelopes
        .iter()
        .filter(|envelope| {
            envelope
                .goal_id
                .as_deref()
                .is_some_and(|goal| !goals.contains(goal))
        })
        .map(|envelope| (envelope.id.clone(), envelope.name.clone()))
        .collect();
    if gone.is_empty() {
        return;
    }
    gone.sort();
    let ids: HashSet<&str> = gone.iter().map(|(id, _)| id.as_str()).collect();
    doc.envelopes
        .retain(|envelope| !ids.contains(envelope.id.as_str()));
    report.dropped_envelopes = gone;
}
