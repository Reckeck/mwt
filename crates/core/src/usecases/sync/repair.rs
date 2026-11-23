//! Scenarios of the repair pass: they run the pure rules of `sync::repair` over the document in
//! the order the rules need, ask the other areas for what a rule cannot read from the document,
//! stamp the result as one tracked edit of this device and record it in the batch that caused it
//! (so undoing the batch undoes the repair).

use std::collections::{BTreeMap, HashSet};

use crate::error::Result;
use crate::ipc_contract::MergeTarget;
use crate::model::notify::NoticeCategory;
use crate::model::vault::now_iso;
use crate::service::{CASH_WALLET_ID, UnlockedVault, notice};
use crate::sync::journal::SyncJournalEntryExt;
use crate::sync::journal::{SyncJournalEntry, SyncJournalOutcome};
use crate::sync::repair::bad_record::journal_has;
use crate::sync::repair::keep::{self, KeepTarget};
use crate::sync::repair::repair::{
    changed_keys, merge_batch_entries, push_notice, repair_account_tree, repair_goal_envelopes,
};
use crate::sync::repair::repair_cash::repair_cash_wallets;
use crate::sync::repair::repair_classes::{repair_class_links, repair_shares};
use crate::sync::repair::repair_events::{
    repair_event_links, repair_event_money, repair_guest_contacts,
};
use crate::sync::repair::repair_imports::repair_pending_imports;
use crate::sync::repair::repair_natural::repair_natural_keys;
use crate::sync::repair::repair_report::RepairReport;
use crate::sync::repair::repair_restore::{
    restore_participants, restore_referenced_records, restore_referenced_transactions,
    restore_removed_with_payment as restore_payment_parts,
};
use crate::sync::repair::repoint::repoint_tx as repoint_references;
use crate::sync::view;
use crate::sync::{BatchEntry, RecordState, SyncSummary};

/// Check the rules and fix what breaks them. `batch_id` is the sync batch
/// whose merge is being settled (the repair is recorded in it for undo);
/// `None` when a vault opens (the change is only stamped).
pub(crate) fn repair_invariants(
    vault: &mut UnlockedVault,
    batch_id: Option<&str>,
) -> Result<RepairReport> {
    let (mut report, changed) = tracked_edit(vault, batch_id, run_rules)?;
    report.records = changed;
    Ok(report)
}

/// Move every reference to the operation `old` onto the operation `new` for each pair, as one
/// tracked edit of this device: the records it changed are stamped (so the other devices get
/// them) and recorded in the batch for its undo.
pub(crate) fn repoint_removed_copies(
    vault: &mut UnlockedVault,
    pairs: &[(String, String)],
    batch_id: &str,
) -> Result<()> {
    if pairs.is_empty() {
        return Ok(());
    }
    tracked_edit(vault, Some(batch_id), |vault| {
        for (old, new) in pairs {
            repoint_references(&mut vault.doc, old, new);
        }
    })?;
    Ok(())
}

/// [`repoint_references`] over the open vault, for the tests that pin it.
#[cfg(test)]
pub(crate) fn repoint_tx(vault: &mut UnlockedVault, old: &str, new: &str) -> bool {
    repoint_references(&mut vault.doc, old, new)
}

/// A payment is back after its deletion: what left the shared event with it returns.
pub(crate) fn restore_removed_with_payment(vault: &mut UnlockedVault) {
    restore_payment_parts(&mut vault.doc);
}

/// «Allow merging»: clear the mark, so the next sync may fold the record
/// into its twin again. A local edit like any other.
pub fn allow_merging(vault: &mut UnlockedVault, target: MergeTarget, id: &str) -> Result<()> {
    let target = match target {
        MergeTarget::Transaction => KeepTarget::Transaction,
        MergeTarget::Reminder => KeepTarget::Reminder,
    };
    if !keep::is_kept(&vault.doc, target, id)? {
        return Ok(());
    }
    vault.transact(|tx| {
        keep::allow_merging(tx.doc_mut(), target, id);
        Ok(())
    })
}

/// Journal records left out of a batch, unless the same line is already
/// there: the peer offers them again every session, and the journal must
/// not fill with one line per session. True when a line was added (the
/// caller saves).
pub(crate) fn journal_skipped(
    vault: &mut UnlockedVault,
    peer_name: &str,
    summary: &SyncSummary,
) -> bool {
    let line = SyncJournalEntry::of(peer_name, summary, SyncJournalOutcome::Failed);
    if journal_has(&vault.doc.sync.journal, &line) {
        return false;
    }
    crate::usecases::sync::sync_journal::journal_push(
        vault,
        peer_name,
        summary,
        SyncJournalOutcome::Failed,
        None,
    );
    true
}

/// The rules in the order they run (see `sync::repair::repair`); the rules that need another
/// area ask the vault for it between the steps.
fn run_rules(vault: &mut UnlockedVault) -> RepairReport {
    let mut report = RepairReport::default();
    repair_restored(vault, &mut report);
    let decided: HashSet<String> =
        crate::cryptotrack::exchange_sync::decided_import_ids(&vault.doc);
    repair_pending_imports(&mut vault.doc, &decided, &mut report);
    // Before the natural keys: a folded guest contact may leave two debt
    // terms of one contact and asset behind.
    repair_guest_contacts(&mut vault.doc, &mut report);
    repair_natural_keys(&mut vault.doc, &mut report);
    repair_cash_wallets(&mut vault.doc, CASH_WALLET_ID, &mut report);
    repair_account_tree(&mut vault.doc, &mut report);
    repair_goal_envelopes(&mut vault.doc, &mut report);
    let known: HashSet<String> = crate::planning::allocation::effective_classes(&vault.doc)
        .into_iter()
        .map(|class| class.id)
        .collect();
    repair_class_links(&mut vault.doc, &known, &mut report);
    repair_shares(&mut vault.doc, &mut report);
    repair_event_links(&mut vault.doc, &mut report);
    repair_event_money(&mut vault.doc, &mut report);
    // R8. Things another device deleted may still be named by a record
    // that arrived (an emergency account, an inflation source). Shared
    // events keep their references: a settlement or item that names a
    // removed participant is user data (the next rules of stage M).
    crate::usecases::planning::emergency::prune_emergency_fund(vault);
    crate::usecases::planning::datasources::prune_inflation_sources(vault);
    announce_repair(vault, &report);
    report
}

/// R3, the first rule: a deleted record that live money still points to comes back.
fn repair_restored(vault: &mut UnlockedVault, report: &mut RepairReport) {
    // First: a purchase that comes back names its buyer, looked at next.
    restore_payment_parts(&mut vault.doc);
    restore_participants(&mut vault.doc, report);
    // Before stores, categories and contacts: a restored payment names
    // its own wallets and categories, and they must be looked at too.
    if restore_referenced_transactions(&mut vault.doc, report) {
        // Balances follow the operations.
        crate::usecases::money::portfolio::recompute_holdings(vault);
    }
    restore_referenced_records(&mut vault.doc, report);
}

/// The notice of a repair, added to the (synced) log under a key the repair itself decides.
fn announce_repair(vault: &mut UnlockedVault, report: &RepairReport) {
    let ru = vault.notice_in_russian();
    let Some(body) = report.notice_body(ru) else {
        return;
    };
    let record = notice(
        NoticeCategory::Security,
        report.notice_key(),
        report.notice_title(ru),
        body,
        Some("/?settings=sync".into()),
        now_iso(),
    );
    push_notice(&mut vault.doc, record);
}

/// Run `edit` as one tracked edit of this device: when it changed the document, stamp the
/// changed records (so the other devices get them) and, given a batch, record them in it for its
/// undo. The keys of the records it changed (none when it changed nothing).
fn tracked_edit<R>(
    vault: &mut UnlockedVault,
    batch_id: Option<&str>,
    edit: impl FnOnce(&mut UnlockedVault) -> R,
) -> Result<(R, Vec<String>)> {
    let before = vault.doc_value()?;
    let out = edit(vault);
    let after = vault.doc_value()?;
    if after == before {
        return Ok((out, Vec::new()));
    }
    let pre_records = view::records(&before);
    let post_records = view::records(&after);
    // The states the batch has to restore, before the tracker stamps.
    let mut before_states: BTreeMap<String, RecordState> =
        changed_keys(&pre_records, &post_records)
            .into_iter()
            .map(|key| {
                let state =
                    crate::sync::manifest_view::record_state(&vault.doc, &pre_records, &key);
                (key, state)
            })
            .collect();
    let changed = vault.track_changes()?;
    if let Some(id) = batch_id {
        let settled = view::records(&vault.doc_value()?);
        let entries: Vec<BatchEntry> = changed
            .iter()
            .filter_map(|key| {
                let prior = before_states.remove(key)?;
                Some(BatchEntry {
                    after: crate::sync::manifest_view::record_state(&vault.doc, &settled, key)
                        .marks(),
                    before: prior,
                    key: key.clone(),
                })
            })
            .collect();
        if let Some(batch) = vault
            .doc
            .sync
            .batches
            .iter_mut()
            .find(|batch| batch.id == id)
        {
            merge_batch_entries(&mut batch.entries, entries);
        }
    }
    Ok((out, changed))
}
