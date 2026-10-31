//! The vault side of the sync journal (`crate::sync::journal`).

use super::UnlockedVault;
use crate::error::Result;
use crate::model::vault::now_iso;
use crate::sync::SyncSummary;
use crate::sync::journal::{SyncJournalEntry, SyncJournalOutcome, push};

impl UnlockedVault {
    /// Add a line without saving (the caller saves right after).
    pub(super) fn journal_push(
        &mut self,
        peer_name: &str,
        summary: &SyncSummary,
        outcome: SyncJournalOutcome,
        batch_id: Option<String>,
    ) {
        let mut entry = SyncJournalEntry::of(peer_name, summary, outcome);
        entry.batch_id = batch_id;
        entry.id = uuid::Uuid::new_v4().to_string();
        entry.at = now_iso();
        push(&mut self.doc.sync.journal, entry);
    }

    /// Note a batch that was not applied (declined, failed) and save.
    pub fn sync_journal_record(
        &mut self,
        peer_name: &str,
        summary: &SyncSummary,
        outcome: SyncJournalOutcome,
    ) -> Result<()> {
        self.journal_push(peer_name, summary, outcome, None);
        self.persist()
    }

    /// One-time cleanup on open: batches and journal lines written before
    /// secret collections were hidden keep no label or value of them.
    /// `true` when something changed (the caller saves only then).
    pub(super) fn scrub_sync_secrets(&mut self) -> bool {
        let mut changed = false;
        for batch in &mut self.doc.sync.batches {
            changed |= batch.summary.scrub_secrets();
        }
        for entry in &mut self.doc.sync.journal {
            changed |= entry.scrub_secrets();
        }
        changed
    }

    /// The journal, newest first.
    pub fn sync_journal(&self) -> Vec<SyncJournalEntry> {
        self.doc.sync.journal.iter().rev().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::empty_vault;
    use super::*;
    use crate::sync::journal::MAX_JOURNAL;
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
                }],
                ..CollectionChanges::default()
            }],
            overwritten: Vec::new(),
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
            vault.journal_push("Pixel", &one_change(), outcome, None);
        }
        let log = vault.sync_journal();
        assert_eq!(log.len(), MAX_JOURNAL);
        assert_eq!(log[0].outcome, SyncJournalOutcome::Failed);
        assert!(!log[0].id.is_empty() && !log[0].at.is_empty());
    }

    #[test]
    fn a_declined_batch_is_saved_in_the_journal() {
        let (_dir, mut vault) = empty_vault();
        vault
            .sync_journal_record("Pixel", &one_change(), SyncJournalOutcome::Declined)
            .unwrap();
        assert_eq!(
            vault.sync_journal()[0].outcome,
            SyncJournalOutcome::Declined
        );
    }
}

#[cfg(test)]
mod scrub_tests {
    use super::super::test_support::empty_vault;
    use super::*;
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
                    }],
                    ..Default::default()
                },
                CollectionChanges {
                    collection: "stores".into(),
                    added: 1,
                    items: vec![ChangeItem {
                        kind: ChangeKind::Added,
                        label: "Cash".into(),
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

        assert!(vault.scrub_sync_secrets());
        let text =
            serde_json::to_string(&(&vault.doc.sync.batches, &vault.doc.sync.journal)).unwrap();
        assert!(
            !text.contains("OLDSECRET") && !text.contains("NEWSECRET"),
            "{text}"
        );
        // Ordinary collections keep theirs.
        assert!(text.contains("Cash"));
        // The second run finds nothing to do.
        assert!(!vault.scrub_sync_secrets());
    }
}
