//! Scenarios of the sync journal: they read the clock and mint the id of a line, then edit the
//! journal in `SyncState` through the pure functions of `sync::sync_journal`.

use crate::error::Result;
use crate::model::vault::now_iso;
use crate::service::UnlockedVault;
use crate::sync::SyncSummary;
use crate::sync::journal::{SyncJournalEntry, SyncJournalOutcome};
use crate::sync::sync_journal::{newest_first, push_line, scrub_secrets};

fn new_line_key() -> (String, String) {
    (uuid::Uuid::new_v4().to_string(), now_iso())
}

/// Add a line without saving (the caller saves right after).
pub(crate) fn journal_push(
    vault: &mut UnlockedVault,
    peer_name: &str,
    summary: &SyncSummary,
    outcome: SyncJournalOutcome,
    batch_id: Option<String>,
) {
    push_line(
        &mut vault.doc.sync,
        peer_name,
        summary,
        outcome,
        batch_id,
        new_line_key(),
    );
}

/// Note a batch that was not applied (declined, failed) and save.
pub fn sync_journal_record(
    vault: &mut UnlockedVault,
    peer_name: &str,
    summary: &SyncSummary,
    outcome: SyncJournalOutcome,
) -> Result<()> {
    let key = new_line_key();
    vault.transact(|tx| {
        push_line(
            &mut tx.doc_mut().sync,
            peer_name,
            summary,
            outcome,
            None,
            key,
        );
        Ok(())
    })
}

/// One-time cleanup on open: `true` when something changed (the caller saves only then).
pub(crate) fn scrub_sync_secrets(vault: &mut UnlockedVault) -> bool {
    scrub_secrets(&mut vault.doc.sync)
}

/// The journal, newest first.
pub fn sync_journal(vault: &UnlockedVault) -> Vec<SyncJournalEntry> {
    newest_first(&vault.doc.sync)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::empty_vault;
    use crate::sync::journal::{MAX_JOURNAL, SyncJournalEntryExt};
    use crate::sync::{ChangeItem, ChangeKind, CollectionChanges};

    fn one_change() -> SyncSummary {
        SyncSummary {
            total: 1,
            collections: vec![CollectionChanges {
                collection: "transactions".into(),
                added: 1,
                items: vec![ChangeItem {
                    kind: ChangeKind::Added,
                    label: "Кофе".into(),
                    ..Default::default()
                }],
                ..CollectionChanges::default()
            }],
            overwritten: Vec::new(),
            ..Default::default()
        }
    }

    #[test]
    fn a_single_change_keeps_its_label_and_many_keep_counts() {
        let entry = SyncJournalEntry::of("Pixel", &one_change(), SyncJournalOutcome::Declined);
        assert_eq!(entry.label.as_deref(), Some("Кофе"));
        assert_eq!(entry.counts[0].count, 1);
        let mut many = one_change();
        many.total = 2;
        many.collections[0].added = 2;
        assert_eq!(
            SyncJournalEntry::of("Pixel", &many, SyncJournalOutcome::Applied).label,
            None
        );
    }

    #[test]
    fn the_journal_is_a_ring_and_lists_newest_first() {
        let (_dir, mut vault) = empty_vault();
        for round in 0..MAX_JOURNAL + 5 {
            let outcome = if round == MAX_JOURNAL + 4 {
                SyncJournalOutcome::Failed
            } else {
                SyncJournalOutcome::Declined
            };
            crate::usecases::sync::sync_journal::journal_push(
                &mut vault,
                "Pixel",
                &one_change(),
                outcome,
                None,
            );
        }
        let log = crate::usecases::sync::sync_journal::sync_journal(&vault);
        assert_eq!(log.len(), MAX_JOURNAL);
        assert_eq!(log[0].outcome, SyncJournalOutcome::Failed);
        assert!(!log[0].id.is_empty() && !log[0].at.is_empty());
    }

    #[test]
    fn a_declined_batch_is_saved_in_the_journal() {
        let (_dir, mut vault) = empty_vault();
        crate::usecases::sync::sync_journal::sync_journal_record(
            &mut vault,
            "Pixel",
            &one_change(),
            SyncJournalOutcome::Declined,
        )
        .unwrap();
        assert_eq!(
            crate::usecases::sync::sync_journal::sync_journal(&vault)[0].outcome,
            SyncJournalOutcome::Declined
        );
    }
}

#[cfg(test)]
mod scrub_tests {
    use super::*;
    use crate::service::test_support::empty_vault;
    use crate::sync::SyncBatch;
    use crate::sync::journal::SyncJournalCount;
    use crate::sync::plan::{ChangeItem, ChangeKind, CollectionChanges, OverwrittenField};

    fn leaky_summary() -> SyncSummary {
        SyncSummary {
            total: 2,
            collections: vec![
                CollectionChanges {
                    collection: "exchanges".into(),
                    updated: 1,
                    items: vec![ChangeItem {
                        kind: ChangeKind::Updated,
                        label: "{\"api_secret\":\"OLDSECRET\"}".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                CollectionChanges {
                    collection: "stores".into(),
                    added: 1,
                    items: vec![ChangeItem {
                        kind: ChangeKind::Added,
                        label: "Cash".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            overwritten: vec![OverwrittenField {
                collection: "exchanges".into(),
                label: "OLDSECRET".into(),
                field: "bybit".into(),
                before: "OLDSECRET".into(),
                after: "NEWSECRET".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn the_scrub_removes_old_secret_labels_and_is_idempotent() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.sync.batches.push(SyncBatch {
            id: "b".into(),
            peer_device: "d".into(),
            peer_name: "Phone".into(),
            applied_at: String::new(),
            summary: leaky_summary(),
            entries: Vec::new(),
            undone_at: None,
        });
        vault.doc.sync.journal.push(SyncJournalEntry {
            peer_name: "Phone".into(),
            counts: vec![SyncJournalCount {
                collection: "exchanges".into(),
                count: 1,
            }],
            label: Some("OLDSECRET".into()),
            outcome: SyncJournalOutcome::Applied,
            ..Default::default()
        });
        vault.doc.sync.journal.push(SyncJournalEntry {
            counts: vec![SyncJournalCount {
                collection: "stores".into(),
                count: 1,
            }],
            label: Some("Cash".into()),
            outcome: SyncJournalOutcome::Applied,
            ..Default::default()
        });

        assert!(crate::usecases::sync::sync_journal::scrub_sync_secrets(
            &mut vault
        ));
        let text =
            serde_json::to_string(&(&vault.doc.sync.batches, &vault.doc.sync.journal)).unwrap();
        assert!(
            !text.contains("OLDSECRET") && !text.contains("NEWSECRET"),
            "{text}"
        );
        // Ordinary collections keep theirs.
        assert!(text.contains("Cash"));
        // The second run finds nothing to do.
        assert!(!crate::usecases::sync::sync_journal::scrub_sync_secrets(
            &mut vault
        ));
    }
}
