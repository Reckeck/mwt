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
